//! Legacy proxy app opening and Clash Verge restart. This does not attest loaded Merge rules.
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) enum Adapter {
    ClashVerge,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) enum Phase {
    Intent,
    Stopped,
    StartingTarget,
    TargetReady,
    RestoringFiles,
    RestoringRuntime,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeState {
    pub adapter: Adapter,
    pub originally_running: bool,
    pub reload_required: bool,
    pub phase: Phase,
}

pub(crate) trait Runtime {
    fn running(&mut self) -> Result<bool, &'static str>;
    fn stop(&mut self) -> Result<(), &'static str>;
    fn start(&mut self) -> Result<(), &'static str>;
    /// Bounded process + TUN + country check; never an assertion about loaded rules.
    fn verify(&mut self, country: &str) -> Result<(), &'static str>;
}

pub(crate) struct Native;

pub(crate) fn reload_if_needed(
    needs_reload: bool,
    runtime: &mut impl Runtime,
) -> Result<(), &'static str> {
    if needs_reload {
        runtime.stop()?;
        runtime.start()?;
        runtime.verify("")?;
    }
    Ok(())
}

// Sync coordinator callers may already run in Tokio. A scoped worker avoids nested runtimes.
fn worker<T: Send>(
    task: impl FnOnce() -> Result<T, &'static str> + Send,
) -> Result<T, &'static str> {
    std::thread::scope(|scope| {
        scope
            .spawn(task)
            .join()
            .map_err(|_| "代理运行检查线程中断")?
    })
}
fn runtime() -> Result<tokio::runtime::Runtime, &'static str> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "代理运行检查不可用")
}
impl Runtime for Native {
    fn running(&mut self) -> Result<bool, &'static str> {
        native(|| runtime()?.block_on(crate::macos::proxy_running("clash-verge")))
    }
    fn stop(&mut self) -> Result<(), &'static str> {
        native(|| {
            runtime()?.block_on(async {
                // Legacy restart requests the app to quit, then gives it three seconds.
                let _ = crate::macos::read_command(
                    "/usr/bin/osascript",
                    &["-e", "tell application \"Clash Verge\" to quit"],
                )
                .await;
                tokio::time::sleep(Duration::from_secs(3)).await;
                Ok(())
            })
        })
    }
    fn start(&mut self) -> Result<(), &'static str> {
        native(|| {
            runtime()?.block_on(async {
                crate::macos::read_command("/usr/bin/open", &["-a", "Clash Verge"])
                    .await
                    .map(|_| ())
            })
        })
    }
    fn verify(&mut self, _country: &str) -> Result<(), &'static str> {
        native(|| runtime()?.block_on(wait_for_proxy("clash-verge")))
    }
}
fn native<T: Send>(
    task: impl FnOnce() -> Result<T, &'static str> + Send,
) -> Result<T, &'static str> {
    if cfg!(test) {
        return Err("native_proxy_runtime_disabled_in_tests");
    }
    worker(task)
}
async fn wait_for_proxy(mode: &str) -> Result<(), &'static str> {
    for _ in 0..40 {
        if crate::macos::proxy_running(mode).await.unwrap_or(false)
            && crate::macos::tunnel_route().await.unwrap_or(false)
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    Err("代理内核/TUN未就绪，请检查节点或线路")
}

pub(crate) const APP_OPEN_MANUAL: &str = "代理未就绪；请检查节点、线路和 TUN";
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct AppOpenPlan {
    issued_at: u64,
    expires_at: u64,
    #[serde(default = "legacy_app_mode")]
    mode: crate::user_repairs::ProxyRepairMode,
    // Old pending repairs stored a Surge identity observation. Retain decoding only;
    // legacy app opening no longer depends on its path or signing identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    observation: Option<serde_json::Value>,
}
fn legacy_app_mode() -> crate::user_repairs::ProxyRepairMode {
    crate::user_repairs::ProxyRepairMode::Surge
}
impl AppOpenPlan {
    pub(crate) fn limit_expiry(&mut self, deadline: u64) {
        self.expires_at = self.expires_at.min(deadline);
    }
}
fn app_name(mode: crate::user_repairs::ProxyRepairMode) -> Option<&'static str> {
    use crate::user_repairs::ProxyRepairMode::*;
    match mode {
        ClashVerge => Some("Clash Verge"),
        Shadowrocket => Some("Shadowrocket"),
        ClashX => Some("ClashX"),
        Surge => Some("Surge"),
        _ => None,
    }
}
fn mode_name(mode: crate::user_repairs::ProxyRepairMode) -> &'static str {
    use crate::user_repairs::ProxyRepairMode::*;
    match mode {
        ClashVerge => "clash-verge",
        Shadowrocket => "shadowrocket",
        ClashX => "clashx",
        Surge => "surge",
        ZeroDCloud => "0dcloud",
        SingBox => "sing-box",
        Auto => "auto",
    }
}
pub(crate) fn prepare_app_open(
    mode: crate::user_repairs::ProxyRepairMode,
) -> Result<Option<AppOpenPlan>, &'static str> {
    Ok(app_name(mode).map(|_| {
        let issued_at = crate::checks::unix_now();
        AppOpenPlan {
            issued_at,
            expires_at: issued_at.saturating_add(120),
            mode,
            observation: None,
        }
    }))
}
pub(crate) fn execute_app_open(plan: &AppOpenPlan) -> Result<(), &'static str> {
    execute_app_open_with(plan, &mut NativeAppOpener, crate::checks::unix_now)
}
pub(crate) trait AppOpener {
    fn running(&mut self, mode: &str) -> Result<bool, &'static str>;
    fn ready(&mut self, mode: &str) -> Result<bool, &'static str>;
    fn open(&mut self, app: &str) -> Result<(), &'static str>;
    fn pause(&mut self);
}
pub(crate) fn execute_app_open_only_with(
    plan: &AppOpenPlan,
    backend: &mut impl AppOpener,
    now: impl Fn() -> u64,
) -> Result<(), &'static str> {
    let current = now();
    if current < plan.issued_at
        || current >= plan.expires_at
        || plan.expires_at > plan.issued_at.saturating_add(120)
    {
        return Err(APP_OPEN_MANUAL);
    }
    let app = app_name(plan.mode).ok_or(APP_OPEN_MANUAL)?;
    let mode = mode_name(plan.mode);
    if !backend.running(mode)? {
        backend.open(app)?;
    }
    Ok(())
}
fn execute_app_open_with(
    plan: &AppOpenPlan,
    backend: &mut impl AppOpener,
    now: impl Fn() -> u64,
) -> Result<(), &'static str> {
    execute_app_open_only_with(plan, backend, now)?;
    let mode = mode_name(plan.mode);
    for _ in 0..40 {
        if backend.ready(mode)? {
            return Ok(());
        }
        backend.pause();
    }
    Err(APP_OPEN_MANUAL)
}
pub(crate) struct NativeAppOpener;
impl AppOpener for NativeAppOpener {
    fn running(&mut self, mode: &str) -> Result<bool, &'static str> {
        native(|| runtime()?.block_on(crate::macos::proxy_running(mode)))
    }
    fn ready(&mut self, mode: &str) -> Result<bool, &'static str> {
        native(|| {
            runtime()?.block_on(async {
                Ok(crate::macos::proxy_running(mode).await.unwrap_or(false)
                    && crate::macos::tunnel_route().await.unwrap_or(false))
            })
        })
    }
    fn open(&mut self, app: &str) -> Result<(), &'static str> {
        native(|| {
            runtime()?.block_on(async {
                crate::macos::read_command("/usr/bin/open", &["-a", app])
                    .await
                    .map(|_| ())
            })
        })
    }
    fn pause(&mut self) {
        std::thread::sleep(Duration::from_secs(1));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        running: bool,
        ready_after: usize,
        checks: usize,
        opened: Vec<String>,
        pauses: usize,
    }
    impl AppOpener for Fake {
        fn running(&mut self, _: &str) -> Result<bool, &'static str> {
            Ok(self.running)
        }
        fn ready(&mut self, _: &str) -> Result<bool, &'static str> {
            self.checks += 1;
            Ok(self.checks > self.ready_after)
        }
        fn open(&mut self, name: &str) -> Result<(), &'static str> {
            self.opened.push(name.into());
            Ok(())
        }
        fn pause(&mut self) {
            self.pauses += 1;
        }
    }
    #[derive(Default, Debug)]
    struct F05Opener {
        running: bool,
        open_error: bool,
        running_checks: usize,
        checks: usize,
        pauses: usize,
        opened: Vec<String>,
    }
    impl AppOpener for F05Opener {
        fn running(&mut self, _: &str) -> Result<bool, &'static str> {
            self.running_checks += 1;
            Ok(self.running)
        }
        fn ready(&mut self, _: &str) -> Result<bool, &'static str> {
            self.checks += 1;
            Ok(false)
        }
        fn open(&mut self, name: &str) -> Result<(), &'static str> {
            self.opened.push(name.to_owned());
            if self.open_error {
                Err("synthetic_open_failure")
            } else {
                Ok(())
            }
        }
        fn pause(&mut self) {
            self.pauses += 1;
        }
    }
    fn f05_plan(mode: crate::user_repairs::ProxyRepairMode) -> AppOpenPlan {
        AppOpenPlan {
            issued_at: 100,
            expires_at: 220,
            mode,
            observation: None,
        }
    }
    #[test]
    fn f05_open_only_has_no_readiness_budget_and_keeps_app_names() {
        use crate::user_repairs::ProxyRepairMode::*;
        for (mode, name) in [
            (ClashVerge, "Clash Verge"),
            (Shadowrocket, "Shadowrocket"),
            (ClashX, "ClashX"),
            (Surge, "Surge"),
        ] {
            for running in [false, true] {
                let mut fake = F05Opener {
                    running,
                    ..Default::default()
                };
                let result = execute_app_open_only_with(&f05_plan(mode), &mut fake, || 101);
                println!(
                    "F05 open-only mode={mode:?} running={running} result={result:?} counters={fake:?}"
                );
                assert_eq!((fake.checks, fake.pauses), (0, 0));
                assert_eq!(result, Ok(()));
                assert_eq!(fake.running_checks, 1);
                assert_eq!(
                    fake.opened,
                    if running {
                        vec![]
                    } else {
                        vec![name.to_owned()]
                    }
                );
            }
        }
    }
    #[test]
    fn f05_open_only_rejects_bad_time_or_unsupported_mode_before_operations() {
        use crate::user_repairs::ProxyRepairMode::*;
        for (issued_at, expires_at, now) in [
            (100, 220, 99),
            (100, 220, 220),
            (100, 100, 100),
            (100, 221, 101),
        ] {
            let mut plan = f05_plan(Surge);
            plan.issued_at = issued_at;
            plan.expires_at = expires_at;
            let mut fake = F05Opener::default();
            assert_eq!(
                execute_app_open_only_with(&plan, &mut fake, || now),
                Err(APP_OPEN_MANUAL)
            );
            println!(
                "F05 timing issued={issued_at} expires={expires_at} now={now} counters={fake:?}"
            );
            assert_eq!(
                (
                    fake.running_checks,
                    fake.checks,
                    fake.pauses,
                    fake.opened.len()
                ),
                (0, 0, 0, 0)
            );
        }
        for mode in [Auto, SingBox, ZeroDCloud] {
            assert!(prepare_app_open(mode).unwrap().is_none());
            let mut fake = F05Opener::default();
            assert_eq!(
                execute_app_open_only_with(&f05_plan(mode), &mut fake, || 101),
                Err(APP_OPEN_MANUAL)
            );
            assert_eq!(fake.running_checks, 0);
        }
    }
    #[test]
    fn f05_open_only_propagates_open_failure_without_waiting() {
        use crate::user_repairs::ProxyRepairMode::*;
        for mode in [ClashVerge, Shadowrocket, ClashX, Surge] {
            let mut fake = F05Opener {
                open_error: true,
                ..Default::default()
            };
            let result = execute_app_open_only_with(&f05_plan(mode), &mut fake, || 101);
            println!("F05 open-error mode={mode:?} result={result:?} counters={fake:?}");
            assert_eq!(result, Err("synthetic_open_failure"));
            assert_eq!((fake.opened.len(), fake.checks, fake.pauses), (1, 0, 0));
        }
    }
    #[test]
    fn legacy_named_apps_open_without_identity_preconditions_and_wait_forty_seconds() {
        use crate::user_repairs::ProxyRepairMode::*;
        for (mode, name) in [
            (Shadowrocket, "Shadowrocket"),
            (ClashX, "ClashX"),
            (Surge, "Surge"),
            (ClashVerge, "Clash Verge"),
        ] {
            let plan = AppOpenPlan {
                issued_at: 100,
                expires_at: 220,
                mode,
                observation: None,
            };
            let mut fake = Fake {
                running: false,
                ready_after: 39,
                checks: 0,
                opened: vec![],
                pauses: 0,
            };
            execute_app_open_with(&plan, &mut fake, || 101).unwrap();
            assert_eq!(fake.opened, [name]);
            assert_eq!(fake.pauses, 39);
        }
        for mode in [Auto, SingBox, ZeroDCloud] {
            assert!(prepare_app_open(mode).unwrap().is_none());
        }
    }
    #[test]
    fn legacy_pending_plan_keeps_decoding_without_identity_restrictions() {
        let old = serde_json::json!({"issued_at":100,"expires_at":220,"observation":{"files":[],"pid":null}});
        let plan: AppOpenPlan = serde_json::from_value(old).unwrap();
        assert_eq!(plan.mode, crate::user_repairs::ProxyRepairMode::Surge);
        let mut fake = Fake {
            running: false,
            ready_after: 0,
            checks: 0,
            opened: vec![],
            pauses: 0,
        };
        execute_app_open_with(&plan, &mut fake, || 101).unwrap();
        assert_eq!(fake.opened, ["Surge"]);
    }
    #[test]
    fn running_app_waits_for_tun_without_reopening_and_timeout_is_bounded() {
        let plan = AppOpenPlan {
            issued_at: 100,
            expires_at: 220,
            mode: crate::user_repairs::ProxyRepairMode::Surge,
            observation: None,
        };
        let mut fake = Fake {
            running: true,
            ready_after: 40,
            checks: 0,
            opened: vec![],
            pauses: 0,
        };
        assert!(execute_app_open_with(&plan, &mut fake, || 101).is_err());
        assert!(fake.opened.is_empty());
        assert_eq!(fake.pauses, 40);
        assert!(execute_app_open_with(&plan, &mut fake, || 220).is_err());
        assert!(Native.running().is_err());
    }
}
