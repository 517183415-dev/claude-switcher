//! Fixed native operations used by the original repair flow.
use super::*;
use std::process::{Command, Stdio};

trait Operations {
    fn command(&mut self, program: &str, args: &[&str]) -> Result<String, &'static str>;
    fn administrator(&mut self, commands: &[String]) -> Result<(), &'static str>;
    fn proxy(&mut self, config: &CheckConfig) -> Result<(), &'static str>;
    fn preferences(&mut self, locale: UserLocale, region_only: bool) -> Vec<String>;
    fn merge(&mut self, mode: &str) -> Result<(), &'static str>;
    fn timezone(&mut self) -> Option<String>;
    fn guardian_commands(&mut self) -> (Vec<String>, bool);
    fn watchdog_command(&mut self, timezone: &str) -> Option<String>;
}
struct Native;
fn shell(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn script(commands: &[String]) -> String {
    // Each fixed item runs even if an earlier item fails. Preserve an aggregate failure.
    let body = commands
        .iter()
        .map(|command| format!("{command} || failed=1"))
        .collect::<Vec<_>>()
        .join("; ");
    let body = format!("failed=0; {body}; exit $failed");
    format!(
        "do shell script \"{}\" with prompt \"Claude 切换器需要修复系统设置\" with administrator privileges",
        body.replace('\\', "\\\\").replace('"', "\\\"")
    )
}
fn egress_client() -> Result<reqwest::Client, &'static str> {
    crate::network::inherited_client()
}
fn legacy_routes_ok() -> bool {
    if cfg!(test) {
        return false;
    }
    std::thread::spawn(|| {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return false;
        };
        runtime.block_on(async {
            for domain in ["claude.ai", "api.anthropic.com", "claudeusercontent.com"] {
                let Ok(addresses) = crate::macos::dns_addresses(domain).await else {
                    return false;
                };
                if !addresses.first().is_some_and(|ip| match ip {
                    std::net::IpAddr::V4(ip) => {
                        ip.octets()[0] == 198 && matches!(ip.octets()[1], 18 | 19)
                    }
                    _ => false,
                }) {
                    return false;
                }
            }
            true
        })
    })
    .join()
    .unwrap_or(false)
}
impl Operations for Native {
    fn timezone(&mut self) -> Option<String> {
        std::fs::read_link("/etc/localtime")
            .ok()
            .and_then(|path| path.to_str().map(str::to_owned))
    }

    fn command(&mut self, program: &str, args: &[&str]) -> Result<String, &'static str> {
        if cfg!(test) {
            return Err("native_repair_disabled_in_tests");
        }
        let output = Command::new(program)
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("LC_ALL", "C")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|_| "修复命令无法执行")?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err("修复命令未完成")
        }
    }
    fn administrator(&mut self, commands: &[String]) -> Result<(), &'static str> {
        self.command("/usr/bin/osascript", &["-e", &script(commands)])
            .map(|_| ())
    }
    fn proxy(&mut self, config: &CheckConfig) -> Result<(), &'static str> {
        let mode = ProxyRepairMode::parse(&config.proxy_mode).ok_or("代理类型不在支持列表")?;
        if let Some(plan) = proxy_repair::prepare_app_open(mode)? {
            let _ = proxy_repair::execute_app_open(&plan);
        }
        if config.proxy_mode == "clash-verge" {
            // Restore the original TUN flag before the final proxy/TUN/egress gate.
            let _ = crate::user_repairs::repair_legacy_tun();
            if !proxy_repair::Native.running().unwrap_or(false) {
                proxy_repair::Native.start()?;
            }
        }
        let mode = config.proxy_mode.clone();
        let target = config.country_target.clone().ok_or("目标国家缺失")?;
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|_| "代理检查不可用")?;
            runtime.block_on(async {
                let mut ready = false;
                for _ in 0..40 {
                    if crate::macos::proxy_running(&mode).await.unwrap_or(false)
                        && crate::macos::tunnel_route().await.unwrap_or(false)
                    {
                        ready = true;
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
                if !ready {
                    return Err("代理内核与 TUN 未就绪；未修改系统与用户偏好");
                }
                let client = egress_client()?;
                let geo = crate::network::lookup_egress(&client).await;
                if geo.ip.is_none() || geo.country.as_deref() != Some(target.as_str()) {
                    return Err("出口国家未确认或与目标不符；未修改系统与用户偏好");
                }
                Ok(())
            })
        })
        .join()
        .map_err(|_| "代理检查线程中断")?
    }
    fn preferences(&mut self, locale: UserLocale, region_only: bool) -> Vec<String> {
        let mut errors = Vec::new();
        if !region_only {
            errors.extend(crate::user_repairs::repair_legacy_preferences(None));
        }
        errors.extend(crate::user_repairs::repair_legacy_preferences(Some(locale)));
        errors
    }
    fn merge(&mut self, mode: &str) -> Result<(), &'static str> {
        if mode == "clash-verge"
            && crate::user_repairs::repair_legacy_merge(legacy_routes_ok())
                .map_err(|_| "置顶规则修复失败")?
        {
            proxy_repair::Native.stop()?;
            proxy_repair::Native.start()?;
            proxy_repair::Native.verify("")?;
        }
        Ok(())
    }
    fn guardian_commands(&mut self) -> (Vec<String>, bool) {
        crate::guardian_repair::legacy_admin_commands()
    }
    fn watchdog_command(&mut self, timezone: &str) -> Option<String> {
        crate::guardian_repair::legacy_watchdog_command(timezone)
    }
}

pub(super) fn execute(
    coordinator: &RegionCoordinator,
    prepared: PreparedRepair,
) -> Result<(), &'static str> {
    execute_with(coordinator, prepared, &mut Native)
}
fn execute_with(
    coordinator: &RegionCoordinator,
    prepared: PreparedRepair,
    io: &mut impl Operations,
) -> Result<(), &'static str> {
    let _lock = storage::LifecycleLock::try_acquire(&coordinator.home.join(".claude-switcher/v2"))
        .map_err(|_| "其他账号、修复或安装事务正在进行")?;
    if recovery_pending(&coordinator.home)? {
        return Err("旧修复事务尚未完成；请先使用恢复入口");
    }
    if encode(&config::load_read_only(&coordinator.home)?)? != prepared.original_config {
        return Err("地区配置已改变，请重新预览");
    }
    let region_only = prepared.legacy_region.ok_or("修复计划类型错误")?;
    let config = prepared.proposed_config;
    let timezone = config.timezone_target.as_deref().ok_or("目标时区缺失")?;
    let locale = UserLocale::parse(config.locale_target.as_deref().ok_or("目标地区格式缺失")?)
        .ok_or("目标地区格式无效")?;
    let mut failed = false;
    let mut guardian_failed = false;
    if region_only {
        // Preserve the original partial-completion contract when administrator access is cancelled.
        storage::atomic_json(
            &coordinator.home.join(".claude-switcher/v2/config.json"),
            &config,
        )
        .map_err(|_| "地区目标保存失败")?;
        failed |= !io.preferences(locale, true).is_empty();
    } else {
        io.proxy(&config)?;
    }
    let mut commands = Vec::new();
    let current_timezone = io.timezone();
    if current_timezone.as_deref() != Some(&format!("/var/db/timezone/zoneinfo/{timezone}")) {
        commands.push(format!(
            "/usr/sbin/systemsetup -settimezone {}",
            shell(timezone)
        ));
    }
    if region_only {
        if let Some(command) = io.watchdog_command(timezone) {
            commands.push(command);
        }
    } else {
        let ipv6 = io.command(
            "/usr/sbin/networksetup",
            &["-getinfo", &config.network_service],
        );
        if !ipv6.is_ok_and(|value| {
            value
                .lines()
                .any(|line| line.eq_ignore_ascii_case("IPv6: Off"))
        }) {
            commands.push(format!(
                "/usr/sbin/networksetup -setv6off {}",
                shell(&config.network_service)
            ));
        }
        let (guardian_commands, guardian_ready) = io.guardian_commands();
        commands.extend(guardian_commands);
        guardian_failed = !guardian_ready;
        failed |= guardian_failed;
        if !commands.is_empty() {
            commands.push("/usr/bin/dscacheutil -flushcache".into());
            commands.push("/usr/bin/killall -HUP mDNSResponder".into());
        }
    }
    if !commands.is_empty() {
        failed |= io.administrator(&commands).is_err();
    }
    if !region_only {
        failed |= !io.preferences(locale, false).is_empty();
        failed |= io.merge(&config.proxy_mode).is_err();
        // Persist the unchanged policy for Guardian, even when all system items were already correct.
        storage::atomic_json(
            &coordinator.home.join(".claude-switcher/v2/config.json"),
            &config,
        )
        .map_err(|_| "地区目标保存失败")?;
        failed |= io.proxy(&config).is_err();
    }
    if failed {
        Err(if region_only {
            "时区或地区格式未完成；国家、时区与地区格式目标已保存，可重新执行"
        } else if guardian_failed {
            "保镖服务未恢复；已继续执行其余修复，可重新修复"
        } else {
            "部分修复未完成；已继续可执行的用户项目，可重新修复"
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        calls: Vec<String>,
        timezone: String,
        ipv6_off: bool,
        guardian: bool,
        guardian_ready: bool,
        deny: bool,
        gate: bool,
    }
    impl Operations for Fake {
        fn command(&mut self, _: &str, _: &[&str]) -> Result<String, &'static str> {
            Ok(if self.ipv6_off {
                "IPv6: Off"
            } else {
                "IPv6: Automatic"
            }
            .into())
        }
        fn administrator(&mut self, commands: &[String]) -> Result<(), &'static str> {
            self.calls.push(format!("admin:{}", commands.join(";")));
            if self.deny { Err("cancelled") } else { Ok(()) }
        }
        fn proxy(&mut self, _: &CheckConfig) -> Result<(), &'static str> {
            self.calls.push("proxy".into());
            if self.gate { Ok(()) } else { Err("gate") }
        }
        fn preferences(&mut self, _: UserLocale, only: bool) -> Vec<String> {
            self.calls
                .push(if only { "locale" } else { "preferences" }.into());
            Vec::new()
        }
        fn merge(&mut self, _: &str) -> Result<(), &'static str> {
            self.calls.push("merge".into());
            Ok(())
        }
        fn timezone(&mut self) -> Option<String> {
            Some(format!("/var/db/timezone/zoneinfo/{}", self.timezone))
        }
        fn guardian_commands(&mut self) -> (Vec<String>, bool) {
            (
                if self.guardian {
                    vec!["/bin/launchctl bootstrap system '/Library/LaunchDaemons/local.claude-bodyguard.plist'".into()]
                } else {
                    Vec::new()
                },
                self.guardian_ready,
            )
        }
        fn watchdog_command(&mut self, _: &str) -> Option<String> {
            Some("watchdog".into())
        }
    }
    fn fixture() -> (PathBuf, RegionCoordinator, Fake) {
        let home = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "legacy-regions-{}-{}",
            std::process::id(),
            crate::checks::unix_now()
        ));
        // Each test has its own thread name to avoid sharing fixture state.
        let home = home.join(std::thread::current().name().unwrap_or("test"));
        fs::create_dir_all(home.join(".claude-switcher/v2")).unwrap();
        storage::atomic_json(
            &home.join(".claude-switcher/v2/config.json"),
            &CheckConfig {
                country_target: Some("US".into()),
                timezone_target: Some("America/Los_Angeles".into()),
                locale_target: Some("en_US".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let coordinator = RegionCoordinator {
            home: home.clone(),
            edition: Edition::Personal,
        };
        let fake = Fake {
            calls: Vec::new(),
            timezone: "America/Los_Angeles".into(),
            ipv6_off: true,
            guardian: false,
            guardian_ready: true,
            deny: false,
            gate: true,
        };
        (home, coordinator, fake)
    }
    #[test]
    fn user_only_repair_never_requests_administrator() {
        let (home, coordinator, mut fake) = fixture();
        let plan = coordinator.preview_legacy(RegionTarget::Current).unwrap();
        execute_with(&coordinator, plan, &mut fake).unwrap();
        assert_eq!(fake.calls, ["proxy", "preferences", "merge", "proxy"]);
        assert!(!recovery_pending(&home).unwrap());
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn administrator_cancel_continues_user_items_and_retry_has_no_journal() {
        let (home, coordinator, mut fake) = fixture();
        fake.ipv6_off = false;
        fake.deny = true;
        let plan = coordinator.preview_legacy(RegionTarget::Current).unwrap();
        assert!(execute_with(&coordinator, plan, &mut fake).is_err());
        assert_eq!(
            &fake.calls[fake.calls.len() - 3..],
            ["preferences", "merge", "proxy"]
        );
        assert!(!recovery_pending(&home).unwrap());
        fake.deny = false;
        execute_with(
            &coordinator,
            coordinator.preview_legacy(RegionTarget::Current).unwrap(),
            &mut fake,
        )
        .unwrap();
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn set_region_saves_target_and_locale_before_cancel_without_fix_expansion() {
        let (home, coordinator, mut fake) = fixture();
        fake.deny = true;
        let plan = coordinator
            .preview_legacy(RegionTarget::Selected(preset(RegionPreset::Jp)))
            .unwrap();
        assert!(execute_with(&coordinator, plan, &mut fake).is_err());
        assert_eq!(
            config::load_read_only(&home)
                .unwrap()
                .country_target
                .as_deref(),
            Some("JP")
        );
        assert_eq!(fake.calls.len(), 2);
        assert_eq!(fake.calls[0], "locale");
        let admin = &fake.calls[1];
        assert!(admin.contains("systemsetup"));
        assert!(admin.contains("watchdog"));
        for forbidden in ["networksetup", "dscacheutil", "mDNSResponder", "bootstrap"] {
            assert!(!admin.contains(forbidden));
        }
        assert!(!recovery_pending(&home).unwrap());
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn guardian_only_repair_includes_both_dns_commands() {
        let (home, coordinator, mut fake) = fixture();
        fake.guardian = true;
        execute_with(
            &coordinator,
            coordinator.preview_legacy(RegionTarget::Current).unwrap(),
            &mut fake,
        )
        .unwrap();
        let admin = fake
            .calls
            .iter()
            .find(|call| call.starts_with("admin:"))
            .unwrap();
        assert!(admin.contains("dscacheutil -flushcache"));
        assert!(admin.contains("killall -HUP mDNSResponder"));
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn failed_proxy_gate_blocks_system_and_preferences() {
        let (home, coordinator, mut fake) = fixture();
        fake.gate = false;
        assert!(
            execute_with(
                &coordinator,
                coordinator.preview_legacy(RegionTarget::Current).unwrap(),
                &mut fake
            )
            .is_err()
        );
        assert_eq!(fake.calls, ["proxy"]);
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn administrator_script_runs_every_fixed_item_and_retains_failure() {
        let value = script(&["one".into(), "two".into()]);
        assert!(value.contains("one || failed=1; two || failed=1; exit $failed"));
    }
    #[test]
    fn unavailable_guardian_continues_fixed_system_and_user_repairs() {
        let (home, coordinator, mut fake) = fixture();
        fake.guardian_ready = false;
        fake.guardian = true;
        let plan = coordinator.preview_legacy(RegionTarget::Current).unwrap();
        let error = execute_with(&coordinator, plan, &mut fake).unwrap_err();
        assert!(error.contains("保镖服务未恢复"));
        let admin = fake
            .calls
            .iter()
            .find(|call| call.starts_with("admin:"))
            .unwrap();
        assert!(admin.contains("bootstrap"));
        assert!(admin.contains("dscacheutil -flushcache"));
        assert!(admin.contains("killall -HUP mDNSResponder"));
        assert_eq!(
            &fake.calls[fake.calls.len() - 3..],
            ["preferences", "merge", "proxy"]
        );
        assert!(!recovery_pending(&home).unwrap());
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn fix_egress_proxy_fixture_child() {
        if std::env::var_os("SWITCHER_FIX_PROXY_FIXTURE_CHILD").is_none() {
            return;
        }
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let response = egress_client()
                .unwrap()
                .get("http://switcher-fix-fixture.invalid/")
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), 407);
        });
    }
    #[test]
    fn fix_egress_production_client_inherits_proxy_in_isolated_child() {
        use std::{
            io::{Read, Write},
            time::{Duration, Instant},
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let proxy = format!("http://{}", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(error) => panic!("fixture proxy was not contacted: {error}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0; 4096];
            let count = stream.read(&mut request).unwrap();
            assert!(
                String::from_utf8_lossy(&request[..count])
                    .starts_with("GET http://switcher-fix-fixture.invalid/")
            );
            stream
                .write_all(
                    b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\n\r\n",
                )
                .unwrap();
        });
        let result = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "regions::legacy::tests::fix_egress_proxy_fixture_child",
            ])
            .env_clear()
            .env("SWITCHER_FIX_PROXY_FIXTURE_CHILD", "1")
            .env("http_proxy", proxy)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        worker.join().unwrap();
    }
}
