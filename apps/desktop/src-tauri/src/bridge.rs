//! Desktop coordination. Remote content never supplies operations or filesystem paths.
use base64::Engine;
use std::{path::PathBuf, sync::Mutex};
use switcher_core::{
    accounts::{AccountId, AccountStore},
    ad_runtime::{AdRuntime, ClickDecision},
    checks, config, diagnostics, regions, storage, time,
};
use switcher_domain::{Edition, checks::CheckReport, desktop::*};

pub struct DesktopState {
    pub root: PathBuf,
    pub edition: Edition,
    sessions: CheckSessions,
    repair_plan: Mutex<RepairPlanState>,
}

/// One pending UI confirmation. The request ID is not an authorization credential.
#[derive(Default)]
pub struct RepairPlanState {
    request_id: Option<String>,
    prepared: Option<regions::PreparedRepair>,
}
impl RepairPlanState {
    pub fn begin(&mut self, request_id: String) {
        self.request_id = Some(request_id);
        self.prepared = None;
    }
    pub fn matches(&self, request_id: &str) -> bool {
        self.request_id.as_deref() == Some(request_id)
    }
    pub fn cancel(&mut self, request_id: &str) {
        if self.matches(request_id) {
            *self = Self::default();
        }
    }
    pub fn complete(
        &mut self,
        request_id: &str,
        prepared: Option<regions::PreparedRepair>,
    ) -> bool {
        if !self.matches(request_id) {
            return false;
        }
        self.prepared = prepared;
        true
    }
    pub(crate) fn take(&mut self, request_id: &str) -> Option<regions::PreparedRepair> {
        if !self.matches(request_id) {
            return None;
        }
        self.request_id = None;
        self.prepared.take()
    }
}

pub fn completed(message: impl Into<String>) -> OperationOutcome {
    OperationOutcome {
        completed: true,
        message: message.into(),
        recovery: None,
    }
}
pub fn waiting(message: impl Into<String>, recovery: impl Into<String>) -> OperationOutcome {
    OperationOutcome {
        completed: false,
        message: message.into(),
        recovery: Some(recovery.into()),
    }
}

/// Recovery guidance follows the persisted transaction, not new-operation dependencies.
pub fn account_action_failure(action: AccountAction, error: impl Into<String>) -> OperationOutcome {
    waiting(
        error,
        match action {
            AccountAction::Recover => "保留账号资料，核对 Claude 退出状态和中断记录后重试恢复",
            _ => "保留账号资料，按错误提示处理后重试",
        },
    )
}

impl DesktopState {
    /// Explicit synthetic root for compile-time-only business tests.
    #[cfg(any(feature = "ipc-business", all(test, feature = "personal")))]
    pub(crate) fn isolated_activity_fixture(root: PathBuf, edition: Edition) -> Self {
        Self {
            root,
            edition,
            sessions: CheckSessions::default(),
            repair_plan: Mutex::new(RepairPlanState::default()),
        }
    }

    pub fn new(edition: Edition) -> Result<Self, String> {
        let root = config::home_dir()
            .map_err(|_| "无法确认本机用户目录")?
            .join(".claude-switcher");
        Ok(Self {
            root,
            edition,
            sessions: CheckSessions::default(),
            repair_plan: Mutex::new(RepairPlanState::default()),
        })
    }

    pub fn directory(&self) -> PathBuf {
        self.root.join("v2").join(edition_name(self.edition))
    }
    fn home(&self) -> Result<PathBuf, String> {
        config::home_dir().map_err(|_| "无法确认本机用户目录".into())
    }
    fn owner(&self) -> u32 {
        unsafe { libc::geteuid() }
    }
    pub fn prepare_directory(&self) -> Result<(), String> {
        if !self.root.exists() {
            storage::create_private_directory(&self.root).map_err(|_| "无法创建私有配置目录")?;
        }
        storage::validate_no_symlinks(&self.root).map_err(|_| "配置目录包含不受支持的链接")?;
        storage::create_private_directory(&self.root.join("v2"))
            .map_err(|_| "无法创建 v2 私有目录")?;
        storage::create_private_directory(&self.directory()).map_err(|_| "无法创建发行配置目录")?;
        Ok(())
    }
    pub fn preferences(&self) -> Result<Preferences, String> {
        Ok(read_optional(&self.directory().join("preferences.json"))?.unwrap_or_default())
    }
    pub fn save_preferences(&self, preferences: Preferences) -> Result<Preferences, String> {
        if preferences.ads.slots_enabled.iter().any(|v| *v)
            && !preferences.ads.request_purpose_acknowledged
        {
            return Err("请先确认广告请求用途，再开启广告位".into());
        }
        self.prepare_directory()?;
        storage::atomic_json(&self.directory().join("preferences.json"), &preferences)
            .map_err(|_| "设置保存失败")?;
        self.log("info", "已保存本机设置；全部广告关闭时不发送广告请求")?;
        Ok(preferences)
    }
    pub fn accounts(&self) -> Result<AccountOverview, String> {
        let active = self.home()?.join("Library/Application Support/Claude");
        let status = AccountStore::inspect(&self.root, &active, self.owner())
            .map_err(|_| "账号元数据无法安全读取；请检查恢复状态")?;
        let current = status.current.map(|id| match id {
            AccountId::A => "A".into(),
            AccountId::B => "B".into(),
        });
        let profiles = [("A", status.a_saved), ("B", status.b_saved)]
            .into_iter()
            .map(|(id, saved)| AccountProfile {
                id: id.into(),
                state: if status.recovery_pending {
                    ProfileState::RecoveryPending
                } else if current.as_deref() == Some(id) {
                    ProfileState::Current
                } else if saved {
                    ProfileState::Saved
                } else {
                    ProfileState::Uninitialized
                },
                login_evidence: "登录状态尚未通过真实登录验收".into(),
            })
            .collect();
        Ok(AccountOverview {
            current,
            profiles,
            claude_running: switcher_core::accounts::ensure_claude_stopped(self.owner()).is_err(),
            action_blocker: if status.recovery_pending {
                Some("存在待恢复的账号事务".into())
            } else {
                None
            },
        })
    }
    pub fn snapshot(&self) -> Result<WorkspaceSnapshot, String> {
        let mut materials = Vec::new();
        if !switcher_core::ad_runtime::local::configured(&self.root) {
            materials.push("本机广告尚未完成首次签名发布；远程广告上线未配置".into());
        }
        let preferences = self.preferences()?;
        let mut runtime = switcher_core::runtime_info(self.edition);
        runtime.ads_enabled = preferences.ads.slots_enabled.iter().any(|v| *v);
        let ads_manifest =
            match AdRuntime::new(self.root.clone(), self.edition).snapshot(&preferences.ads) {
                Ok(manifest) => manifest,
                Err(reason) => {
                    materials.push(format!(
                        "广告已收起：{reason}。请检查 Personal 本机运营内容。"
                    ));
                    None
                }
            };
        Ok(WorkspaceSnapshot {
            runtime,
            accounts: self.accounts()?,
            ads_manifest,
            preferences,
            check_report: self.sessions.report()?,
            update: crate::update_runtime::UpdateRuntime::new(self.root.clone(), self.edition)
                .snapshot(),
            pending_materials: materials,
        })
    }
    pub async fn check(&self) -> Result<CheckReport, String> {
        let session = self.sessions.begin()?;
        let config =
            config::load_read_only(&self.home()?).map_err(|_| "地区目标配置无效，请先修复配置")?;
        let report = tokio::select! {
            biased;
            () = session.wait_cancelled() => return Err("已取消本次环境检查；后台守护继续运行".into()),
            report = checks::run_checks(config) => report,
        };
        self.log(
            "info",
            &format!(
                "环境检查完成：{} 项通过，{} 项提示，{} 项阻塞",
                report.summary.passed, report.summary.advisory, report.summary.blocking
            ),
        )?;
        session.publish(report.clone())?;
        Ok(report)
    }
    pub fn cancel_check(&self) -> OperationOutcome {
        match self.sessions.cancel() {
            Ok(true) => completed("已取消本次检查；后台守护继续运行"),
            Ok(false) => completed("当前没有进行中的检查"),
            Err(error) => waiting(error, "检查状态不可用，请重新连接桌面核心"),
        }
    }
    pub fn migration_preparation(&self) -> Result<AccountPreparation, String> {
        let coordinator =
            switcher_core::migration::MigrationCoordinator::for_current_user(self.edition)
                .map_err(|_| "无法确认本机迁移上下文，请核对系统用户目录")?;
        let readiness = coordinator
            .readiness()
            .map_err(|_| "无法安全核对迁移准备状态，请保留资料并检查恢复入口")?;
        let active = self.home()?.join("Library/Application Support/Claude");
        let status = AccountStore::inspect(&self.root, &active, self.owner())
            .map_err(|_| "账号元数据无法安全读取；请检查恢复状态")?;
        let action = in_place_action_for_status(&status);
        let scoped = action
            .map(|action| {
                use switcher_core::migration::MigrationIntent;
                coordinator.readiness_for(match action {
                    InPlaceAction::InitializeA => MigrationIntent::InitializeA,
                    InPlaceAction::AssociateA => MigrationIntent::AssociateA,
                })
            })
            .transpose()
            .map_err(|_| "无法安全核对原位准备状态，请保留资料并检查恢复入口")?;
        Ok(with_in_place_preparation(
            account_preparation_view(readiness),
            action,
            scoped,
        ))
    }
    pub fn create_migration_backup(&self) -> OperationOutcome {
        waiting(
            "旧版账号切换会自动存档，无需迁移备份或隔离恢复步骤",
            "此接口仅保留兼容；请使用账号选择。启动前仍需通过环境检查。",
        )
    }
    pub fn account_action(&self, action: AccountAction) -> OperationOutcome {
        let coordinator =
            match switcher_core::migration::MigrationCoordinator::for_current_user(self.edition) {
                Ok(value) => value,
                Err(error) => return waiting(error.to_string(), "检查本机用户目录与迁移准备状态"),
            };
        let result = match action {
            AccountAction::InitializeA => coordinator.initialize_a(),
            AccountAction::AssociateA => coordinator.associate_a(),
            AccountAction::SelectA => coordinator.select_a(),
            AccountAction::SelectB => coordinator.select_b(),
            AccountAction::Recover => coordinator.recover(),
        };
        match result {
            Ok(()) => {
                let _ = self.sessions.replace(None);
                let _ = self.log("info", "账号事务已完成；启动前需重新检查环境");
                completed("账号事务已完成；登录状态仍需实际核实")
            }
            Err(error) => account_action_failure(action, error.to_string()),
        }
    }
    pub async fn switch_account(&self, target: AccountId) -> OperationOutcome {
        let result = switcher_core::lifecycle::switch_checked(self.edition, target).await;
        let _ = self.sessions.replace(result.report);
        let _ = self.log(
            if result.outcome.completed {
                "info"
            } else {
                "warning"
            },
            &result.outcome.message,
        );
        result.outcome
    }
    pub async fn launch_checked(&self) -> OperationOutcome {
        let result = switcher_core::lifecycle::launch_checked(self.edition).await;
        let _ = self.sessions.replace(result.report);
        result.outcome
    }
    pub fn region_view(&self) -> Result<RegionView, String> {
        regions::RegionCoordinator::for_current_user(self.edition)
            .and_then(|coordinator| coordinator.view())
            .map_err(str::to_owned)
    }
    /// Register before enqueueing a worker so cancellation also invalidates queued work.
    pub fn begin_repair_preview(&self, request_id: String) -> Result<(), String> {
        self.repair_plan
            .lock()
            .map_err(|_| "修复计划状态不可用".to_owned())?
            .begin(request_id);
        Ok(())
    }
    pub fn preview_region(
        &self,
        region: Option<RegionPreset>,
        request_id: String,
    ) -> RepairPreview {
        let original = self.region_view().unwrap_or(RegionView {
            country: "未设置".into(),
            timezone: "未设置".into(),
            locale: "未设置".into(),
        });
        let target = region
            .map(regions::preset)
            .unwrap_or_else(|| original.clone());
        let result = regions::RegionCoordinator::for_current_user(self.edition)
            .and_then(|coordinator| coordinator.preview(region));
        match result {
            Ok(plan) => {
                let preview = plan.preview.clone();
                match self.repair_plan.lock() {
                    Ok(mut saved) => {
                        let prepared = if preview.can_execute && !preview.recovery_required {
                            Some(plan)
                        } else {
                            None
                        };
                        if !saved.complete(&request_id, prepared) {
                            return RepairPreview {
                                can_execute: false,
                                message: "此计划已取消或被新的预览取代".into(),
                                ..preview
                            };
                        }
                        preview
                    }
                    Err(_) => RepairPreview {
                        can_execute: false,
                        message: "修复计划状态不可用".into(),
                        ..preview
                    },
                }
            }
            Err(error) => RepairPreview {
                can_execute: false,
                message: error.into(),
                original,
                target,
                network_service: "未读取".into(),
                original_ipv6: "未读取".into(),
                target_ipv6: "关闭".into(),
                user_changes: Vec::new(),
                recovery_required: false,
            },
        }
    }
    pub fn preview_repair(&self, request_id: String) -> RepairPreview {
        self.preview_region(None, request_id)
    }
    pub fn cancel_repair(&self, request_id: &str) -> OperationOutcome {
        match self.repair_plan.lock() {
            Ok(mut plan) => {
                plan.cancel(request_id);
                completed("修复计划已取消")
            }
            Err(_) => waiting("修复计划状态不可用", "重新连接后预览"),
        }
    }
    pub fn recover_repair(&self) -> OperationOutcome {
        if let Ok(mut plan) = self.repair_plan.lock() {
            *plan = RepairPlanState::default();
        }
        let result = regions::RegionCoordinator::for_current_user(self.edition)
            .and_then(|coordinator| coordinator.recover_user_repair());
        match result {
            Ok(recovered) => {
                let _ = self.sessions.replace(None);
                completed(if recovered {
                    "修复事务已处理。请核对地区目标、系统时区与 IPv6，再重新检查环境。"
                } else {
                    "没有待恢复的修复事务。"
                })
            }
            Err(error) => waiting(error, "保留事务与备份，核对冲突后重试恢复"),
        }
    }
    pub fn execute_repair(&self, request_id: &str) -> OperationOutcome {
        let prepared = self
            .repair_plan
            .lock()
            .ok()
            .and_then(|mut value| value.take(request_id));
        let Some(prepared) = prepared else {
            return waiting("尚无可执行的修复计划", "先预览修复计划");
        };
        let result = regions::RegionCoordinator::for_current_user(self.edition)
            .and_then(|coordinator| coordinator.execute(prepared));
        match result {
            Ok(()) => {
                let _ = self.sessions.replace(None);
                let _ = self.log("info", "系统组件已执行修复并保存地区目标");
                completed("系统修复已完成并保存地区目标；请重新检查环境与地区格式")
            }
            Err(error) => waiting(error, "核对逐项恢复结果，再重新预览"),
        }
    }
    pub fn rebuild_time(&self) -> Result<OperationOutcome, String> {
        self.prepare_directory()?;
        let _lock = storage::LifecycleLock::try_acquire(&self.root.join("v2"))
            .map_err(|_| "其他生命周期操作正在进行")?;
        let path = self.directory().join("clock.json");
        let mut clock: time::ClockState = read_optional(&path)?.unwrap_or_default();
        clock.establish_baseline(time::sample().map_err(|_| "无法读取系统时间")?);
        storage::atomic_json(&path, &clock).map_err(|_| "时间基线保存失败")?;
        Ok(waiting(
            "已重新建立本地时间基线",
            "等待固定来源签名刷新通过；广告与软件安装暂不恢复",
        ))
    }
    pub fn refresh_ads(&self, manual: bool) -> Result<OperationOutcome, String> {
        let preferences = self.preferences()?;
        let result = AdRuntime::new(self.root.clone(), self.edition)
            .refresh_if_due(&preferences.ads, manual)
            .map_err(str::to_owned)?;
        if result.refreshed {
            Ok(completed("已验证并刷新广告内容"))
        } else {
            Ok(waiting(
                match result.reason {
                    "wait_ad_content_materials" => "本机广告尚未完成首次签名发布".into(),
                    "ads_disabled" | "all_slots_disabled" => {
                        "全部广告位已关闭，未发送广告请求".into()
                    }
                    _ => format!("本次没有刷新广告：{}", result.reason),
                },
                "在 Personal 中发布到本机正式目录，再开启广告位",
            ))
        }
    }
    pub fn cached_image(
        &self,
        slot: switcher_domain::AdSlot,
        sha256: &str,
    ) -> Result<String, String> {
        let preferences = self.preferences()?;
        let image = AdRuntime::new(self.root.clone(), self.edition)
            .image_bytes(slot, &preferences.ads)
            .map_err(str::to_owned)?;
        let Some((asset, bytes)) = image else {
            return Err("该广告位当前没有有效缓存素材".into());
        };
        if asset.sha256 != sha256 {
            return Err("素材已改变，请刷新广告展示".into());
        }
        Ok(format!(
            "data:{};base64,{}",
            asset.mime,
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    }
    pub fn open_ad(&self, slot: switcher_domain::AdSlot, confirmed: bool) -> OperationOutcome {
        let preferences = match self.preferences() {
            Ok(value) => value,
            Err(error) => return waiting(error, "检查本机广告设置"),
        };
        let decision = AdRuntime::new(self.root.clone(), self.edition).click_target(
            slot,
            confirmed,
            &preferences.ads,
        );
        match decision {
            Ok(ClickDecision::NeedsConfirmation {
                merchant_name,
                normalized_host,
            }) => waiting(
                format!(
                    "即将打开商家「{}」：{}。系统浏览器会访问该商家。",
                    merchant_name, normalized_host
                ),
                "confirm_ad_link",
            ),
            Ok(ClickDecision::Ready { url }) => match std::process::Command::new("/usr/bin/open")
                .arg(url)
                .status()
            {
                Ok(status) if status.success() => completed("已向系统浏览器提交商家链接"),
                _ => waiting("系统未接受商家链接", "检查默认浏览器设置后重试"),
            },
            Err(_) => waiting("当前没有可打开的有效广告", "刷新已验证内容后重试"),
        }
    }
    pub fn activity(&self) -> Result<Vec<ActivityEntry>, String> {
        let values: Vec<ActivityEntry> =
            read_optional(&self.directory().join("activity.json"))?.unwrap_or_default();
        Ok(values
            .into_iter()
            .map(|mut entry| {
                entry.message = diagnostics::redact(&entry.message);
                entry
            })
            .collect())
    }
    /// No caller-controlled path or contents. Return exactly the exported snapshot.
    pub fn export_activity(&self) -> Result<(String, Vec<ActivityEntry>), String> {
        self.prepare_directory()?;
        let directory = self.directory().join("exports");
        storage::create_private_directory(&directory)
            .map_err(|_| "无法安全创建私有日志导出目录")?;
        let entries = self.activity()?;
        let path = directory.join("activity-redacted.json");
        storage::atomic_json(&path, &entries).map_err(|_| "脱敏日志无法安全写入固定导出文件")?;
        Ok((path.to_string_lossy().into_owned(), entries))
    }
    pub fn log(&self, level: &str, message: &str) -> Result<(), String> {
        self.prepare_directory()?;
        let mut entries = self.activity()?;
        entries.push(ActivityEntry {
            observed_at: checks::unix_now(),
            level: level.into(),
            message: diagnostics::redact(message),
        });
        while entries.len() > 100
            || serde_json::to_vec(&entries)
                .map_err(|_| "活动记录编码失败")?
                .len()
                > 60 * 1024
        {
            entries.remove(0);
        }
        storage::atomic_json(&self.directory().join("activity.json"), &entries)
            .map_err(|_| "活动记录保存失败".into())
    }
}

pub fn edition_name(edition: Edition) -> &'static str {
    match edition {
        Edition::Personal => "personal",
        Edition::Public => "public",
    }
}
fn read_optional<T: serde::de::DeserializeOwned>(
    path: &std::path::Path,
) -> Result<Option<T>, String> {
    match storage::read_json(path) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("本地元数据无法安全读取".into()),
    }
}

pub fn open_date_settings() -> OperationOutcome {
    match std::process::Command::new("/usr/bin/open")
        .arg("x-apple.systempreferences:com.apple.Date-Time-Settings.extension")
        .status()
    {
        Ok(status) if status.success() => completed("已打开系统日期与时间设置"),
        _ => waiting("无法打开系统日期与时间设置", "请从系统设置进入日期与时间"),
    }
}

#[cfg(test)]
mod migration_compatibility_tests {
    use super::*;

    #[test]
    fn unsupported_backup_returns_without_opening_workspace() {
        for edition in [Edition::Personal, Edition::Public] {
            let state = DesktopState {
                root: PathBuf::from("/synthetic/unopened-workspace"),
                edition,
                sessions: CheckSessions::default(),
                repair_plan: Mutex::new(RepairPlanState::default()),
            };
            let outcome = state.create_migration_backup();
            assert!(!outcome.completed);
            assert!(outcome.message.contains("无需迁移备份"));
            let guidance = outcome.recovery.unwrap();
            assert!(guidance.contains("仅保留兼容"));
            assert!(guidance.contains("账号选择"));
            assert!(guidance.contains("环境检查"));
        }
    }
}

/// Project only fixed readiness blockers. Never expose Keychain metadata or selectors.
pub fn account_preparation_view(
    readiness: switcher_core::migration::MigrationReadiness,
) -> AccountPreparation {
    use switcher_core::migration::DependencyAssessment;
    let mut blockers = preparation_blockers(readiness.blockers);
    if !matches!(
        readiness.keychain.dependency_assessment,
        DependencyAssessment::HandlingPlanAvailable | DependencyAssessment::VerifiedPreserved
    ) && !blockers
        .iter()
        .any(|item| item.code == "keychain_dependencies_unresolved")
    {
        blockers.push(PreparationBlocker {
            code: "keychain_dependencies_unresolved".into(),
            message: "当前 Claude 版本与 Keychain 依赖的处理方式尚未确定".into(),
            recovery: "核对当前 Claude 版本与 Keychain 依赖；处理方式确定后仍需实际登录验收。"
                .into(),
        });
    }
    AccountPreparation {
        ready: readiness.ready && blockers.is_empty(),
        backup_allowed: false,
        blockers,
        in_place_action: None,
        in_place_ready: false,
        in_place_blockers: Vec::new(),
    }
}

/// Derive the only applicable fixed action from inspected account state.
pub fn in_place_action_for_status(
    status: &switcher_core::accounts::AccountStatus,
) -> Option<InPlaceAction> {
    if status.current.is_some() {
        None
    } else if status.a_saved {
        Some(InPlaceAction::AssociateA)
    } else {
        Some(InPlaceAction::InitializeA)
    }
}

/// Scoped readiness never changes the global directory-exchange gate.
pub fn with_in_place_preparation(
    mut view: AccountPreparation,
    action: Option<InPlaceAction>,
    scoped: Option<switcher_core::migration::MigrationReadiness>,
) -> AccountPreparation {
    if let (Some(action), Some(scoped)) = (action, scoped) {
        view.in_place_blockers = preparation_blockers(scoped.blockers);
        view.in_place_ready = scoped.ready && view.in_place_blockers.is_empty();
        view.in_place_action = Some(action);
    }
    view
}

fn preparation_blockers(
    blockers: Vec<switcher_core::migration::MigrationBlocker>,
) -> Vec<PreparationBlocker> {
    blockers
        .into_iter()
        .map(|item| {
            let recovery = match item.code.as_str() {
                "installation_unverified" => {
                    "先完成正式系统组件安装与后台健康验证，再预览准备状态。"
                }
                "claude_running_or_unknown" => {
                    "完全退出 Claude；无法确认进程状态时先排查，再重试。"
                }
                "claude_application_unverified" => {
                    "核对官方 Claude 应用签名与版本；保留现有资料，不绕过应用核验。"
                }
                "association_source_mismatch" => {
                    "当前资料与已存 A 不一致；保留两个来源，不覆盖档案，先核对恢复方案。"
                }
                "account_recovery_pending" => "先恢复中断的账号事务，再重新预览原位准备状态。",
                "intent_state_mismatch" => {
                    "重新读取本机账号状态；当前操作不适用于已关联或不一致的档案。"
                }
                "repair_recovery_pending" => "先从地区设置恢复修复事务，再重新核对。",
                "backup_missing" => "满足安装与进程门禁后，由本人点击创建并验证迁移备份。",
                "backup_stale_or_unverified" | "backup_receipt_invalid" => {
                    "保留原备份；重新创建并验证迁移备份，不手改收据。"
                }
                "backup_waits_installation" => {
                    "完成正式安装并退出 Claude 后，重新核对现有备份收据。"
                }
                "keychain_dependencies_unresolved" => {
                    "核对当前 Claude 版本、共享材料与设备状态的处理方式；元数据可读不等于登录状态已保留。"
                }
                _ => "保留现有资料，检查恢复入口；未识别状态不能视为放行。",
            };
            PreparationBlocker {
                code: item.code,
                message: diagnostics::redact(&item.message),
                recovery: recovery.into(),
            }
        })
        .collect()
}

#[derive(Default)]
struct CheckSessionState {
    next: u64,
    active: Option<u64>,
    report: Option<CheckReport>,
}
#[derive(Default)]
struct CheckSessions(Mutex<CheckSessionState>);
struct CheckSession<'a> {
    sessions: &'a CheckSessions,
    epoch: u64,
}
impl CheckSessions {
    fn begin(&self) -> Result<CheckSession<'_>, String> {
        self.begin_initialized(|| {})
    }
    fn begin_initialized(&self, initialized: impl FnOnce()) -> Result<CheckSession<'_>, String> {
        let mut state = self.0.lock().map_err(|_| "检查状态不可用")?;
        if state.active.is_some() {
            return Err("环境检查正在进行".into());
        }
        state.next = state.next.checked_add(1).ok_or("检查代次已耗尽")?;
        let epoch = state.next;
        state.active = Some(epoch);
        state.report = None;
        initialized();
        Ok(CheckSession {
            sessions: self,
            epoch,
        })
    }
    fn report(&self) -> Result<Option<CheckReport>, String> {
        Ok(self.0.lock().map_err(|_| "检查状态不可用")?.report.clone())
    }
    fn replace(&self, report: Option<CheckReport>) -> Result<(), String> {
        let mut state = self.0.lock().map_err(|_| "检查状态不可用")?;
        state.active = None;
        state.report = report;
        Ok(())
    }
    fn cancel(&self) -> Result<bool, String> {
        let mut state = self.0.lock().map_err(|_| "检查状态不可用")?;
        let running = state.active.take().is_some();
        state.report = None;
        Ok(running)
    }
}
impl CheckSession<'_> {
    fn active(&self) -> bool {
        self.sessions
            .0
            .lock()
            .is_ok_and(|state| state.active == Some(self.epoch))
    }
    async fn wait_cancelled(&self) {
        while self.active() {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }
    fn publish(&self, report: CheckReport) -> Result<(), String> {
        let mut state = self.sessions.0.lock().map_err(|_| "检查状态不可用")?;
        if state.active != Some(self.epoch) {
            return Err("本次检查已取消或失效，未保存报告".into());
        }
        state.report = Some(report);
        Ok(())
    }
}
impl Drop for CheckSession<'_> {
    fn drop(&mut self) {
        if let Ok(mut state) = self.sessions.0.lock()
            && state.active == Some(self.epoch)
        {
            state.active = None;
        }
    }
}

#[cfg(test)]
mod check_session_tests {
    use super::*;
    use std::sync::{Barrier, mpsc};
    fn report() -> CheckReport {
        CheckReport {
            schema_version: 1,
            observed_at: 1,
            completed_at: 2,
            duration_ms: 1,
            network_generation: "synthetic".into(),
            checks: vec![],
            summary: switcher_domain::checks::CheckSummary {
                passed: 0,
                advisory: 0,
                blocking: 14,
            },
            launch: switcher_domain::checks::LaunchDecision {
                allowed: false,
                reasons: vec![],
            },
        }
    }
    #[test]
    fn check_session_cancel_during_begin_initialization_cannot_be_overwritten() {
        let sessions = CheckSessions::default();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (attempt_tx, attempt_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        std::thread::scope(|scope| {
            let sessions = &sessions;
            scope.spawn(move || {
                ready_rx.recv().unwrap();
                attempt_tx.send(()).unwrap();
                done_tx.send(sessions.cancel()).unwrap();
            });
            let session = sessions
                .begin_initialized(|| {
                    ready_tx.send(()).unwrap();
                    attempt_rx.recv().unwrap();
                    // The cancel call is now contending with the actual begin lock.
                    assert!(matches!(done_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
                })
                .unwrap();
            assert!(done_rx.recv().unwrap().unwrap());
            assert!(!session.active());
            assert!(session.publish(report()).is_err());
            assert!(sessions.report().unwrap().is_none());
        });
    }
    #[test]
    fn check_session_simultaneously_ready_report_and_cancel_never_leave_a_report() {
        let sessions = CheckSessions::default();
        let session = sessions.begin().unwrap();
        let ready = Barrier::new(3);
        std::thread::scope(|scope| {
            let publish = scope.spawn(|| {
                ready.wait();
                session.publish(report())
            });
            let cancel = scope.spawn(|| {
                ready.wait();
                sessions.cancel()
            });
            ready.wait();
            let _ = publish.join().unwrap();
            assert!(cancel.join().unwrap().unwrap());
        });
        assert!(sessions.report().unwrap().is_none());
        assert!(!session.active());
        assert!(session.publish(report()).is_err());
    }
    #[test]
    fn check_session_late_cancel_clears_published_report_and_rejects_late_publish() {
        let sessions = CheckSessions::default();
        let session = sessions.begin().unwrap();
        session.publish(report()).unwrap();
        assert!(sessions.report().unwrap().is_some());
        assert!(sessions.cancel().unwrap());
        assert!(sessions.report().unwrap().is_none());
        assert!(session.publish(report()).is_err());
    }
    #[test]
    fn check_session_old_drop_does_not_release_new_check_and_drop_always_releases_own() {
        let sessions = CheckSessions::default();
        let old = sessions.begin().unwrap();
        assert!(sessions.begin().is_err());
        assert!(sessions.cancel().unwrap());
        let new = sessions.begin().unwrap();
        drop(old);
        assert!(new.active());
        assert!(sessions.begin().is_err());
        new.publish(report()).unwrap();
        drop(new);
        let next = sessions.begin().unwrap();
        assert!(sessions.report().unwrap().is_none());
        drop(next);
        assert!(sessions.begin().is_ok());
    }
}
