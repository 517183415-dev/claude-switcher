//! Bounded, read-only macOS observations. Raw output is never a report field.
use sha2::{Digest, Sha256};
use std::{process::Stdio, sync::Mutex, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};

const OUTPUT_LIMIT: u64 = 256 * 1024;
const DNS_COMMAND_TIMEOUT: Duration = Duration::from_secs(2);
#[derive(Default)]
struct Generation {
    fingerprint: Option<String>,
    revision: u64,
}
impl Generation {
    fn observe(
        &mut self,
        fingerprint: String,
        expected_revision: u64,
    ) -> Result<String, &'static str> {
        if self.revision != expected_revision {
            return Err("network_changed_during_observation");
        }
        if self
            .fingerprint
            .as_ref()
            .is_some_and(|previous| previous != &fingerprint)
        {
            self.revision = self.revision.saturating_add(1);
        }
        self.fingerprint = Some(fingerprint.clone());
        Ok(format!("{}:{fingerprint}", self.revision))
    }
}
static NETWORK_GENERATION: Mutex<Generation> = Mutex::new(Generation {
    fingerprint: None,
    revision: 0,
});

/// Call for every native network/wake event, including a change back to the original route.
pub fn invalidate_network_generation() {
    if let Ok(mut state) = NETWORK_GENERATION.lock() {
        state.revision = state.revision.saturating_add(1);
    }
}

/// A process keeps its registration and receiver for its entire lifetime. A failed
/// registration is sticky: evidence must never outlive the observer that protects it.
struct ProcessObserver<T> {
    registration: std::sync::OnceLock<Result<T, &'static str>>,
}
impl<T> ProcessObserver<T> {
    const fn new() -> Self {
        Self {
            registration: std::sync::OnceLock::new(),
        }
    }
    fn ensure(
        &self,
        register: impl FnOnce() -> Result<T, &'static str>,
        healthy: impl FnOnce(&T) -> bool,
    ) -> Result<(), &'static str> {
        let registration = self
            .registration
            .get_or_init(register)
            .as_ref()
            .map_err(|e| *e)?;
        if !healthy(registration) {
            return Err("network_observer_stopped");
        }
        Ok(())
    }
}
#[cfg(target_os = "macos")]
struct NetworkObserver {
    native: crate::guardian::NativeEvents,
    receiver: Mutex<tokio::sync::mpsc::Receiver<crate::guardian::ProtectionEvent>>,
}
#[cfg(target_os = "macos")]
static NETWORK_OBSERVER: ProcessObserver<NetworkObserver> = ProcessObserver::new();

/// Synchronously install both native subscriptions before collecting any evidence.
/// Reuse one observer rather than starting threads for individual checks or launches.
pub(crate) fn ensure_network_observation() -> Result<(), &'static str> {
    #[cfg(target_os = "macos")]
    {
        NETWORK_OBSERVER.ensure(
            || {
                let (native, receiver) = crate::guardian::native_events()?;
                Ok(NetworkObserver {
                    native,
                    receiver: Mutex::new(receiver),
                })
            },
            |observer| {
                observer.native.is_running()
                    && observer
                        .receiver
                        .lock()
                        .is_ok_and(|receiver| !receiver.is_closed())
            },
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("platform_unsupported")
    }
}

#[cfg(test)]
pub(crate) mod command_fixture {
    pub type Outputs = std::collections::HashMap<String, Result<String, &'static str>>;
    tokio::task_local! { pub static OUTPUTS: Outputs; }
    #[derive(Debug)]
    pub struct TimeoutReceipt {
        pub pid: Option<u32>,
        pub kill_ok: bool,
        pub wait_ok: bool,
    }
    tokio::task_local! { pub static TIMEOUTS: std::cell::RefCell<Vec<TimeoutReceipt>>; }
    pub fn key(program: &str, args: &[&str]) -> String {
        std::iter::once(program)
            .chain(args.iter().copied())
            .collect::<Vec<_>>()
            .join("\t")
    }
}

pub async fn read_command(program: &str, args: &[&str]) -> Result<String, &'static str> {
    read_command_with_timeout(program, args, Duration::from_secs(2)).await
}

async fn read_command_with_timeout(
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<String, &'static str> {
    #[cfg(test)]
    if let Ok(result) = command_fixture::OUTPUTS.try_with(|outputs| {
        outputs
            .get(&command_fixture::key(program, args))
            .cloned()
            .unwrap_or(Err("fixture_command_absent"))
    }) {
        return result;
    }
    if !cfg!(target_os = "macos") {
        return Err("platform_unsupported");
    }
    let mut child = Command::new(program)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "observation_unavailable")?;
    let stdout = child.stdout.take().ok_or("observation_unavailable")?;
    let observation = async {
        let mut data = Vec::new();
        stdout
            .take(OUTPUT_LIMIT + 1)
            .read_to_end(&mut data)
            .await
            .map_err(|_| "observation_unavailable")?;
        if data.len() > OUTPUT_LIMIT as usize {
            return Err("observation_too_large");
        }
        if !child
            .wait()
            .await
            .map_err(|_| "observation_unavailable")?
            .success()
        {
            return Err("observation_unavailable");
        }
        String::from_utf8(data).map_err(|_| "observation_invalid")
    };
    match tokio::time::timeout(timeout, observation).await {
        Ok(result) => result,
        Err(_) => {
            let _pid = child.id();
            let _killed = child.kill().await;
            let _waited = child.wait().await;
            #[cfg(test)]
            let _ = command_fixture::TIMEOUTS.try_with(|receipts| {
                receipts.borrow_mut().push(command_fixture::TimeoutReceipt {
                    pid: _pid,
                    kill_ok: _killed.is_ok(),
                    wait_ok: _waited.is_ok(),
                });
            });
            Err("observation_timeout")
        }
    }
}

/// Opaque network generation. Interface, route and resolver changes invalidate old evidence.
pub async fn network_generation() -> Result<String, &'static str> {
    ensure_network_observation()?;
    let revision = NETWORK_GENERATION
        .lock()
        .map_err(|_| "network_generation_unavailable")?
        .revision;
    let (interfaces, route, dns) = tokio::join!(
        read_command("/usr/sbin/scutil", &["--nwi"]),
        read_command("/sbin/route", &["-n", "get", "1.1.1.1"]),
        read_command("/usr/sbin/scutil", &["--dns"])
    );
    let mut hash = Sha256::new();
    for value in [interfaces?, route?, dns?] {
        hash.update(value.as_bytes());
        hash.update([0]);
    }
    let fingerprint = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    ensure_network_observation()?;
    NETWORK_GENERATION
        .lock()
        .map_err(|_| "network_generation_unavailable")?
        .observe(fingerprint, revision)
}

pub fn route_uses_tunnel(output: &str) -> bool {
    output
        .lines()
        .filter_map(|line| line.trim().strip_prefix("interface:"))
        .map(str::trim)
        .any(|value| {
            value
                .strip_prefix("utun")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
}

pub fn fake_ip_interface(output: &str) -> bool {
    output.lines().any(|line| {
        let mut fields = line.split_whitespace();
        fields.next() == Some("inet") && fields.next().is_some_and(|ip| ip.starts_with("198.18."))
    })
}
fn shadowrocket_connected(output: &str) -> Option<bool> {
    let rows: Vec<_> = output
        .lines()
        .filter(|line| line.contains("com.liguangming.Shadowrocket"))
        .collect();
    (!rows.is_empty()).then(|| rows.iter().any(|line| line.contains("(Connected)")))
}
pub async fn tunnel_route() -> Result<bool, &'static str> {
    read_command("/sbin/ifconfig", &[])
        .await
        .map(|s| fake_ip_interface(&s))
}
async fn shadowrocket_up() -> bool {
    match read_command("/usr/sbin/scutil", &["--nc", "list"])
        .await
        .ok()
        .and_then(|output| shadowrocket_connected(&output))
    {
        Some(connected) => connected,
        None => tunnel_route().await.unwrap_or(false),
    }
}

fn proxy_process_names(mode: &str) -> &'static [&'static str] {
    match mode {
        "clash-verge" => &["verge-mihomo"],
        "0dcloud" => &["0dcloudCore"],
        "shadowrocket" => &["Shadowrocket"],
        "sing-box" => &["sing-box"],
        "clashx" => &["ClashX", "clash"],
        "surge" => &["Surge"],
        _ => &[
            "verge-mihomo",
            "0dcloudCore",
            "Shadowrocket",
            "sing-box",
            "ClashX",
            "clash",
            "Surge",
        ],
    }
}

pub async fn proxy_running(mode: &str) -> Result<bool, &'static str> {
    if mode == "shadowrocket" {
        return Ok(shadowrocket_up().await);
    }
    for name in proxy_process_names(mode) {
        if *name == "Shadowrocket" {
            if shadowrocket_up().await {
                return Ok(true);
            }
            continue;
        }
        let flag = if matches!(*name, "0dcloudCore" | "ClashX") {
            "-x"
        } else {
            "-f"
        };
        if read_command("/usr/bin/pgrep", &[flag, name])
            .await
            .is_ok_and(|s| s.lines().any(|line| line.parse::<u32>().is_ok()))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub async fn dns_addresses(host: &str) -> Result<Vec<std::net::IpAddr>, &'static str> {
    dns_addresses_with_program(host, "/usr/bin/dig").await
}

async fn dns_addresses_with_program(
    host: &str,
    program: &str,
) -> Result<Vec<std::net::IpAddr>, &'static str> {
    let output = read_command_with_timeout(
        program,
        &["+short", "+time=3", "+tries=1", host],
        DNS_COMMAND_TIMEOUT,
    )
    .await?;
    // Keep the first DNS answer authoritative, including a non-address (CNAME).
    let values: Vec<_> = output
        .lines()
        .next()
        .and_then(|line| line.trim().parse().ok())
        .into_iter()
        .collect();
    if values.is_empty() {
        Err("dns_observation_unavailable")
    } else {
        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // No DNS resolver or network module is used by these disposable child programs.
    #[cfg(target_os = "macos")]
    struct F08Child {
        directory: std::path::PathBuf,
        script: String,
    }
    #[cfg(target_os = "macos")]
    impl F08Child {
        fn new(body: &str) -> Self {
            use std::os::unix::fs::PermissionsExt;
            let python = std::process::Command::new("python3")
                .args(["-I", "-S", "-c", "import sys; print(sys.executable)"])
                .output()
                .expect("F08 requires the existing Python interpreter");
            assert!(python.status.success());
            let python = String::from_utf8(python.stdout).unwrap();
            let python = python.trim();
            assert!(python.starts_with('/') && !python.contains(char::is_whitespace));
            let directory = std::env::temp_dir().join(format!(
                "switcher-f08-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&directory).unwrap();
            let path = directory.join("fake-dig.py");
            let source = format!(
                "#!{python} -I\nimport os, sys, time\nfrom pathlib import Path\nPath(__file__).with_suffix('.pid').write_text(str(os.getpid()))\nassert os.environ['PATH'] == '/usr/bin:/bin:/usr/sbin:/sbin'\nassert os.environ['LC_ALL'] == 'C'\nassert sys.stdin.read() == ''\n{body}\n"
            );
            std::fs::write(&path, &source).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            use sha2::Digest;
            println!(
                "F08 child python={python} source_sha256={:x} body={body:?}",
                Sha256::digest(source.as_bytes())
            );
            Self {
                directory,
                script: path.to_str().unwrap().to_owned(),
            }
        }
        fn pid(&self) -> u32 {
            std::fs::read_to_string(self.directory.join("fake-dig.pid"))
                .unwrap()
                .parse()
                .unwrap()
        }
    }
    #[cfg(target_os = "macos")]
    impl Drop for F08Child {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn f08_dns_accepts_real_delayed_child() {
        let child = F08Child::new(
            "assert sys.argv[1:] == ['+short', '+time=3', '+tries=1', 'fixture.invalid']\ntime.sleep(2.5)\nprint('198.18.0.1')",
        );
        let started = std::time::Instant::now();
        let result = dns_addresses_with_program("fixture.invalid", &child.script).await;
        println!(
            "F08 delayed DNS elapsed={:?} pid={} result={result:?}",
            started.elapsed(),
            child.pid()
        );
        assert_eq!(result, Ok(vec!["198.18.0.1".parse().unwrap()]));
        assert!(started.elapsed() >= Duration::from_millis(2500));
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn f08_generic_keeps_two_second_timeout_and_waits() {
        let child = F08Child::new("time.sleep(2.5)\nprint('198.18.0.1')");
        command_fixture::TIMEOUTS
            .scope(Default::default(), async {
                let started = std::time::Instant::now();
                assert_eq!(
                    read_command(&child.script, &[]).await,
                    Err("observation_timeout")
                );
                let elapsed = started.elapsed();
                assert!(elapsed >= Duration::from_secs(2));
                command_fixture::TIMEOUTS.with(|receipts| {
                    let receipts = receipts.borrow();
                    println!("F08 generic elapsed={elapsed:?} cleanup={receipts:?}");
                    assert_eq!(receipts.len(), 1);
                    assert_eq!(receipts[0].pid, Some(child.pid()));
                    assert!(receipts[0].kill_ok && receipts[0].wait_ok);
                });
            })
            .await;
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn f08_dns_deadline_covers_stdout_and_wait_then_reaps() {
        for body in ["time.sleep(30)", "os.close(1)\ntime.sleep(30)"] {
            let child = F08Child::new(body);
            command_fixture::TIMEOUTS.scope(Default::default(), async {
                let started = std::time::Instant::now();
                assert_eq!(dns_addresses_with_program("fixture.invalid", &child.script).await, Err("observation_timeout"));
                let elapsed = started.elapsed();
                assert!(elapsed >= DNS_COMMAND_TIMEOUT);
                command_fixture::TIMEOUTS.with(|receipts| {
                    let receipts = receipts.borrow();
                    println!("F08 DNS deadline elapsed={elapsed:?} body={body:?} cleanup={receipts:?}");
                    assert_eq!(receipts.len(), 1);
                    assert_eq!(receipts[0].pid, Some(child.pid()));
                    assert!(receipts[0].kill_ok && receipts[0].wait_ok);
                });
            }).await;
        }
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn f08_real_children_preserve_output_and_first_answer_contract() {
        for (body, expected) in [
            (
                "sys.stdout.buffer.write(b'\\xff')",
                Err("observation_invalid"),
            ),
            (
                "print('198.18.0.1')\nsys.exit(7)",
                Err("observation_unavailable"),
            ),
            (
                "sys.stdout.write('x' * (256 * 1024 + 1))",
                Err("observation_too_large"),
            ),
            (
                "print('alias.invalid')\nprint('198.18.0.1')",
                Err("dns_observation_unavailable"),
            ),
            (
                "print('198.18.0.1')\nprint('198.18.0.2')\nsys.stderr.write('ignored')",
                Ok(vec!["198.18.0.1".parse().unwrap()]),
            ),
        ] {
            let child = F08Child::new(body);
            let result = dns_addresses_with_program("fixture.invalid", &child.script).await;
            println!(
                "F08 output child pid={} body={body:?} result={result:?}",
                child.pid()
            );
            assert_eq!(result, expected);
        }
        let child = F08Child::new("sys.stdout.write('x' * (256 * 1024))");
        assert_eq!(
            read_command(&child.script, &[]).await.unwrap().len(),
            OUTPUT_LIMIT as usize
        );
    }

    #[tokio::test]
    async fn legacy_proxy_and_tun_observe_vpn_and_fake_ip_not_route() {
        use command_fixture::*;
        for (vpn, expected) in [
            ("* (Connected) com.liguangming.Shadowrocket", true),
            ("* (Disconnected) com.liguangming.Shadowrocket", false),
            ("other VPN", true),
        ] {
            let outputs = Outputs::from([
                (key("/usr/sbin/scutil", &["--nc", "list"]), Ok(vpn.into())),
                (
                    key("/sbin/ifconfig", &[]),
                    Ok("inet 198.18.0.1 netmask 0xffff0000".into()),
                ),
            ]);
            OUTPUTS
                .scope(outputs, async {
                    assert_eq!(proxy_running("shadowrocket").await.unwrap(), expected);
                    assert!(tunnel_route().await.unwrap());
                })
                .await;
        }
    }
    #[test]
    fn legacy_tun_and_shadowrocket_conditions() {
        assert!(fake_ip_interface(
            "utun0: flags\n    inet 198.18.0.1 netmask 0xffff0000"
        ));
        assert!(!fake_ip_interface("inet 198.19.0.1"));
        assert_eq!(
            shadowrocket_connected("* (Connected) com.liguangming.Shadowrocket"),
            Some(true)
        );
        assert_eq!(
            shadowrocket_connected("* (Disconnected) com.liguangming.Shadowrocket"),
            Some(false)
        );
        assert_eq!(shadowrocket_connected("other VPN"), None);
    }
    #[test]
    fn only_a_real_tunnel_interface_is_accepted() {
        assert!(route_uses_tunnel("  interface: utun8\n"));
        for input in [
            "interface: en0",
            "interface: utun",
            "gateway: utun8",
            "interface: utun8evil",
        ] {
            assert!(!route_uses_tunnel(input));
        }
    }
    #[test]
    fn automatic_proxy_adapter_includes_odcloud_and_clash_verge() {
        assert!(proxy_process_names("auto").contains(&"0dcloudCore"));
        assert!(proxy_process_names("auto").contains(&"verge-mihomo"));
        assert_eq!(proxy_process_names("0dcloud"), ["0dcloudCore"]);
    }
    #[test]
    fn generation_rejects_aba_and_events_during_observation() {
        let mut state = Generation::default();
        let first = state.observe("route-a".into(), 0).unwrap();
        assert_eq!(state.observe("route-a".into(), 0).unwrap(), first);
        let second = state.observe("route-b".into(), 0).unwrap();
        assert_ne!(first, second);
        let returned = state.observe("route-a".into(), 1).unwrap();
        assert_ne!(returned, first);
        state.revision += 1;
        assert!(state.observe("route-a".into(), 2).is_err());
    }
}

/// Isolated subscription backend. Events go only to registered process states;
/// sampling the same route never invents an event or registers a listener.
#[cfg(test)]
pub(crate) mod test_observer {
    use super::ProcessObserver;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    pub(crate) struct Observer {
        process: ProcessObserver<AtomicU64>,
        failed: bool,
        alive: AtomicBool,
        registrations: AtomicUsize,
    }
    impl Observer {
        pub(crate) fn new(failed: bool) -> Self {
            Self {
                process: ProcessObserver::new(),
                failed,
                alive: AtomicBool::new(true),
                registrations: AtomicUsize::new(0),
            }
        }
        pub(crate) fn ensure(&self) -> Result<(), &'static str> {
            self.process.ensure(
                || {
                    self.registrations.fetch_add(1, Ordering::SeqCst);
                    if self.failed {
                        Err("network_events_unavailable")
                    } else {
                        Ok(AtomicU64::new(0))
                    }
                },
                |_| self.alive.load(Ordering::SeqCst),
            )
        }
        pub(crate) fn generation(&self) -> Result<String, &'static str> {
            if !self.alive.load(Ordering::SeqCst) {
                return Err("network_observer_stopped");
            }
            let epoch = self
                .process
                .registration
                .get()
                .and_then(|r| r.as_ref().ok())
                .map_or(0, |s| s.load(Ordering::SeqCst));
            Ok(if epoch == 0 {
                "generation".into()
            } else {
                format!("{epoch}:route-a")
            })
        }
        pub(crate) fn emit(&self, event: crate::guardian::ProtectionEvent) {
            if event != crate::guardian::ProtectionEvent::Timer
                && let Some(Ok(epoch)) = self.process.registration.get()
            {
                // ABA returns to route A without changing the sampled fingerprint.
                epoch.fetch_add(1, Ordering::SeqCst);
            }
        }
        pub(crate) fn stop(&self) {
            self.alive.store(false, Ordering::SeqCst);
        }
        pub(crate) fn registrations(&self) -> usize {
            self.registrations.load(Ordering::SeqCst)
        }
    }
    #[test]
    fn network_epoch_guardian_event_does_not_invalidate_unregistered_frontend() {
        let guardian = Observer::new(false);
        let frontend = Observer::new(false);
        guardian.ensure().unwrap();
        let old = frontend.generation().unwrap();
        guardian.emit(crate::guardian::ProtectionEvent::NetworkChanged);
        guardian.emit(crate::guardian::ProtectionEvent::Wake);
        assert_ne!(guardian.generation().unwrap(), old);
        assert_eq!(frontend.generation().unwrap(), old);
        assert_eq!(frontend.registrations(), 0);
    }
    #[test]
    fn network_epoch_registration_is_reused_and_failure_is_sticky() {
        for failure in [false, true] {
            let observer = Observer::new(failure);
            for _ in 0..3 {
                assert_eq!(observer.ensure().is_err(), failure);
            }
            assert_eq!(observer.registrations(), 1);
            observer.stop();
            assert!(observer.ensure().is_err());
            assert!(observer.generation().is_err());
        }
    }
}
