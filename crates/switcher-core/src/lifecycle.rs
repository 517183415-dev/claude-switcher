//! One launch boundary for GUI and CLI. No caller-supplied evidence can authorize launch.
use crate::{
    accounts::{AccountId, AccountStore},
    checks, config, macos,
    storage::LifecycleLock,
};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use switcher_domain::{Edition, checks::CheckReport, desktop::OperationOutcome};

#[derive(Debug, Serialize)]
pub struct LaunchOutcome {
    pub report: Option<CheckReport>,
    pub outcome: OperationOutcome,
}
fn outcome(completed: bool, message: &str, recovery: Option<&str>) -> OperationOutcome {
    OperationOutcome {
        completed,
        message: message.into(),
        recovery: recovery.map(str::to_owned),
    }
}
fn blocked(report: Option<CheckReport>, message: &str, recovery: &str) -> LaunchOutcome {
    LaunchOutcome {
        report,
        outcome: outcome(false, message, Some(recovery)),
    }
}

trait LaunchEnvironment {
    type Lock;
    fn installed(&self, edition: Edition) -> Result<PathBuf, &'static str>;
    fn lock(&self, home: &Path) -> Result<Self::Lock, &'static str>;
    fn account_ready(&self, home: &Path, lock: &Self::Lock) -> Result<AccountId, &'static str>;
    async fn check(&self, home: &Path) -> Result<CheckReport, &'static str>;
    async fn generation(&self) -> Result<String, &'static str>;
    fn now(&self) -> u64;
    async fn open(&self) -> Result<(), &'static str>;
    async fn select(&self, edition: Edition, target: AccountId) -> Result<(), &'static str>;
}
struct NativeLaunch;
impl LaunchEnvironment for NativeLaunch {
    type Lock = LifecycleLock;
    fn installed(&self, _edition: Edition) -> Result<PathBuf, &'static str> {
        crate::migration::account_home(unsafe { libc::geteuid() }).map_err(|_| "用户目录无法读取")
    }
    fn lock(&self, home: &Path) -> Result<Self::Lock, &'static str> {
        for directory in [
            home.join(".claude-switcher"),
            home.join(".claude-switcher/v2"),
        ] {
            if !directory.exists() {
                crate::storage::create_private_directory(&directory)
                    .map_err(|_| "无法创建账号锁目录")?;
            }
        }
        LifecycleLock::try_acquire(&home.join(".claude-switcher/v2"))
            .map_err(|_| "生命周期锁被占用或目录身份无效")
    }
    fn account_ready(&self, home: &Path, lock: &Self::Lock) -> Result<AccountId, &'static str> {
        let root = home.join(".claude-switcher");
        if !lock.protects(&root.join("v2")) {
            return Err("生命周期锁身份已变化");
        }
        let active = home.join("Library/Application Support/Claude");
        let status = AccountStore::inspect(&root, &active, unsafe { libc::geteuid() })
            .map_err(|_| "账号状态无法验证")?;
        if status.recovery_pending {
            return Err("账号事务尚未恢复完成");
        }
        Ok(status.current.unwrap_or(AccountId::A))
    }
    async fn check(&self, home: &Path) -> Result<CheckReport, &'static str> {
        let configuration = config::load_read_only(home).map_err(|_| "地区目标配置无效")?;
        Ok(checks::run_checks(configuration).await)
    }
    async fn generation(&self) -> Result<String, &'static str> {
        macos::network_generation().await
    }
    fn now(&self) -> u64 {
        checks::unix_now()
    }
    async fn select(&self, edition: Edition, target: AccountId) -> Result<(), &'static str> {
        tokio::task::spawn_blocking(move || {
            let coordinator = crate::migration::MigrationCoordinator::for_current_user(edition)
                .map_err(|_| "账号目录不可用")?;
            match target {
                AccountId::A => coordinator.select_a(),
                AccountId::B => coordinator.select_b(),
            }
            .map_err(|_| "Claude 未能退出或账号事务未完成；资料已保留")
        })
        .await
        .map_err(|_| "账号切换任务中断")?
    }
    async fn open(&self) -> Result<(), &'static str> {
        // The application and executable selectors are fixed. No shell, URL, or caller path.
        bounded_status("/usr/bin/open", &["-a", "Claude"], Duration::from_secs(5)).await
    }
}

/// Check and revalidate environment evidence while holding the account lifecycle lock.
pub async fn launch_checked(edition: Edition) -> LaunchOutcome {
    launch_with(&NativeLaunch, edition).await
}
async fn launch_with<E: LaunchEnvironment>(environment: &E, edition: Edition) -> LaunchOutcome {
    launch_expected_with(environment, edition, None).await
}
/// Reacquires the lifecycle lock after selection and refuses to launch another account.
pub async fn launch_selected(edition: Edition, expected: AccountId) -> LaunchOutcome {
    launch_expected_with(&NativeLaunch, edition, Some(expected)).await
}
/// The old switch command checks the environment before requesting quit or changing account data.
pub async fn switch_checked(edition: Edition, target: AccountId) -> LaunchOutcome {
    switch_with(&NativeLaunch, edition, target).await
}
async fn switch_with<E: LaunchEnvironment>(
    environment: &E,
    edition: Edition,
    target: AccountId,
) -> LaunchOutcome {
    let home = match environment.installed(edition) {
        Ok(home) => home,
        Err(message) => return blocked(None, message, "检查用户目录后重试"),
    };
    let mut report = match environment.check(&home).await {
        Ok(report) => report,
        Err(message) => return blocked(None, message, "修复地区配置后重试"),
    };
    refresh_launch_decision(environment, &mut report).await;
    if !report.launch.allowed {
        return blocked(
            Some(report),
            "环境检查未通过，尚未切换账号",
            "修复阻塞项后重试",
        );
    }
    if let Err(message) = environment.select(edition, target).await {
        return blocked(
            Some(report),
            message,
            "退出 Claude 后重试；未完成事务会自动恢复",
        );
    }
    launch_expected_with(environment, edition, Some(target)).await
}
async fn refresh_launch_decision<E: LaunchEnvironment>(environment: &E, report: &mut CheckReport) {
    let generation = environment.generation().await.unwrap_or_default();
    report.launch = checks::evaluate_launch(report, environment.now(), &generation);
}
fn verify_account<E: LaunchEnvironment>(
    environment: &E,
    home: &Path,
    lock: &E::Lock,
    expected: Option<AccountId>,
) -> Result<(), &'static str> {
    let current = environment.account_ready(home, lock)?;
    if expected.is_some_and(|account| account != current) {
        return Err("当前账号已被其他操作改变，已阻止启动错误账号");
    }
    Ok(())
}
async fn launch_expected_with<E: LaunchEnvironment>(
    environment: &E,
    edition: Edition,
    expected: Option<AccountId>,
) -> LaunchOutcome {
    let home = match environment.installed(edition) {
        Ok(home) => home,
        Err(_) => {
            return blocked(None, "无法读取当前用户目录", "检查当前用户目录后重试");
        }
    };
    let lock = match environment.lock(&home) {
        Ok(lock) => lock,
        Err(message) => return blocked(None, message, "等待账号、安装或更新事务完成后重试"),
    };
    if let Err(message) = verify_account(environment, &home, &lock, expected) {
        return blocked(None, message, "等待账号事务完成后重试");
    }
    let mut report = match environment.check(&home).await {
        Ok(report) => report,
        Err(message) => return blocked(None, message, "修复地区配置后重新执行完整检查"),
    };
    refresh_launch_decision(environment, &mut report).await;
    if !report.launch.allowed {
        return blocked(
            Some(report),
            "环境检查未授予启动许可",
            "修复阻塞项后重新执行完整检查",
        );
    }
    if expected.is_some()
        && let Err(message) = verify_account(environment, &home, &lock, expected)
    {
        return blocked(
            Some(report),
            message,
            "重新选择目标账号，再执行完整检查与启动",
        );
    }
    refresh_launch_decision(environment, &mut report).await;
    if !report.launch.allowed {
        return blocked(
            Some(report),
            "启动前检查证据已失效",
            "重新执行完整检查后重试",
        );
    }
    match environment.open().await {
        Ok(()) => LaunchOutcome {
            report: Some(report),
            outcome: outcome(true, "已向系统提交 Claude 启动请求", None),
        },
        Err(_) => blocked(
            Some(report),
            "系统启动请求失败或超时",
            "检查 Claude 应用与系统响应，再执行完整检查",
        ),
    }
}

fn installed_home(
    edition: Edition,
) -> Result<(PathBuf, switcher_domain::system::InstallRecord), &'static str> {
    #[cfg(target_os = "macos")]
    {
        let owner = unsafe { libc::geteuid() };
        let home = crate::migration::account_home(owner).map_err(|_| "用户目录无法核对")?;
        // Existing install/health verification uses HOME; reject an override before it runs.
        if config::home_dir()? != home {
            return Err("用户目录与系统账户不一致");
        }
        let record = crate::install::validate_installed(edition, owner)?;
        Ok((home, record))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = edition;
        Err("仅支持 macOS")
    }
}

async fn bounded_status(
    program: &str,
    arguments: &[&str],
    timeout: Duration,
) -> Result<(), &'static str> {
    let mut child = tokio::process::Command::new(program)
        .args(arguments)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "固定系统命令无法执行")?;
    match tokio::time::timeout(timeout, child.wait()).await {
        Ok(Ok(status)) if status.success() => Ok(()),
        _ => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            Err("固定系统命令失败或超时")
        }
    }
}
pub(crate) fn codesign_verification_args(requirement: &str, path: &str, deep: bool) -> Vec<String> {
    let mut args = vec!["--verify".into()];
    if deep {
        args.push("--deep".into());
    }
    args.extend([
        "--strict".into(),
        "--test-requirement".into(),
        format!("={requirement}"),
        path.into(),
    ]);
    args
}
async fn verify_app(app: &Path, requirement: &str) -> Result<(), &'static str> {
    crate::storage::validate_no_symlinks(app).map_err(|_| "应用路径不安全")?;
    macos::read_command(
        "/usr/bin/codesign",
        &codesign_verification_args(requirement, app.to_str().ok_or("应用路径无效")?, true)
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    )
    .await
    .map(|_| ())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManualUpdate {
    Check,
    Download,
    Install,
}
impl ManualUpdate {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "check" => Some(Self::Check),
            "download" => Some(Self::Download),
            "install" => Some(Self::Install),
            _ => None,
        }
    }
    fn argument(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Download => "download",
            Self::Install => "install",
        }
    }
}
/// Execute only a verified installed desktop, with the same fixed manual-update contract used by its GUI.
pub async fn update_manual(edition: Edition, action: ManualUpdate) -> OperationOutcome {
    let result = async {
        let (home, record) = installed_home(edition)?;
        let app = Path::new(match edition {
            Edition::Personal => "/Applications/Claude 切换器 Personal.app",
            Edition::Public => "/Applications/Claude 切换器 Public.app",
        });
        record.developer_team()?;
        let requirement = record.client_requirement(switcher_domain::system::ClientRole::Gui)?;
        verify_app(app, &requirement).await?;
        let executable = app.join("Contents/MacOS/switcher-desktop");
        crate::storage::validate_no_symlinks(executable.parent().ok_or("桌面入口无效")?)
            .map_err(|_| "桌面入口路径不安全")?;
        let metadata = std::fs::symlink_metadata(&executable).map_err(|_| "桌面入口不存在")?;
        if !metadata.is_file() {
            return Err("桌面入口不是普通文件");
        }
        macos::read_command(
            "/usr/bin/codesign",
            &codesign_verification_args(
                &requirement,
                executable.to_str().ok_or("桌面入口无效")?,
                false,
            )
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        )
        .await?;
        let (_, current) = installed_home(edition)?;
        if current != record {
            return Err("安装身份已变化");
        }
        run_desktop_update(&executable, &home, action).await
    }
    .await;
    match result {
        Ok(result) => result,
        Err("source_local_rebuild_required") => outcome(
            false,
            "source_local_rebuild_required：请从源码重新构建并由本地安装器更新",
            Some("运行本地构建与管理员安装流程"),
        ),
        Err(_) => outcome(
            false,
            "更新未完成：本机安装或更新任务校验未通过",
            Some("源码版本请重新构建并使用本地安装器更新；其他版本可在更新页查看原因"),
        ),
    }
}

async fn run_desktop_update(
    executable: &Path,
    home: &Path,
    action: ManualUpdate,
) -> Result<OperationOutcome, &'static str> {
    use tokio::io::AsyncReadExt;
    let mut child = tokio::process::Command::new(executable)
        .args(["--update-manual", action.argument()])
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .env("HOME", home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| "桌面更新任务无法执行")?;
    let stdout = child.stdout.take().ok_or("桌面更新响应不可用")?;
    let task = async {
        let mut bytes = Vec::new();
        stdout
            .take(16385)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| "桌面更新响应不可用")?;
        if bytes.len() > 16384 {
            return Err("桌面更新响应超过限制");
        }
        let status = child.wait().await.map_err(|_| "桌面更新任务无法确认")?;
        let result: OperationOutcome =
            crate::signed::strict_json(&bytes, 16384).map_err(|_| "桌面更新响应无效")?;
        if status.success() != result.completed
            || result.message.len() > 4096
            || result
                .recovery
                .as_ref()
                .is_some_and(|value| value.len() > 4096)
        {
            return Err("桌面更新状态不一致");
        }
        Ok(result)
    };
    match tokio::time::timeout(Duration::from_secs(15 * 60), task).await {
        Ok(Ok(result)) => Ok(result),
        _ => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            Err("桌面更新失败、响应无效或超时")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use switcher_domain::checks::{
        CheckId, CheckResult, CheckStatus, CheckSummary, LaunchDecision,
    };
    struct Fake {
        log: Mutex<Vec<&'static str>>,
        current: Mutex<AccountId>,
        generations: Mutex<std::collections::VecDeque<Result<&'static str, &'static str>>>,
        times: Mutex<std::collections::VecDeque<u64>>,
        generation_calls: std::sync::atomic::AtomicUsize,
        failure: Mutex<Option<&'static str>>,
        fail_check: bool,
        account_changes: bool,
        fail_open: bool,
    }
    impl Default for Fake {
        fn default() -> Self {
            Self {
                log: Mutex::new(vec![]),
                current: Mutex::new(AccountId::A),
                generations: Mutex::new([Ok("0:synthetic")].into()),
                times: Mutex::new([101].into()),
                generation_calls: std::sync::atomic::AtomicUsize::new(0),
                failure: Mutex::new(None),
                fail_check: false,
                account_changes: false,
                fail_open: false,
            }
        }
    }
    impl Fake {
        fn fail_at(&self, stage: &'static str) -> Result<(), &'static str> {
            if *self.failure.lock().unwrap() == Some(stage) {
                Err(stage)
            } else {
                Ok(())
            }
        }
    }
    impl LaunchEnvironment for Fake {
        type Lock = ();
        fn installed(&self, _: Edition) -> Result<PathBuf, &'static str> {
            self.fail_at("home-unavailable")?;
            Ok(PathBuf::from("/synthetic"))
        }
        fn lock(&self, _: &Path) -> Result<(), &'static str> {
            self.log.lock().unwrap().push("lock");
            self.fail_at("lock-unavailable")?;
            Ok(())
        }
        fn account_ready(&self, _: &Path, _: &()) -> Result<AccountId, &'static str> {
            self.fail_at("account-recovery-pending")?;
            Ok(*self.current.lock().unwrap())
        }
        async fn generation(&self) -> Result<String, &'static str> {
            self.generation_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let mut samples = self.generations.lock().unwrap();
            let sample = if samples.len() > 1 {
                samples.pop_front().unwrap()
            } else {
                *samples.front().unwrap()
            };
            sample.map(str::to_owned)
        }
        fn now(&self) -> u64 {
            let mut samples = self.times.lock().unwrap();
            if samples.len() > 1 {
                samples.pop_front().unwrap()
            } else {
                *samples.front().unwrap()
            }
        }
        async fn check(&self, _: &Path) -> Result<CheckReport, &'static str> {
            self.log.lock().unwrap().push("check");
            self.fail_at("check-unavailable")?;
            if self.log.lock().unwrap().contains(&"quit-and-select") {
                self.fail_at("check-after-select")?;
            }
            if self.account_changes {
                *self.current.lock().unwrap() = AccountId::B;
            }
            let mut rows: Vec<_> = CheckId::ALL
                .into_iter()
                .map(|id| CheckResult {
                    id,
                    status: CheckStatus::Pass,
                    reason: "synthetic".into(),
                    observed_at: 100,
                    duration_ms: 1,
                    valid_until: 130,
                    network_generation: "0:synthetic".into(),
                    evidence: vec![],
                })
                .collect();
            if self.fail_check {
                rows[0].status = CheckStatus::Fail;
            }
            Ok(CheckReport {
                schema_version: 1,
                observed_at: 100,
                completed_at: 101,
                duration_ms: 1,
                network_generation: "0:synthetic".into(),
                checks: rows,
                summary: CheckSummary {
                    passed: 14,
                    advisory: 0,
                    blocking: 0,
                },
                launch: LaunchDecision {
                    allowed: true,
                    reasons: vec![],
                },
            })
        }
        async fn select(&self, _: Edition, target: AccountId) -> Result<(), &'static str> {
            self.log.lock().unwrap().push("quit-and-select");
            self.fail_at("selection-interrupted")?;
            *self.current.lock().unwrap() = target;
            Ok(())
        }
        async fn open(&self) -> Result<(), &'static str> {
            self.log.lock().unwrap().push("open");
            if self.fail_open {
                Err("synthetic error")
            } else {
                Ok(())
            }
        }
    }
    #[tokio::test]
    async fn preflight_failure_does_not_quit_switch_or_open() {
        let fake = Fake {
            fail_check: true,
            ..Default::default()
        };
        assert!(
            !switch_with(&fake, Edition::Personal, AccountId::B)
                .await
                .outcome
                .completed
        );
        assert_eq!(*fake.current.lock().unwrap(), AccountId::A);
        assert_eq!(*fake.log.lock().unwrap(), vec!["check"]);
    }
    #[tokio::test]
    async fn first_b_checks_before_selection_and_again_before_open() {
        let fake = Fake::default();
        assert!(
            switch_with(&fake, Edition::Personal, AccountId::B)
                .await
                .outcome
                .completed
        );
        assert_eq!(*fake.current.lock().unwrap(), AccountId::B);
        assert_eq!(
            *fake.log.lock().unwrap(),
            vec!["check", "quit-and-select", "lock", "check", "open"]
        );
    }
    #[tokio::test]
    async fn selected_account_change_during_check_prevents_wrong_account_launch() {
        let fake = Fake {
            account_changes: true,
            ..Default::default()
        };
        assert!(
            !launch_expected_with(&fake, Edition::Personal, Some(AccountId::A))
                .await
                .outcome
                .completed
        );
        assert!(!fake.log.lock().unwrap().contains(&"open"));
    }
    #[tokio::test]
    async fn ordinary_launch_needs_only_checks_and_system_open() {
        let fake = Fake::default();
        assert!(
            launch_with(&fake, Edition::Personal)
                .await
                .outcome
                .completed
        );
        assert_eq!(*fake.log.lock().unwrap(), vec!["lock", "check", "open"]);
    }
    #[tokio::test]
    async fn open_failure_is_reported_without_claiming_launch() {
        let fake = Fake {
            fail_open: true,
            ..Default::default()
        };
        assert!(
            !launch_with(&fake, Edition::Personal)
                .await
                .outcome
                .completed
        );
    }

    #[tokio::test]
    async fn changed_or_unavailable_network_blocks_switch_before_selection() {
        for sample in [Ok("1:other-route"), Ok(""), Err("observation_unavailable")] {
            let fake = Fake {
                generations: Mutex::new([sample].into()),
                ..Default::default()
            };
            let result = switch_with(&fake, Edition::Personal, AccountId::B).await;
            assert!(!result.outcome.completed);
            assert!(!result.report.unwrap().launch.allowed);
            assert_eq!(*fake.current.lock().unwrap(), AccountId::A);
            assert_eq!(*fake.log.lock().unwrap(), vec!["check"]);
        }
    }

    #[tokio::test]
    async fn final_network_recheck_blocks_aba_wake_and_sampling_failure() {
        for sample in [
            Ok("2:synthetic"),
            Ok("1:other-route"),
            Err("observer_stopped"),
        ] {
            let fake = Fake {
                generations: Mutex::new([Ok("0:synthetic"), sample].into()),
                ..Default::default()
            };
            let result = launch_expected_with(&fake, Edition::Personal, Some(AccountId::A)).await;
            assert!(!result.outcome.completed);
            assert!(!result.report.unwrap().launch.allowed);
            assert!(!fake.log.lock().unwrap().contains(&"open"));
            assert_eq!(
                fake.generation_calls
                    .load(std::sync::atomic::Ordering::SeqCst),
                2
            );
        }
    }

    #[tokio::test]
    async fn evidence_expiring_after_initial_gate_never_reaches_system_open() {
        let fake = Fake {
            times: Mutex::new([101, 131].into()),
            ..Default::default()
        };
        let result = launch_with(&fake, Edition::Public).await;
        assert!(!result.outcome.completed);
        assert!(!result.report.unwrap().launch.allowed);
        assert!(!fake.log.lock().unwrap().contains(&"open"));
    }

    #[tokio::test]
    async fn stable_network_allows_both_editions_at_the_thirty_second_boundary() {
        for edition in [Edition::Personal, Edition::Public] {
            let fake = Fake {
                times: Mutex::new([130].into()),
                ..Default::default()
            };
            let result = launch_with(&fake, edition).await;
            assert!(result.outcome.completed);
            assert!(result.report.unwrap().launch.allowed);
            assert_eq!(
                fake.generation_calls
                    .load(std::sync::atomic::Ordering::SeqCst),
                2
            );
        }
    }

    #[tokio::test]
    async fn prerequisite_errors_block_launch_until_explicit_fresh_retry() {
        for edition in [Edition::Personal, Edition::Public] {
            for (stage, expected_calls) in [
                ("home-unavailable", vec![]),
                ("lock-unavailable", vec!["lock"]),
                ("account-recovery-pending", vec!["lock"]),
                ("check-unavailable", vec!["lock", "check"]),
            ] {
                let fake = Fake {
                    failure: Mutex::new(Some(stage)),
                    ..Default::default()
                };
                let blocked = launch_with(&fake, edition).await;
                assert!(!blocked.outcome.completed);
                assert!(blocked.outcome.recovery.is_some());
                assert!(blocked.report.is_none());
                assert_eq!(*fake.current.lock().unwrap(), AccountId::A);
                assert_eq!(*fake.log.lock().unwrap(), expected_calls);
                assert_eq!(
                    fake.generation_calls
                        .load(std::sync::atomic::Ordering::SeqCst),
                    0
                );
                // Clearing an external prerequisite cannot replay the blocked request.
                *fake.failure.lock().unwrap() = None;
                fake.log.lock().unwrap().clear();
                let retried = launch_with(&fake, edition).await;
                assert!(retried.outcome.completed);
                assert!(retried.report.unwrap().launch.allowed);
                assert_eq!(*fake.log.lock().unwrap(), vec!["lock", "check", "open"]);
                assert_eq!(*fake.current.lock().unwrap(), AccountId::A);
                assert_eq!(
                    fake.generation_calls
                        .load(std::sync::atomic::Ordering::SeqCst),
                    2
                );
            }
        }
    }

    #[tokio::test]
    async fn interrupted_selection_keeps_account_and_requires_explicit_retry() {
        for edition in [Edition::Personal, Edition::Public] {
            let fake = Fake {
                failure: Mutex::new(Some("selection-interrupted")),
                ..Default::default()
            };
            let blocked = switch_with(&fake, edition, AccountId::B).await;
            assert!(!blocked.outcome.completed);
            assert_eq!(blocked.outcome.message, "selection-interrupted");
            assert!(blocked.outcome.recovery.is_some());
            assert!(blocked.report.unwrap().launch.allowed);
            assert_eq!(*fake.current.lock().unwrap(), AccountId::A);
            assert_eq!(*fake.log.lock().unwrap(), vec!["check", "quit-and-select"]);
            *fake.failure.lock().unwrap() = None;
            fake.log.lock().unwrap().clear();
            let retried = switch_with(&fake, edition, AccountId::B).await;
            assert!(retried.outcome.completed);
            assert_eq!(*fake.current.lock().unwrap(), AccountId::B);
            assert_eq!(
                *fake.log.lock().unwrap(),
                vec!["check", "quit-and-select", "lock", "check", "open"]
            );
        }
    }

    #[tokio::test]
    async fn check_error_after_selection_preserves_target_until_fresh_launch_check() {
        for edition in [Edition::Personal, Edition::Public] {
            let fake = Fake {
                failure: Mutex::new(Some("check-after-select")),
                ..Default::default()
            };
            let blocked = switch_with(&fake, edition, AccountId::B).await;
            assert!(!blocked.outcome.completed);
            assert_eq!(blocked.outcome.message, "check-after-select");
            assert!(
                blocked
                    .outcome
                    .recovery
                    .unwrap()
                    .contains("重新执行完整检查")
            );
            assert!(blocked.report.is_none());
            assert_eq!(*fake.current.lock().unwrap(), AccountId::B);
            assert_eq!(
                *fake.log.lock().unwrap(),
                vec!["check", "quit-and-select", "lock", "check"]
            );
            assert_eq!(
                fake.generation_calls
                    .load(std::sync::atomic::Ordering::SeqCst),
                1
            );
            *fake.failure.lock().unwrap() = None;
            fake.log.lock().unwrap().clear();
            let retried = launch_expected_with(&fake, edition, Some(AccountId::B)).await;
            assert!(retried.outcome.completed);
            assert!(retried.report.unwrap().launch.allowed);
            assert_eq!(*fake.current.lock().unwrap(), AccountId::B);
            assert_eq!(*fake.log.lock().unwrap(), vec!["lock", "check", "open"]);
            assert_eq!(
                fake.generation_calls
                    .load(std::sync::atomic::Ordering::SeqCst),
                3
            );
        }
    }
}
