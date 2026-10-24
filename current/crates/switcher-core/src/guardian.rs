//! User-session protection. The stop channel never acquires LifecycleLock.
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};
use switcher_domain::checks::CheckReport;
pub const FALLBACK_INTERVAL: Duration = Duration::from_secs(10);
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuardianHealth {
    pub owner_uid: u32,
    pub pid: i32,
    pub heartbeat_at: u64,
    pub started_at: u64,
    pub policy: crate::config::CheckConfig,
    pub policy_fingerprint: String,
    pub degraded: bool,
}

pub fn read_health(
    path: &Path,
    expected_uid: u32,
    now: u64,
) -> Result<GuardianHealth, &'static str> {
    use std::{
        io::Read,
        os::unix::fs::{MetadataExt, OpenOptionsExt},
    };
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| "guardian_missing")?;
    let m = file.metadata().map_err(|_| "guardian_metadata")?;
    if m.uid() != expected_uid || !m.is_file() || m.mode() & 0o077 != 0 || m.len() > 4096 {
        return Err("guardian_permissions");
    }
    let mut bytes = Vec::new();
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| "guardian_read")?;
    let health: GuardianHealth = serde_json::from_slice(&bytes).map_err(|_| "guardian_schema")?;
    if health.owner_uid != expected_uid
        || health.pid <= 1
        || health.heartbeat_at > now
        || now.saturating_sub(health.heartbeat_at) > 30
        || health.started_at > health.heartbeat_at
    {
        return Err("guardian_stale");
    }
    validate_health_policy(path, &health)?;
    let record = read_install_record()?;
    if record.owner_uid != expected_uid {
        return Err("guardian_install_owner");
    }
    verify_process_requirement(
        health.pid,
        expected_uid,
        &record.client_requirement(switcher_domain::system::ClientRole::Guardian)?,
        None,
    )?;
    if read_install_record()? != record {
        return Err("guardian_install_identity_changed");
    }
    Ok(health)
}

pub(crate) fn read_install_record() -> Result<switcher_domain::system::InstallRecord, &'static str>
{
    use std::{
        io::Read,
        os::unix::fs::{MetadataExt, OpenOptionsExt},
    };
    let path = Path::new(switcher_domain::system::INSTALL_RECORD);
    let mut parent = path.parent();
    while let Some(p) = parent {
        let m = std::fs::symlink_metadata(p).map_err(|_| "install_record_parent")?;
        if !m.is_dir() || m.uid() != 0 || m.mode() & 0o022 != 0 {
            return Err("install_record_permissions");
        }
        parent = p.parent();
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| "install_record_missing")?;
    let m = file.metadata().map_err(|_| "install_record_metadata")?;
    if !m.is_file() || m.uid() != 0 || m.mode() & 0o022 != 0 || m.len() > 16384 {
        return Err("install_record_permissions");
    }
    let mut bytes = Vec::new();
    file.take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| "install_record_read")?;
    let record: switcher_domain::system::InstallRecord =
        serde_json::from_slice(&bytes).map_err(|_| "install_record_invalid")?;
    record.validate()?;
    Ok(record)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionEvent {
    Timer,
    NetworkChanged,
    Wake,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopRequest {
    pub owner_uid: u32,
    pub reason: &'static str,
}
pub struct Guardian {
    owner_uid: u32,
    report: Option<CheckReport>,
}
impl Guardian {
    pub fn new(owner_uid: u32) -> Result<Self, &'static str> {
        if owner_uid == 0 {
            return Err("guardian_owner_root");
        }
        Ok(Self {
            owner_uid,
            report: None,
        })
    }
    pub fn update(&mut self, report: CheckReport) {
        self.report = Some(report);
    }
    pub fn evaluate(
        &mut self,
        event: ProtectionEvent,
        now: u64,
        generation: &str,
    ) -> Option<StopRequest> {
        let _ = (event, now, generation);
        // The legacy bodyguard only acts on an observed proxy or TUN failure.
        let failed = self.report.as_ref().is_some_and(|report| {
            report.checks.iter().any(|row| {
                matches!(
                    row.id,
                    switcher_domain::checks::CheckId::ProxyProcess
                        | switcher_domain::checks::CheckId::TunRoute
                ) && row.status == switcher_domain::checks::CheckStatus::Fail
            })
        });
        failed.then_some(StopRequest {
            owner_uid: self.owner_uid,
            reason: "proxy_or_tun_offline",
        })
    }
}
/// Concrete independent stop consumer. Never accepts a caller-supplied identity boolean.
/// Production callers must be the user-session guardian, not the root helper.
pub fn stop_verified_claude(request: &StopRequest) -> Result<usize, &'static str> {
    if unsafe { libc::geteuid() } != request.owner_uid || request.owner_uid == 0 {
        return Err("stop_owner_uid");
    }
    let mut inventory = std::process::Command::new("/bin/ps");
    inventory
        .args(["-axo", "pid=,command="])
        .env_clear()
        .env("LC_ALL", "C");
    let output = bounded_inventory(&mut inventory, Duration::from_secs(1))?;
    if !output.status.success() {
        return Err("process_inventory");
    }
    signal_legacy_claude(&String::from_utf8_lossy(&output.stdout), |pid| {
        if unsafe { libc::kill(pid, libc::SIGTERM) } == 0
            || std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
        {
            Ok(())
        } else {
            Err("stop_signal_failed")
        }
    })
}

fn signal_legacy_claude(
    inventory: &str,
    mut terminate: impl FnMut(i32) -> Result<(), &'static str>,
) -> Result<usize, &'static str> {
    let mut stopped = 0;
    let mut failure = None;
    for line in inventory.lines() {
        let line = line.trim_start();
        let Some((pid, command)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let Some(pid) = pid.parse::<i32>().ok().filter(|pid| *pid > 1) else {
            continue;
        };
        if command.contains("/Applications/Claude.app/Contents/MacOS/Claude") {
            match terminate(pid) {
                Ok(()) => stopped += 1,
                Err(error) => failure = Some(error),
            }
        }
    }
    failure.map_or(Ok(stopped), Err)
}

/// Poll the exact inventory child with nonblocking stdout and a size/deadline budget.
/// After SIGKILL, OS reaping is still synchronous; the single stop slot supervises that boundary too.
fn bounded_inventory(
    command: &mut std::process::Command,
    budget: Duration,
) -> Result<std::process::Output, &'static str> {
    use std::{io::Read, os::fd::AsRawFd, process::Stdio};
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "process_inventory")?;
    let mut stdout = child.stdout.take().ok_or("process_inventory")?;
    let descriptor = stdout.as_raw_fd();
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(descriptor, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        let _ = child.kill();
        let _ = child.wait();
        return Err("process_inventory");
    }
    let deadline = std::time::Instant::now() + budget;
    let mut bytes = Vec::new();
    let mut eof = false;
    loop {
        let mut chunk = [0u8; 4096];
        let mut idle = false;
        match stdout.read(&mut chunk) {
            Ok(0) => {
                eof = true;
                idle = true;
            }
            Ok(length) => bytes.extend_from_slice(&chunk[..length]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                idle = true;
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("process_inventory");
            }
        }
        if bytes.len() > 1024 * 1024 || std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("process_inventory_budget");
        }
        match child.try_wait() {
            Ok(Some(status)) if eof => {
                return Ok(std::process::Output {
                    status,
                    stdout: bytes,
                    stderr: Vec::new(),
                });
            }
            Ok(_) => {}
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("process_inventory");
            }
        }
        if idle {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
#[cfg(target_os = "macos")]
fn verify_process_requirement(
    pid: i32,
    uid: u32,
    requirement: &str,
    path: Option<&str>,
) -> Result<(), &'static str> {
    use std::ffi::{CString, c_void};
    type Obj = *mut c_void;
    #[link(name = "proc")]
    unsafe extern "C" {
        fn proc_pidpath(pid: i32, buf: *mut c_void, size: u32) -> i32;
        fn proc_pidinfo(pid: i32, flavor: i32, arg: u64, buf: *mut c_void, size: i32) -> i32;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFNumberCreate(a: Obj, t: i32, v: *const c_void) -> Obj;
        fn CFStringCreateWithCString(a: Obj, s: *const i8, e: u32) -> Obj;
        fn CFDictionaryCreate(
            a: Obj,
            k: *const Obj,
            v: *const Obj,
            n: isize,
            kcb: Obj,
            vcb: Obj,
        ) -> Obj;
        fn CFRelease(o: Obj);
    }
    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        static kSecGuestAttributePid: Obj;
        fn SecCodeCopyGuestWithAttributes(
            host: Obj,
            attributes: Obj,
            flags: u32,
            code: *mut Obj,
        ) -> i32;
        fn SecRequirementCreateWithString(s: Obj, flags: u32, r: *mut Obj) -> i32;
        fn SecCodeCheckValidity(code: Obj, flags: u32, r: Obj) -> i32;
    }
    if pid <= 1 {
        return Err("process_pid");
    }
    // PROC_PIDTBSDINFO: fixed Darwin proc_bsdinfo prefix includes real/effective UID.
    let mut bsd = [0u8; 136];
    let len = unsafe { proc_pidinfo(pid, 3, 0, bsd.as_mut_ptr().cast(), bsd.len() as i32) };
    if len < 24 || u32::from_ne_bytes(bsd[20..24].try_into().unwrap()) != uid {
        return Err("process_owner");
    }
    if let Some(expected) = path {
        let mut buf = [0u8; 4096];
        let n = unsafe { proc_pidpath(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
        if n <= 0 || buf.split(|b| *b == 0).next() != Some(expected.as_bytes()) {
            return Err("process_path");
        }
    }
    unsafe {
        let number = CFNumberCreate(std::ptr::null_mut(), 3, (&pid as *const i32).cast());
        if number.is_null() {
            return Err("process_identity");
        }
        let attrs = CFDictionaryCreate(
            std::ptr::null_mut(),
            &kSecGuestAttributePid,
            &number,
            1,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        let mut code = std::ptr::null_mut();
        let status = SecCodeCopyGuestWithAttributes(std::ptr::null_mut(), attrs, 0, &mut code);
        CFRelease(attrs);
        CFRelease(number);
        if status != 0 || code.is_null() {
            return Err("process_identity");
        }
        let text = CString::new(requirement).map_err(|_| "process_requirement")?;
        let string = CFStringCreateWithCString(std::ptr::null_mut(), text.as_ptr(), 0x08000100);
        let mut req = std::ptr::null_mut();
        let made = SecRequirementCreateWithString(string, 0, &mut req);
        CFRelease(string);
        let valid = made == 0 && !req.is_null() && SecCodeCheckValidity(code, 0, req) == 0;
        if !req.is_null() {
            CFRelease(req);
        }
        CFRelease(code);
        if valid {
            Ok(())
        } else {
            Err("process_signature")
        }
    }
}
#[cfg(not(target_os = "macos"))]
fn verify_process_requirement(
    _: i32,
    _: u32,
    _: &str,
    _: Option<&str>,
) -> Result<(), &'static str> {
    Err("macos_required")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_and_wake_do_not_stop_without_failure() {
        let mut g = Guardian::new(501).unwrap();
        assert!(
            g.evaluate(ProtectionEvent::Timer, 1, "generation")
                .is_none()
        );
        assert!(g.evaluate(ProtectionEvent::Wake, 2, "generation").is_none());
        assert!(Guardian::new(0).is_err());
    }
    #[test]
    fn stop_is_independent_of_lifecycle_lock() {
        let base = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("guardian-lock-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _lock = crate::storage::LifecycleLock::try_acquire(&base).unwrap();
        let mut g = Guardian::new(501).unwrap();
        assert!(
            g.evaluate(ProtectionEvent::NetworkChanged, 1, "new")
                .is_none()
        );
        drop(_lock);
        std::fs::remove_dir_all(base).unwrap();
    }
}

/// Prepare the legacy root without weakening private v2 state.
pub fn prepare_state_directory(home: &Path) -> Result<std::path::PathBuf, &'static str> {
    let base = home.join(".claude-switcher");
    crate::storage::prepare_data_root(&base).map_err(|_| "guardian_state_directory")?;
    let directory = base.join("v2");
    crate::storage::create_private_directory(&directory).map_err(|_| "guardian_state_directory")?;
    Ok(directory)
}

/// A snapshot binds both policy values and the exact committed configuration file.
/// File identity also invalidates an A -> B -> A replacement between observations.
#[derive(Debug, Clone)]
pub(crate) struct PolicySnapshot {
    pub(crate) config: crate::config::CheckConfig,
    fingerprint: String,
}
impl PolicySnapshot {
    pub(crate) fn load(v2: &Path) -> Result<Self, &'static str> {
        use sha2::{Digest, Sha256};
        use std::{
            io::Read,
            os::unix::fs::{MetadataExt, OpenOptionsExt},
        };
        crate::storage::validate_no_symlinks(v2).map_err(|_| "guardian_configuration_directory")?;
        let directory = std::fs::metadata(v2).map_err(|_| "guardian_configuration_directory")?;
        if directory.uid() != unsafe { libc::geteuid() } || directory.mode() & 0o077 != 0 {
            return Err("guardian_configuration_directory");
        }
        if crate::regions::recovery_pending_v2(v2)? {
            return Err("guardian_repair_pending");
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(v2.join("config.json"))
            .map_err(|_| "guardian_configuration_missing")?;
        let metadata = file
            .metadata()
            .map_err(|_| "guardian_configuration_metadata")?;
        if !metadata.is_file()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
            || metadata.len() > 16 * 1024
        {
            return Err("guardian_configuration_permissions");
        }
        let mut bytes = Vec::new();
        (&file)
            .take(16 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "guardian_configuration_read")?;
        if bytes.len() > 16 * 1024 {
            return Err("guardian_configuration_size");
        }
        let config: crate::config::CheckConfig =
            serde_json::from_slice(&bytes).map_err(|_| "guardian_configuration_invalid")?;
        config.validate()?;
        if config.country_target.is_none()
            || config.timezone_target.is_none()
            || config
                .locale_target
                .as_ref()
                .is_none_or(|value| value.is_empty())
        {
            return Err("guardian_configuration_incomplete");
        }
        let identity = |m: &std::fs::Metadata| {
            (
                m.dev(),
                m.ino(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
            )
        };
        let after = file
            .metadata()
            .map_err(|_| "guardian_configuration_metadata")?;
        let named = std::fs::symlink_metadata(v2.join("config.json"))
            .map_err(|_| "guardian_configuration_missing")?;
        if identity(&metadata) != identity(&after)
            || identity(&metadata) != identity(&named)
            || named.file_type().is_symlink()
            || crate::regions::recovery_pending_v2(v2)?
        {
            return Err("guardian_configuration_changed");
        }
        let mut digest = Sha256::new();
        digest.update(format!("{:?}", identity(&metadata)).as_bytes());
        digest.update(&bytes);
        Ok(Self {
            config,
            fingerprint: digest
                .finalize()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        })
    }
}
fn validate_health_policy(path: &Path, health: &GuardianHealth) -> Result<(), &'static str> {
    if health.degraded {
        return Err("guardian_degraded");
    }
    let current = PolicySnapshot::load(path.parent().ok_or("guardian_health_path")?)?;
    if health.policy_fingerprint != current.fingerprint
        || serde_json::to_vec(&health.policy).map_err(|_| "guardian_policy_schema")?
            != serde_json::to_vec(&current.config).map_err(|_| "guardian_policy_schema")?
    {
        return Err("guardian_policy_changed");
    }
    Ok(())
}

/// Runs the guardian in its owner session. Configuration always comes from fixed v2/config.json.
/// Stop work runs independently from checks and the lifecycle writer lock.
pub async fn run(
    config: crate::config::CheckConfig,
    state_path: &Path,
    events: tokio::sync::mpsc::Receiver<ProtectionEvent>,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), &'static str> {
    let timezone_guard = crate::timezone_guard::spawn(state_path, shutdown.clone())?;
    let mut installation_probe = crate::installation_health::GuardianProbe::default();
    let result = run_driver(
        (state_path, config),
        events,
        shutdown,
        DriverTiming::PRODUCTION,
        observe_legacy_guardian,
        stop_verified_claude,
        (
            || async {
                tokio::time::timeout(Duration::from_secs(1), crate::macos::network_generation())
                    .await
                    .ok()
                    .and_then(Result::ok)
                    .unwrap_or_default()
            },
            move |sequence| installation_probe.tick(sequence),
        ),
    )
    .await;
    timezone_guard.abort();
    result
}
#[derive(Clone, Copy)]
struct DriverTiming {
    tick: Duration,
    stop_timeout: Duration,
    retry: Duration,
}
impl DriverTiming {
    const PRODUCTION: Self = Self {
        tick: FALLBACK_INTERVAL,
        stop_timeout: Duration::from_secs(3),
        retry: FALLBACK_INTERVAL,
    };
    #[cfg(test)]
    fn testing(tick: Duration) -> Self {
        Self {
            tick,
            stop_timeout: Duration::from_millis(60),
            retry: Duration::from_millis(40),
        }
    }
}
type StopConsumer = dyn Fn(&StopRequest) -> Result<usize, &'static str> + Send + Sync;
struct StopJob {
    epoch: u64,
    request: StopRequest,
    deadline: tokio::time::Instant,
    timed_out: bool,
    handle: tokio::task::JoinHandle<Result<usize, &'static str>>,
}
struct StopCompletion {
    epoch: u64,
    request: StopRequest,
    timed_out: bool,
    result: Result<usize, &'static str>,
}
/// Exactly one blocking execution slot. Timeout revokes health; it does not cancel an OS thread.
/// A timed-out synchronous Security call keeps this slot occupied until it actually returns.
struct StopSupervisor {
    consumer: std::sync::Arc<StopConsumer>,
    job: Option<StopJob>,
    pending: Option<(u64, StopRequest)>,
    confirmed_epoch: Option<u64>,
    failed: bool,
    timeout: Duration,
    retry: Duration,
    retry_at: tokio::time::Instant,
}
impl StopSupervisor {
    fn new(
        consumer: impl Fn(&StopRequest) -> Result<usize, &'static str> + Send + Sync + 'static,
        timeout: Duration,
        retry: Duration,
    ) -> Self {
        Self {
            consumer: std::sync::Arc::new(consumer),
            job: None,
            pending: None,
            confirmed_epoch: None,
            failed: false,
            timeout,
            retry,
            retry_at: tokio::time::Instant::now(),
        }
    }
    fn request(&mut self, epoch: u64, request: StopRequest) {
        self.confirmed_epoch = None;
        if self.job.as_ref().is_some_and(|job| job.epoch == epoch) {
            return;
        }
        self.pending = Some((epoch, request));
    }
    fn start_ready(&mut self) {
        if self.job.is_some() || tokio::time::Instant::now() < self.retry_at {
            return;
        }
        if let Some((epoch, request)) = self.pending.take() {
            let consumer = self.consumer.clone();
            let operation = request.clone();
            self.job = Some(StopJob {
                epoch,
                request,
                deadline: tokio::time::Instant::now() + self.timeout,
                timed_out: false,
                handle: tokio::task::spawn_blocking(move || consumer(&operation)),
            });
        }
    }
    async fn join(&mut self) -> StopCompletion {
        let result = (&mut self.job.as_mut().expect("active stop slot").handle)
            .await
            .unwrap_or(Err("guardian_stop_worker_join"));
        let job = self.job.take().expect("completed stop slot");
        StopCompletion {
            epoch: job.epoch,
            request: job.request,
            timed_out: job.timed_out || tokio::time::Instant::now() >= job.deadline,
            result,
        }
    }
    fn finish(&mut self, completion: StopCompletion, current_epoch: u64) -> bool {
        if !completion.timed_out
            && completion.result.is_ok()
            && completion.epoch == current_epoch
            && self.pending.is_none()
        {
            self.failed = false;
            self.confirmed_epoch = Some(current_epoch);
            true
        } else {
            self.failed = true;
            self.confirmed_epoch = None;
            // A late success from a timed-out or old epoch cannot acknowledge a newer request.
            if self.pending.is_none() && completion.epoch == current_epoch {
                self.pending = Some((current_epoch, completion.request));
            }
            self.retry_at = tokio::time::Instant::now() + self.retry;
            false
        }
    }
}
async fn run_driver<C, CF, S, G, GF, H>(
    state: (&Path, crate::config::CheckConfig),
    mut events: tokio::sync::mpsc::Receiver<ProtectionEvent>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
    timing: DriverTiming,
    check: C,
    stop: S,
    hooks: (G, H),
) -> Result<(), &'static str>
where
    C: Fn(crate::config::CheckConfig) -> CF,
    CF: std::future::Future<Output = CheckReport> + Send + 'static,
    S: Fn(&StopRequest) -> Result<usize, &'static str> + Send + Sync + 'static,
    G: Fn() -> GF,
    GF: std::future::Future<Output = String>,
    H: FnMut(u64),
{
    let (state_path, mut config) = state;
    let (_, mut installation_tick) = hooks;
    let uid = unsafe { libc::geteuid() };
    let v2 = state_path.parent().ok_or("guardian_state_directory")?;
    let mut guardian = Guardian::new(uid)?;
    let mut ticker = tokio::time::interval(timing.tick);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut checks = tokio::task::JoinSet::new();
    let mut stop = StopSupervisor::new(stop, timing.stop_timeout, timing.retry);
    let started = crate::checks::unix_now();
    let mut sequence = 0;
    let mut events_open = true;
    let mut fingerprint = String::new();
    #[cfg(all(target_os = "macos", not(test)))]
    let mut update_checked = 0;
    loop {
        let stop_running = stop.job.is_some();
        let event = tokio::select! {
            result = shutdown.changed() => {
                if result.is_err() || *shutdown.borrow() { checks.abort_all(); return Ok(()); }
                continue;
            },
            report = checks.join_next(), if !checks.is_empty() => {
                if let Some(Ok(report)) = report {
                    guardian.update(report);
                    if let Some(request) = guardian.evaluate(ProtectionEvent::Timer, 0, "") {
                        stop.request(0, request);
                    } else {
                        stop.pending = None;
                    }
                }
                false
            },
            result = stop.join(), if stop_running => {
                stop.finish(result, 0);
                if guardian.evaluate(ProtectionEvent::Timer, 0, "").is_none() { stop.pending = None; }
                false
            },
            event = events.recv(), if events_open => {
                if event.is_none() { events_open = false; continue; }
                true
            },
            _ = ticker.tick() => {
                sequence += 1;
                installation_tick(sequence);
                #[cfg(all(target_os = "macos", not(test)))]
                {
                    let now = crate::checks::unix_now();
                    if now.saturating_sub(update_checked) >= 60 {
                        update_checked = now;
                        tokio::task::spawn_blocking(spawn_public_update_check);
                    }
                }
                true
            },
        };
        if event && checks.is_empty() {
            // Keep the previous proxy mode if configuration is temporarily unavailable.
            if let Some(home) = v2.parent().and_then(Path::parent)
                && (home.join(".claude-switcher/v2/config.json").exists()
                    || home.join(".local/bin/claude-switch.sh").exists())
                && let Ok(current) = crate::config::load_read_only(home)
            {
                config = current;
                // The snapshot is optional heartbeat diagnostics, never a refresh gate.
                fingerprint = PolicySnapshot::load(v2)
                    .map(|snapshot| snapshot.fingerprint)
                    .unwrap_or_default();
            }
            let future = check(config.clone());
            checks.spawn(future);
        }
        stop.start_ready();
        let health = GuardianHealth {
            owner_uid: uid,
            pid: std::process::id() as i32,
            heartbeat_at: crate::checks::unix_now(),
            started_at: started,
            policy: config.clone(),
            policy_fingerprint: fingerprint.clone(),
            degraded: stop.failed,
        };
        // Heartbeats are diagnostics. Snapshot contention cannot disable the bodyguard.
        if let Ok(_lock) = crate::storage::GuardianSnapshotLock::try_acquire(v2, true) {
            let _ = crate::storage::atomic_json(state_path, &health);
        }
    }
}

/// Legacy checks are local only: fake-IP TUN plus the configured proxy process/VPN.
async fn observe_legacy_guardian(config: crate::config::CheckConfig) -> CheckReport {
    use switcher_domain::checks::*;
    let tun = crate::macos::read_command("/sbin/ifconfig", &[])
        .await
        .is_ok_and(|text| {
            text.lines().any(|line| {
                line.split_whitespace()
                    .collect::<Vec<_>>()
                    .windows(2)
                    .any(|pair| pair[0] == "inet" && pair[1].starts_with("198.18."))
            })
        });
    let running = |pattern: &'static str, exact: bool| async move {
        crate::macos::read_command(
            "/usr/bin/pgrep",
            &[if exact { "-x" } else { "-f" }, pattern],
        )
        .await
        .is_ok()
    };
    let proxy = match config.proxy_mode.as_str() {
        "auto" => true,
        "shadowrocket" => {
            let text = crate::macos::read_command("/usr/sbin/scutil", &["--nc", "list"])
                .await
                .unwrap_or_default();
            let rows: Vec<_> = text
                .lines()
                .filter(|line| line.contains("com.liguangming.Shadowrocket"))
                .collect();
            rows.is_empty() || rows.iter().any(|line| line.contains("(Connected)"))
        }
        "sing-box" => running("sing-box", false).await,
        "clashx" => running("ClashX", true).await || running("clash", false).await,
        "surge" => running("Surge", false).await,
        _ => running("verge-mihomo", false).await,
    };
    let now = crate::checks::unix_now();
    let checks: Vec<_> = [(CheckId::ProxyProcess, proxy), (CheckId::TunRoute, tun)]
        .into_iter()
        .map(|(id, ok)| CheckResult {
            id,
            status: if ok {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            reason: "legacy bodyguard local observation".into(),
            observed_at: now,
            duration_ms: 0,
            valid_until: now + 10,
            network_generation: String::new(),
            evidence: vec![],
        })
        .collect();
    CheckReport {
        schema_version: 1,
        observed_at: now,
        completed_at: now,
        duration_ms: 0,
        network_generation: String::new(),
        summary: crate::checks::summarize(&checks),
        checks,
        launch: LaunchDecision {
            allowed: proxy && tun,
            reasons: vec![],
        },
    }
}

/// Invalidate before attempting the bounded notification queue. Overflow may
/// coalesce guardian work, but it must never preserve old launch evidence.
#[cfg(any(target_os = "macos", test))]
fn publish_native_event(
    sender: &tokio::sync::mpsc::Sender<ProtectionEvent>,
    event: ProtectionEvent,
    invalidate: impl FnOnce(),
) {
    invalidate();
    let _ = sender.try_send(event);
}

#[cfg(target_os = "macos")]
pub struct NativeEvents {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
#[cfg(target_os = "macos")]
impl NativeEvents {
    pub(crate) fn is_running(&self) -> bool {
        self.thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
    }
}
#[cfg(target_os = "macos")]
impl Drop for NativeEvents {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
#[cfg(target_os = "macos")]
pub fn native_events()
-> Result<(NativeEvents, tokio::sync::mpsc::Receiver<ProtectionEvent>), &'static str> {
    use std::ffi::c_void;
    type Obj = *mut c_void;
    #[repr(C)]
    struct StoreContext {
        version: isize,
        info: Obj,
        retain: Obj,
        release: Obj,
        copy_description: Obj,
    }
    struct PowerContext {
        sender: tokio::sync::mpsc::Sender<ProtectionEvent>,
        port: u32,
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFRunLoopDefaultMode: Obj;
        fn CFRunLoopGetCurrent() -> Obj;
        fn CFRunLoopAddSource(runloop: Obj, source: Obj, mode: Obj);
        fn CFRunLoopRemoveSource(runloop: Obj, source: Obj, mode: Obj);
        fn CFRunLoopRunInMode(mode: Obj, seconds: f64, return_after_source: bool) -> i32;
        fn CFStringCreateWithCString(a: Obj, s: *const i8, e: u32) -> Obj;
        fn CFArrayCreate(a: Obj, values: *const Obj, count: isize, callbacks: Obj) -> Obj;
        fn CFRelease(o: Obj);
    }
    #[link(name = "SystemConfiguration", kind = "framework")]
    unsafe extern "C" {
        fn SCDynamicStoreCreate(
            a: Obj,
            name: Obj,
            callback: unsafe extern "C" fn(Obj, Obj, Obj),
            context: *mut StoreContext,
        ) -> Obj;
        fn SCDynamicStoreSetNotificationKeys(store: Obj, keys: Obj, patterns: Obj) -> bool;
        fn SCDynamicStoreCreateRunLoopSource(a: Obj, store: Obj, order: isize) -> Obj;
    }
    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IORegisterForSystemPower(
            context: Obj,
            notification_port: *mut Obj,
            callback: unsafe extern "C" fn(Obj, u32, u32, Obj),
            notifier: *mut u32,
        ) -> u32;
        fn IONotificationPortGetRunLoopSource(port: Obj) -> Obj;
        fn IODeregisterForSystemPower(notifier: *mut u32) -> i32;
        fn IONotificationPortDestroy(port: Obj);
        fn IOServiceClose(port: u32) -> i32;
        fn IOAllowPowerChange(port: u32, id: isize) -> i32;
    }
    unsafe extern "C" fn changed(_: Obj, _: Obj, context: Obj) {
        let sender = unsafe { &*context.cast::<tokio::sync::mpsc::Sender<ProtectionEvent>>() };
        publish_native_event(
            sender,
            ProtectionEvent::NetworkChanged,
            crate::macos::invalidate_network_generation,
        );
    }
    unsafe extern "C" fn power(context: Obj, _service: u32, kind: u32, argument: Obj) {
        let context = unsafe { &*context.cast::<PowerContext>() };
        if kind == 0xe0000300 {
            publish_native_event(
                &context.sender,
                ProtectionEvent::Wake,
                crate::macos::invalidate_network_generation,
            );
        } else if kind == 0xe0000270 || kind == 0xe0000280 {
            unsafe { IOAllowPowerChange(context.port, argument as isize) };
        }
    }
    let (tx, rx) = tokio::sync::mpsc::channel(16);
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let thread_stop = stop.clone();
    let thread = std::thread::Builder::new()
        .name("switcher-network-observer".into())
        .spawn(move || unsafe {
            let null = std::ptr::null_mut();
            let name =
                CFStringCreateWithCString(null, c"ClaudeSwitcherGuardian".as_ptr(), 0x08000100);
            let mut context = StoreContext {
                version: 0,
                info: (&tx as *const tokio::sync::mpsc::Sender<ProtectionEvent>)
                    .cast_mut()
                    .cast(),
                retain: null,
                release: null,
                copy_description: null,
            };
            let store = SCDynamicStoreCreate(null, name, changed, &mut context);
            CFRelease(name);
            if store.is_null() {
                let _ = ready_tx.send(Err("network_events_unavailable"));
                return;
            }
            let keys = [
                c"State:/Network/Global/IPv4",
                c"State:/Network/Global/IPv6",
                c"State:/Network/Global/DNS",
            ]
            .map(|s| CFStringCreateWithCString(null, s.as_ptr(), 0x08000100));
            let array = CFArrayCreate(null, keys.as_ptr(), keys.len() as isize, null);
            // Global route/resolver keys alone miss interface and per-service changes.
            let patterns = [
                c"State:/Network/Interface/.*/.*",
                c"State:/Network/Service/.*/.*",
            ]
            .map(|s| CFStringCreateWithCString(null, s.as_ptr(), 0x08000100));
            let pattern_array =
                CFArrayCreate(null, patterns.as_ptr(), patterns.len() as isize, null);
            let configured = SCDynamicStoreSetNotificationKeys(store, array, pattern_array);
            CFRelease(pattern_array);
            for pattern in patterns {
                CFRelease(pattern);
            }
            CFRelease(array);
            for key in keys {
                CFRelease(key);
            }
            let source = SCDynamicStoreCreateRunLoopSource(null, store, 0);
            if !configured || source.is_null() {
                if !source.is_null() {
                    CFRelease(source);
                }
                CFRelease(store);
                let _ = ready_tx.send(Err("network_events_unavailable"));
                return;
            }
            let mut notification_port: Obj = std::ptr::null_mut();
            let mut notifier = 0;
            let mut context = Box::new(PowerContext {
                sender: tx.clone(),
                port: 0,
            });
            context.port = IORegisterForSystemPower(
                (&mut *context as *mut PowerContext).cast(),
                &mut notification_port,
                power,
                &mut notifier,
            );
            if context.port == 0 || notification_port.is_null() {
                if notifier != 0 {
                    IODeregisterForSystemPower(&mut notifier);
                }
                if context.port != 0 {
                    IOServiceClose(context.port);
                }
                if !notification_port.is_null() {
                    IONotificationPortDestroy(notification_port);
                }
                CFRelease(source);
                CFRelease(store);
                let _ = ready_tx.send(Err("power_events_unavailable"));
                return;
            }
            let runloop = CFRunLoopGetCurrent();
            let power_source = IONotificationPortGetRunLoopSource(notification_port);
            if power_source.is_null() {
                IODeregisterForSystemPower(&mut notifier);
                IOServiceClose(context.port);
                IONotificationPortDestroy(notification_port);
                CFRelease(source);
                CFRelease(store);
                let _ = ready_tx.send(Err("power_events_unavailable"));
                return;
            }
            CFRunLoopAddSource(runloop, source, kCFRunLoopDefaultMode);
            CFRunLoopAddSource(runloop, power_source, kCFRunLoopDefaultMode);
            let _ = ready_tx.send(Ok(()));
            while !thread_stop.load(std::sync::atomic::Ordering::Acquire) && !tx.is_closed() {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.25, true);
            }
            CFRunLoopRemoveSource(runloop, source, kCFRunLoopDefaultMode);
            CFRunLoopRemoveSource(runloop, power_source, kCFRunLoopDefaultMode);
            IODeregisterForSystemPower(&mut notifier);
            IOServiceClose(context.port);
            IONotificationPortDestroy(notification_port);
            CFRelease(source);
            CFRelease(store);
        })
        .map_err(|_| "native_events_thread_unavailable")?;
    let guard = NativeEvents {
        stop,
        thread: Some(thread),
    };
    ready_rx
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| "native_events_timeout")??;
    Ok((guard, rx))
}
#[cfg(target_os = "macos")]
pub async fn run_native(
    config: crate::config::CheckConfig,
    state_path: &Path,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<(), &'static str> {
    match native_events() {
        Ok((_native, events)) => run(config, state_path, events, shutdown).await,
        Err(_) => {
            // Native notifications are optional accelerators for the ten-second poll.
            let (_sender, events) = tokio::sync::mpsc::channel(1);
            run(config, state_path, events, shutdown).await
        }
    }
}
#[cfg(all(test, target_os = "macos"))]
mod native_tests {
    #[test]
    fn network_epoch_native_queue_overflow_still_invalidates_every_event() {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        let revision = std::sync::atomic::AtomicU64::new(0);
        for event in [
            super::ProtectionEvent::NetworkChanged,
            super::ProtectionEvent::NetworkChanged,
            super::ProtectionEvent::Wake,
        ] {
            super::publish_native_event(&sender, event, || {
                revision.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            });
        }
        assert_eq!(revision.load(std::sync::atomic::Ordering::SeqCst), 3);
        assert_eq!(receiver.len(), 1);
        assert!(!receiver.is_closed());
    }
    #[test]
    fn network_epoch_native_receiver_keeps_observer_alive_and_drop_is_detectable() {
        let (guard, receiver) = super::native_events().unwrap();
        assert!(guard.is_running());
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(guard.is_running());
        drop(receiver);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while guard.is_running() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!guard.is_running());
    }
    #[test]
    fn read_only_native_event_registration_and_cleanup() {
        let (guard, receiver) =
            super::native_events().expect("native read-only observer registration");
        drop(receiver);
        drop(guard);
    }
}

#[cfg(test)]
fn public_update_signature_args(app: &Path, team: &str) -> Vec<std::ffi::OsString> {
    let requirement = format!(
        "=anchor apple generic and identifier \"local.claudeswitcher.public\" and certificate leaf[subject.OU] = \"{team}\""
    );
    ["--verify", "--strict", "--test-requirement", &requirement]
        .into_iter()
        .map(std::ffi::OsString::from)
        .chain(std::iter::once(app.as_os_str().to_owned()))
        .collect()
}
#[cfg(test)]
#[test]
fn public_update_codesign_inline_requirement_argv() {
    let app = Path::new("/Applications/Claude 切换器 Public.app");
    assert_eq!(public_update_signature_args(app, "ABCDEFGHIJ"), vec![
        std::ffi::OsString::from("--verify"),
        "--strict".into(), "--test-requirement".into(),
        "=anchor apple generic and identifier \"local.claudeswitcher.public\" and certificate leaf[subject.OU] = \"ABCDEFGHIJ\"".into(),
        app.as_os_str().to_owned(),
    ]);
}

#[cfg(all(target_os = "macos", not(test)))]
fn spawn_public_update_check() -> Result<(), &'static str> {
    let record = read_install_record()?;
    if record.owner_uid != unsafe { libc::geteuid() } {
        return Err("update_owner_uid");
    }
    if record.edition != switcher_domain::Edition::Public {
        return Ok(());
    }
    record.developer_team()?;
    let app = Path::new("/Applications/Claude 切换器 Public.app");
    let status = std::process::Command::new("/usr/bin/codesign")
        .args([
            "--verify",
            "--strict",
            "--test-requirement",
            &format!(
                "={}",
                record.client_requirement(switcher_domain::system::ClientRole::Gui)?
            ),
        ])
        .arg(app)
        .status()
        .map_err(|_| "update_app_signature_unavailable")?;
    if !status.success() {
        return Err("update_app_signature");
    }
    if read_install_record()? != record {
        return Err("update_install_identity_changed");
    }
    std::process::Command::new(app.join("Contents/MacOS/switcher-desktop"))
        .args(["--update-task", "check"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| "update_task_spawn")?;
    Ok(())
}
#[cfg(test)]
mod health_tests {
    use super::*;
    #[test]
    fn fifo_health_is_rejected_without_blocking() {
        let directory = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("guardian-fifo-{}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("health.json");
        let raw = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(raw.as_ptr(), 0o600) }, 0);
        let start = std::time::Instant::now();
        assert_eq!(
            read_health(&path, unsafe { libc::geteuid() }, 100).unwrap_err(),
            "guardian_permissions"
        );
        assert!(start.elapsed() < Duration::from_secs(1));
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}

#[cfg(test)]
mod reload_tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    #[test]
    fn legacy_0755_root_keeps_permissions_and_private_v2() {
        let home = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("guardian-legacy-{}", std::process::id()));
        std::fs::create_dir_all(home.join(".claude-switcher")).unwrap();
        let root = home.join(".claude-switcher");
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        let result = prepare_state_directory(&home);
        let result = result.map(|v2| {
            assert_eq!(std::fs::metadata(&root).unwrap().mode() & 0o777, 0o755);
            assert_eq!(std::fs::metadata(v2).unwrap().mode() & 0o777, 0o700);
        });
        std::fs::remove_dir_all(home).unwrap();
        assert!(
            result.is_ok(),
            "legacy 0755 root must remain supported: {result:?}"
        );
    }
}

#[cfg(test)]
mod legacy_behavior_tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use switcher_domain::checks::*;

    fn report(failed: CheckId) -> CheckReport {
        let checks = CheckId::ALL
            .into_iter()
            .map(|id| CheckResult {
                id,
                status: if id == failed {
                    CheckStatus::Fail
                } else {
                    CheckStatus::Pass
                },
                reason: "injected".into(),
                observed_at: 0,
                duration_ms: 0,
                valid_until: 0,
                network_generation: "old-network".into(),
                evidence: vec![],
            })
            .collect::<Vec<_>>();
        CheckReport {
            schema_version: 1,
            observed_at: 0,
            completed_at: 0,
            duration_ms: 0,
            network_generation: "old-network".into(),
            summary: crate::checks::summarize(&checks),
            checks,
            launch: LaunchDecision {
                allowed: false,
                reasons: vec!["full launch gate fails".into()],
            },
        }
    }
    #[test]
    fn only_proxy_or_tun_failure_requests_stop_for_every_event() {
        assert_eq!(FALLBACK_INTERVAL, Duration::from_secs(10));
        let mut guardian = Guardian::new(501).unwrap();
        for failed in CheckId::ALL {
            guardian.update(report(failed));
            for event in [
                ProtectionEvent::Timer,
                ProtectionEvent::Wake,
                ProtectionEvent::NetworkChanged,
            ] {
                assert_eq!(
                    guardian.evaluate(event, u64::MAX, "changed").is_some(),
                    matches!(failed, CheckId::ProxyProcess | CheckId::TunRoute)
                );
            }
        }
    }
    #[test]
    fn legacy_path_scope_sends_term_once_per_matching_process_without_signature_gate() {
        let mut signalled = Vec::new();
        let count = signal_legacy_claude("11 /Applications/Claude.app/Contents/MacOS/Claude\n12 /Applications/Claude.app/Contents/MacOS/Claude --type=renderer\n13 /Applications/Other.app/Contents/MacOS/Claude\n14 claude\n", |pid| { signalled.push(pid); Ok(()) }).unwrap();
        assert_eq!(count, 2);
        assert_eq!(signalled, [11, 12]);
    }
    #[tokio::test]
    async fn legacy_shell_only_configuration_loads_at_startup_and_refreshes() {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let home = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "legacy-guardian-config-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let directory = home.join(".claude-switcher/v2");
        std::fs::create_dir_all(home.join(".local/bin")).unwrap();
        crate::storage::create_private_directory(&home.join(".claude-switcher")).unwrap();
        crate::storage::create_private_directory(&directory).unwrap();
        let script = home.join(".local/bin/claude-switch.sh");
        std::fs::write(&script, "PROXY_MODE=\"clash-verge\"\n").unwrap();
        let config = crate::config::load_read_only(&home).unwrap();
        assert_eq!(config.proxy_mode, "clash-verge");
        assert!(!directory.join("config.json").exists());
        let (observed_send, mut observed) = tokio::sync::mpsc::unbounded_channel();
        let (event_send, events) = tokio::sync::mpsc::channel(2);
        let (shutdown_send, shutdown) = tokio::sync::watch::channel(false);
        let state_path = directory.join("guardian-health.json");
        let task = tokio::spawn(async move {
            run_driver(
                (&state_path, config),
                events,
                shutdown,
                DriverTiming::testing(Duration::from_secs(60)),
                move |config| {
                    observed_send.send(config.proxy_mode).unwrap();
                    async { report(CheckId::Timezone) }
                },
                |_| panic!("configuration changes cannot stop Claude"),
                (|| async { String::new() }, |_| {}),
            )
            .await
        });
        async fn next_mode(
            observed: &mut tokio::sync::mpsc::UnboundedReceiver<String>,
            wanted: &str,
        ) {
            tokio::time::timeout(Duration::from_secs(2), async {
                loop {
                    if observed
                        .recv()
                        .await
                        .expect("guardian check should complete")
                        == wanted
                    {
                        break;
                    }
                }
            })
            .await
            .unwrap();
        }
        next_mode(&mut observed, "clash-verge").await;
        // Wait for the injected check to complete before requesting a refresh.
        tokio::time::sleep(Duration::from_millis(20)).await;
        std::fs::write(&script, "PROXY_MODE=\"surge\"\n").unwrap();
        event_send.send(ProtectionEvent::Wake).await.unwrap();
        next_mode(&mut observed, "surge").await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        std::fs::write(&script, "PROXY_MODE=\"unknown-mode\"\n").unwrap();
        event_send.send(ProtectionEvent::Wake).await.unwrap();
        next_mode(&mut observed, "surge").await;
        tokio::time::sleep(Duration::from_millis(20)).await;
        std::fs::write(&script, "PROXY_MODE=\"auto\"\n").unwrap();
        event_send
            .send(ProtectionEvent::NetworkChanged)
            .await
            .unwrap();
        next_mode(&mut observed, "auto").await;
        shutdown_send.send(true).unwrap();
        task.await.unwrap().unwrap();
        std::fs::remove_dir_all(home).unwrap();
    }

    #[tokio::test]
    async fn driver_checks_before_stop_and_events_config_or_heartbeat_cannot_stop() {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "legacy-driver-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        crate::storage::create_private_directory(&root).unwrap();
        // No config, install identity or repair receipt exists in this fixture.
        let path = root.join("guardian-health.json");
        let checks = Arc::new(AtomicUsize::new(0));
        let stops = Arc::new(AtomicUsize::new(0));
        let check_count = checks.clone();
        let stop_count = stops.clone();
        let (send, receive) = tokio::sync::mpsc::channel(4);
        let (shutdown_send, shutdown) = tokio::sync::watch::channel(false);
        let task = tokio::spawn(async move {
            run_driver(
                (&path, crate::config::CheckConfig::default()),
                receive,
                shutdown,
                DriverTiming::testing(Duration::from_millis(20)),
                move |_| {
                    check_count.fetch_add(1, Ordering::Relaxed);
                    async { report(CheckId::Timezone) }
                },
                move |_| {
                    stop_count.fetch_add(1, Ordering::Relaxed);
                    Ok(1)
                },
                (
                    || async { panic!("guardian must not wait on network epoch") },
                    |_| {},
                ),
            )
            .await
        });
        send.send(ProtectionEvent::Wake).await.unwrap();
        send.send(ProtectionEvent::NetworkChanged).await.unwrap();
        drop(send);
        tokio::time::timeout(Duration::from_secs(2), async {
            while checks.load(Ordering::Relaxed) < 3 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        shutdown_send.send(true).unwrap();
        task.await.unwrap().unwrap();
        assert_eq!(stops.load(Ordering::Relaxed), 0);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod retained_diagnostic_tests {
    use super::*;
    use std::path::PathBuf;
    pub(super) struct Fixture(pub(super) PathBuf);
    impl Fixture {
        pub(super) fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "guardian-reload-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            crate::storage::create_private_directory(&path).unwrap();
            Self(path)
        }
        pub(super) fn save(&self, country: &str) {
            let config = crate::config::CheckConfig {
                country_target: Some(country.into()),
                timezone_target: Some(
                    if country == "JP" {
                        "Asia/Tokyo"
                    } else {
                        "America/New_York"
                    }
                    .into(),
                ),
                locale_target: Some(if country == "JP" { "ja_JP" } else { "en_US" }.into()),
                ..Default::default()
            };
            crate::storage::atomic_json(&self.0.join("config.json"), &config).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn health_binds_current_complete_policy_without_needing_green_report() {
        let f = Fixture::new();
        f.save("US");
        let current = PolicySnapshot::load(&f.0).unwrap();
        let mut health = GuardianHealth {
            owner_uid: unsafe { libc::geteuid() },
            pid: std::process::id() as i32,
            started_at: crate::checks::unix_now(),
            heartbeat_at: crate::checks::unix_now(),
            policy: current.config,
            policy_fingerprint: current.fingerprint,
            degraded: false,
        };
        let path = f.0.join("guardian-health.json");
        assert!(validate_health_policy(&path, &health).is_ok());
        health.degraded = true;
        assert_eq!(
            validate_health_policy(&path, &health),
            Err("guardian_degraded")
        );
        crate::storage::atomic_json(&path, &health).unwrap();
        assert_eq!(
            read_health(&path, health.owner_uid, health.heartbeat_at).unwrap_err(),
            "guardian_degraded"
        );
        health.degraded = false;
        crate::storage::atomic_json(&path, &health).unwrap();
        f.save("JP");
        assert_eq!(
            read_health(&path, health.owner_uid, health.heartbeat_at).unwrap_err(),
            "guardian_policy_changed"
        );
        assert_eq!(
            validate_health_policy(&path, &health),
            Err("guardian_policy_changed")
        );
        f.save("US");
        assert_eq!(
            validate_health_policy(&path, &health),
            Err("guardian_policy_changed")
        );
        std::fs::remove_file(f.0.join("config.json")).unwrap();
        assert!(validate_health_policy(&path, &health).is_err());
    }
    #[test]
    fn unsafe_legacy_roots_and_nonprivate_v2_are_rejected() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let f = Fixture::new();
        let root = f.0.join(".claude-switcher");
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(prepare_state_directory(&f.0).is_err());
        std::fs::remove_dir(&root).unwrap();
        symlink(&f.0, &root).unwrap();
        assert!(prepare_state_directory(&f.0).is_err());
        std::fs::remove_file(&root).unwrap();
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::create_dir(root.join("v2")).unwrap();
        std::fs::set_permissions(root.join("v2"), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(prepare_state_directory(&f.0).is_err());
    }
    #[test]
    fn inventory_child_is_deadline_limited_and_output_is_collected() {
        let mut echo = std::process::Command::new("/bin/echo");
        echo.arg("123");
        assert_eq!(
            bounded_inventory(&mut echo, Duration::from_secs(1))
                .unwrap()
                .stdout,
            b"123\n"
        );
        let mut sleeping = std::process::Command::new("/bin/sleep");
        sleeping.arg("2");
        let started = std::time::Instant::now();
        assert_eq!(
            bounded_inventory(&mut sleeping, Duration::from_millis(30)).unwrap_err(),
            "process_inventory_budget"
        );
        assert!(started.elapsed() < Duration::from_millis(500));
    }
}
