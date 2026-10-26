//! User-confirmed region changes. IPC accepts a preset; CLI detection uses a verified target.
use crate::{
    config::{self, CheckConfig},
    install,
    proxy_repair::{self, AppOpenPlan, Phase, Runtime, RuntimeState},
    storage,
    user_repairs::{
        PreparedUserRepair, ProxyRepairMode, UserLocale, UserRepairCoordinator, UserRepairReceipt,
        UserRepairState, UserRepairTarget,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use switcher_domain::{
    Edition,
    desktop::{RegionPreset, RegionView, RepairChange, RepairPreview},
    system::{HelperResponse, Ipv6Policy, RegionPolicy, RepairPlan, SystemState},
};

mod legacy;

pub struct PreparedRepair {
    legacy_region: Option<bool>,
    plan_id: String,
    original_config: Vec<u8>,
    proposed_config: CheckConfig,
    user_plan: Option<PreparedUserRepair>,
    system_plan: Option<RepairPlan>,
    app_open_plan: Option<AppOpenPlan>,
    pub preview: RepairPreview,
}

pub struct RegionCoordinator {
    home: PathBuf,
    edition: Edition,
}

enum RegionTarget {
    Current,
    Selected(RegionView),
}

impl From<Option<RegionPreset>> for RegionTarget {
    fn from(region: Option<RegionPreset>) -> Self {
        region.map_or(Self::Current, |region| Self::Selected(preset(region)))
    }
}

impl From<crate::region_detection::DetectedRegion> for RegionTarget {
    fn from(region: crate::region_detection::DetectedRegion) -> Self {
        Self::Selected(region.into_view())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RepairStage {
    UserPending,
    SystemPending,
    CommitIntent,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RecoveryOutcome {
    NoWork,
    Restored,
    Committed,
}

/// One durable decision spans the user journal, root receipt and local target.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegionJournal {
    schema: u8,
    owner: u32,
    edition: Edition,
    stage: RepairStage,
    original: CheckConfig,
    proposed: CheckConfig,
    plan: RepairPlan,
    user_required: bool,
    receipt: Option<UserRepairReceipt>,
    #[serde(default)]
    runtime_state: Option<RuntimeState>,
    #[serde(default)]
    app_open_plan: Option<AppOpenPlan>,
}

pub fn recovery_pending(home: &Path) -> Result<bool, &'static str> {
    recovery_pending_v2(&home.join(".claude-switcher/v2"))
}

pub fn recovery_pending_v2(v2: &Path) -> Result<bool, &'static str> {
    ["region-repair.json", "user-repairs/journal.json"]
        .iter()
        .try_fold(false, |pending, name| {
            match fs::symlink_metadata(v2.join(name)) {
                Ok(_) => Ok(true),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(pending),
                Err(_) => Err("无法读取修复事务状态"),
            }
        })
}

fn state_matches(state: &SystemState, policy: &RegionPolicy) -> bool {
    state.timezone == policy.timezone
        && state.network_service == policy.network_service
        && state.ipv6 == policy.ipv6
}

fn original_matches(state: &SystemState, original: &SystemState) -> bool {
    state.timezone == original.timezone
        && state.network_service == original.network_service
        && state.ipv6 == original.ipv6
}

enum Resolution {
    Applied,
    Restored,
    Unknown,
}

fn resolution(journal: &RegionJournal, response: &HelperResponse) -> Resolution {
    let Some(plan) = response.plan.as_ref() else {
        return Resolution::Unknown;
    };
    let Some(state) = response.state.as_ref() else {
        return Resolution::Unknown;
    };
    if plan.id != journal.plan.id
        || !state_matches(
            &SystemState {
                timezone: plan.target.timezone.clone(),
                network_service: plan.target.network_service.clone(),
                ipv6: plan.target.ipv6,
            },
            &journal.plan.target,
        )
        || !original_matches(&plan.original, &journal.plan.original)
    {
        return Resolution::Unknown;
    }
    match response.code.as_str() {
        "repair_applied" | "applied"
            if response.ok && state_matches(state, &journal.plan.target) =>
        {
            Resolution::Applied
        }
        "repair_restored" | "repair_not_started"
            if original_matches(state, &journal.plan.original) =>
        {
            Resolution::Restored
        }
        _ => Resolution::Unknown,
    }
}

pub fn preset(region: RegionPreset) -> RegionView {
    let (country, timezone, locale) = match region {
        RegionPreset::Us => ("US", "America/Los_Angeles", "en_US"),
        RegionPreset::Jp => ("JP", "Asia/Tokyo", "ja_JP"),
        RegionPreset::Tw => ("TW", "Asia/Taipei", "zh_TW"),
        RegionPreset::Hk => ("HK", "Asia/Hong_Kong", "zh_HK"),
        RegionPreset::Sg => ("SG", "Asia/Singapore", "en_SG"),
        RegionPreset::Gb => ("GB", "Europe/London", "en_GB"),
        RegionPreset::De => ("DE", "Europe/Berlin", "de_DE"),
        RegionPreset::Fr => ("FR", "Europe/Paris", "fr_FR"),
        RegionPreset::Kr => ("KR", "Asia/Seoul", "ko_KR"),
        RegionPreset::Au => ("AU", "Australia/Sydney", "en_AU"),
        RegionPreset::Ca => ("CA", "America/Toronto", "en_CA"),
    };
    RegionView {
        country: country.into(),
        timezone: timezone.into(),
        locale: locale.into(),
    }
}

fn view(config: &CheckConfig) -> RegionView {
    RegionView {
        country: config
            .country_target
            .clone()
            .unwrap_or_else(|| "未设置".into()),
        timezone: config
            .timezone_target
            .clone()
            .unwrap_or_else(|| "未设置".into()),
        locale: config
            .locale_target
            .clone()
            .unwrap_or_else(|| "未设置".into()),
    }
}
fn ipv6(value: Ipv6Policy) -> String {
    match value {
        Ipv6Policy::Off => "关闭",
        Ipv6Policy::Automatic => "自动",
    }
    .into()
}
fn encode(config: &CheckConfig) -> Result<Vec<u8>, &'static str> {
    serde_json::to_vec(config).map_err(|_| "地区配置编码失败")
}

impl RegionCoordinator {
    pub fn for_current_user(edition: Edition) -> Result<Self, &'static str> {
        Ok(Self {
            home: crate::migration::account_home(unsafe { libc::geteuid() })
                .map_err(|_| "无法核对系统用户主目录")?,
            edition,
        })
    }
    pub fn view(&self) -> Result<RegionView, &'static str> {
        Ok(view(&config::load_read_only(&self.home)?))
    }
    pub fn preview(&self, region: Option<RegionPreset>) -> Result<PreparedRepair, &'static str> {
        self.preview_legacy(region.into())
    }
    pub fn preview_detected(
        &self,
        region: crate::region_detection::DetectedRegion,
    ) -> Result<PreparedRepair, &'static str> {
        self.preview_legacy(region.into())
    }
    fn preview_legacy(&self, region: RegionTarget) -> Result<PreparedRepair, &'static str> {
        if recovery_pending(&self.home)? {
            return self.preview_target(None);
        }
        let current = config::load_read_only(&self.home)?;
        let mut proposed = current.clone();
        let selected = matches!(&region, RegionTarget::Selected(_));
        if let RegionTarget::Selected(target) = region {
            proposed.country_target = Some(target.country);
            proposed.timezone_target = Some(target.timezone);
            proposed.locale_target = Some(target.locale);
        }
        proposed.validate()?;
        Ok(PreparedRepair {
            legacy_region: Some(selected), plan_id: String::new(),
            original_config: encode(&current)?, proposed_config: proposed.clone(),
            user_plan: None, system_plan: None, app_open_plan: None,
            preview: RepairPreview {
                can_execute: true, recovery_required: false,
                message: if selected { "先保存国家、时区和地区格式目标；再按需授权时区和看门狗。取消授权会保留已更新目标。" } else { "先恢复代理并核对 TUN 与出口；只在系统项目需要修复时授权。授权取消后仍继续用户项目。" }.into(),
                original: view(&current), target: view(&proposed), network_service: proposed.network_service.clone(),
                original_ipv6: "执行时读取".into(), target_ipv6: if selected { "保持原值" } else { "关闭" }.into(), user_changes: Vec::new(),
            },
        })
    }
    fn preview_target(
        &self,
        region: impl Into<RegionTarget>,
    ) -> Result<PreparedRepair, &'static str> {
        let mut prepared = self.preview_validated_with(
            region,
            || {
                install::validate_installed_identity(self.edition, unsafe { libc::geteuid() })
                    .map(|_| ())
            },
            || install::validate_installed_health(unsafe { libc::geteuid() }),
            install::preview_repair,
        )?;
        if !prepared.preview.recovery_required {
            crate::guardian_repair::validate(self.edition)?;
            if prepared.proposed_config.proxy_mode == "clash-verge" {
                proxy_repair::Native.running()?;
                prepared.preview.user_changes.push(RepairChange {
                    id: "proxy_runtime".into(), state: "ready".into(),
                    original: "当前用户的固定 Clash Verge".into(),
                    target: "重启并验证 TUN 与目标国家".into(),
                    message: "管理员确认后按需退出、保存并重启；失败恢复原运行状态。尚不验证实际加载的置顶规则。".into(),
                });
            }
            self.preview_app_open_with(&mut prepared, proxy_repair::prepare_app_open)?;
        }
        Ok(prepared)
    }
    fn preview_app_open_with(
        &self,
        prepared: &mut PreparedRepair,
        prepare: impl FnOnce(ProxyRepairMode) -> Result<Option<AppOpenPlan>, &'static str>,
    ) -> Result<(), &'static str> {
        let mode = ProxyRepairMode::parse(&prepared.proposed_config.proxy_mode)
            .ok_or("代理类型不在支持列表")?;
        prepared.app_open_plan = None;
        let result = prepare(mode);
        let (state, message) = match result {
            Ok(Some(mut plan)) => {
                plan.limit_expiry(
                    prepared
                        .system_plan
                        .as_ref()
                        .ok_or("系统修复计划缺失")?
                        .expires_at,
                );
                prepared.app_open_plan = Some(plan);
                (
                    "ready",
                    "管理员确认后复核同一计划；停止时仅打开固定 Surge.app。不会连接 VPN、修改规则或证明网络就绪；失败请手动处理。",
                )
            }
            Ok(None) => return Ok(()),
            Err(_) => (
                "unsupported",
                if mode == ProxyRepairMode::Surge {
                    "固定 Surge.app 缺失、身份或运行形态未通过验证；请手动处理。其他列出的配置修复仍可执行。"
                } else {
                    "Shadowrocket／ClashX 官方发布者身份尚未验证；本轮只支持 Surge 自动打开。请手动打开当前客户端并检查节点、线路及 TUN。"
                },
            ),
        };
        prepared.preview.user_changes.push(RepairChange {
            id: "proxy_app_open".into(),
            state: state.into(),
            original: "当前客户端与运行状态需按计划复核".into(),
            target: if mode == ProxyRepairMode::Surge {
                "/Applications/Surge.app（仅打开 App）"
            } else {
                "手动打开已选代理客户端"
            }
            .into(),
            message: message.into(),
        });
        Ok(())
    }
    fn preview_validated_with(
        &self,
        region: impl Into<RegionTarget>,
        identity: impl FnOnce() -> Result<(), &'static str>,
        health: impl FnOnce() -> Result<(), &'static str>,
        request: impl FnOnce(RegionPolicy) -> Result<HelperResponse, &'static str>,
    ) -> Result<PreparedRepair, &'static str> {
        // Hold the lifecycle capability across pending detection and plan construction.
        let _lock = storage::LifecycleLock::try_acquire(&self.home.join(".claude-switcher/v2"))
            .map_err(|_| "其他账号、修复或安装事务正在进行")?;
        let pending = recovery_pending(&self.home)?;
        identity().map_err(|_| "正式系统组件尚未安装并验证；当前设置保持原值")?;
        if pending {
            // Recovery never needs a new root plan or a valid current configuration.
            // An invalid journal remains visible, but recovery still validates it before writes.
            let journal = if self.has_region_journal()? {
                Some(self.read_journal())
            } else {
                None
            };
            let mut message = match &journal {
                Some(Ok(_)) => "上次修复尚未完成。恢复会先查询系统组件的持久结果，再完成提交或恢复原值；结果未知时保留现场。",
                Some(Err(_)) => "地区修复记录无法验证。已保留全部事务与备份；恢复仍需验证记录，无法验证时不会修改设置。",
                None => "上次用户配置修复尚未提交；请先恢复，再重新预览。恢复会验证记录，无法验证时保留现场。",
            }.to_string();
            let (original, proposed, plan, configuration_error) = match journal {
                Some(Ok(journal)) => (journal.original, journal.proposed, Some(journal.plan), None),
                _ => match config::load_read_only(&self.home) {
                    Ok(current) => (current.clone(), current, None, None),
                    Err(error) => {
                        // Internal placeholders only: this preview cannot execute, and the
                        // values below are never projected as confirmed current settings.
                        (
                            CheckConfig::default(),
                            CheckConfig::default(),
                            None,
                            Some(error),
                        )
                    }
                },
            };
            let (original_view, target_view, network_service) =
                if let Some(error) = configuration_error {
                    message.push_str(&format!(
                        " 当前配置无法读取（{error}）；原值与目标均未确认。"
                    ));
                    let unknown = || RegionView {
                        country: "未确认".into(),
                        timezone: "未确认".into(),
                        locale: "未确认".into(),
                    };
                    (unknown(), unknown(), "未确认".into())
                } else {
                    (
                        view(&original),
                        view(&proposed),
                        proposed.network_service.clone(),
                    )
                };
            return Ok(PreparedRepair {
                legacy_region: None,
                plan_id: plan
                    .as_ref()
                    .map(|plan| plan.id.clone())
                    .unwrap_or_default(),
                original_config: encode(&original)?,
                proposed_config: proposed.clone(),
                user_plan: None,
                app_open_plan: None,
                preview: RepairPreview {
                    can_execute: false,
                    recovery_required: true,
                    message,
                    original: original_view,
                    target: target_view,
                    network_service,
                    original_ipv6: plan
                        .as_ref()
                        .map(|plan| ipv6(plan.original.ipv6))
                        .unwrap_or_else(|| "未确认".into()),
                    target_ipv6: plan
                        .as_ref()
                        .map(|plan| ipv6(plan.target.ipv6))
                        .unwrap_or_else(|| "未确认".into()),
                    user_changes: Vec::new(),
                },
                system_plan: plan,
            });
        }
        let guardian_healthy = health().is_ok();
        let mut prepared = self.preview_with(region, request)?;
        let target = UserRepairTarget {
            locale: UserLocale::parse(
                prepared
                    .proposed_config
                    .locale_target
                    .as_deref()
                    .ok_or("请先选择地区格式")?,
            )
            .ok_or("地区格式不在支持列表")?,
            proxy: ProxyRepairMode::parse(&prepared.proposed_config.proxy_mode)
                .ok_or("代理类型不在支持列表")?,
        };
        let user_plan = UserRepairCoordinator::for_current_user(self.edition)
            .map_err(|_| "无法确认用户修复目录")?
            .preview(target)
            .map_err(|_| "用户配置无法安全预览；原设置保持不变")?;
        prepared.preview.user_changes = user_plan
            .preview
            .items
            .iter()
            .map(|item| RepairChange {
                id: item.id.into(),
                state: match item.state {
                    UserRepairState::Ready => "ready",
                    UserRepairState::Unchanged => "unchanged",
                    UserRepairState::Unsupported => "unsupported",
                }
                .into(),
                original: item.original.clone(),
                target: item.target.clone(),
                message: item.message.clone(),
            })
            .collect();
        prepared.preview.user_changes.push(RepairChange {
            id: "guardian_service".into(),
            state: if guardian_healthy {
                "unchanged"
            } else {
                "ready"
            }
            .into(),
            original: if guardian_healthy {
                "守护心跳正常"
            } else {
                "守护心跳缺失或不可用"
            }
            .into(),
            target: "恢复已安装守护服务并复检心跳".into(),
            message: "仅恢复当前用户的固定守护服务；组件损坏需重新运行安装器。".into(),
        });
        prepared.preview.recovery_required = user_plan.preview.recovery_pending;
        prepared.preview.can_execute = !user_plan.preview.recovery_pending;
        prepared.preview.message = if user_plan.preview.recovery_pending {
            "上次用户配置修复尚未提交；请先恢复，再重新预览。"
        } else {
            "请核对原值与目标。仅执行列出的安全操作；不支持的代理项目需手动处理。管理员确认后持续恢复目标时区。执行后重新检查环境。"
        }.into();
        if user_plan.preview.can_execute {
            prepared.user_plan = Some(user_plan);
        }
        Ok(prepared)
    }
    fn preview_with(
        &self,
        region: impl Into<RegionTarget>,
        request: impl FnOnce(RegionPolicy) -> Result<HelperResponse, &'static str>,
    ) -> Result<PreparedRepair, &'static str> {
        let current = config::load_read_only(&self.home)?;
        let mut proposed = current.clone();
        if let RegionTarget::Selected(target) = region.into() {
            proposed.country_target = Some(target.country);
            proposed.timezone_target = Some(target.timezone);
            proposed.locale_target = Some(target.locale);
            proposed.previous_egress_hash = None;
        }
        proposed.validate()?;
        let target = RegionPolicy {
            timezone: proposed.timezone_target.clone().ok_or("请先选择地区目标")?,
            network_service: proposed.network_service.clone(),
            ipv6: Ipv6Policy::Off,
        };
        let response = request(target)?;
        if !response.ok {
            return Err("系统组件拒绝修复预览；请核对安装、身份和服务状态");
        }
        let plan = response.plan.ok_or("系统组件未返回修复计划")?;
        if plan.target.timezone != proposed.timezone_target.as_deref().unwrap_or("")
            || plan.target.network_service != proposed.network_service
            || plan.target.ipv6 != Ipv6Policy::Off
        {
            return Err("系统组件返回的计划与已选目标不一致");
        }
        let mut original = view(&current);
        original.timezone = plan.original.timezone.clone();
        Ok(PreparedRepair {
            legacy_region: None,
            plan_id: plan.id.clone(),
            system_plan: Some(plan.clone()),
            original_config: encode(&current)?,
            preview: RepairPreview {
                can_execute: true,
                message: "请核对系统原值与目标。执行后需重新检查出口；地区格式在系统设置中确认。"
                    .into(),
                original,
                target: view(&proposed),
                network_service: proposed.network_service.clone(),
                original_ipv6: ipv6(plan.original.ipv6),
                target_ipv6: ipv6(plan.target.ipv6),
                user_changes: Vec::new(),
                recovery_required: false,
            },
            proposed_config: proposed,
            user_plan: None,
            app_open_plan: None,
        })
    }
    /// Consumes the in-memory plan. A failed or stale plan requires another preview.
    pub fn execute(&self, prepared: PreparedRepair) -> Result<(), &'static str> {
        if prepared.legacy_region.is_some() {
            return legacy::execute(self, prepared);
        }
        crate::guardian_repair::validate(self.edition).map_err(|_| "正式系统组件尚未安装并验证")?;
        self.execute_authorized_with(
            prepared,
            install::authorize_repair_with_lock,
            install::execute_repair_with_lock,
        )?;
        // execute_authorized_with has returned and dropped its lifecycle lock.
        crate::guardian_repair::restore_after_commit(self.edition)
    }
    #[cfg(test)]
    fn execute_with(
        &self,
        prepared: PreparedRepair,
        execute: impl FnOnce(String, &storage::LifecycleLock) -> Result<HelperResponse, &'static str>,
    ) -> Result<(), &'static str> {
        self.execute_authorized_with(prepared, |_, _| Ok(()), execute)
    }
    fn execute_authorized_with(
        &self,
        prepared: PreparedRepair,
        authorize: impl FnOnce(&str, &storage::LifecycleLock) -> Result<(), &'static str>,
        execute: impl FnOnce(String, &storage::LifecycleLock) -> Result<HelperResponse, &'static str>,
    ) -> Result<(), &'static str> {
        self.execute_runtime_with(prepared, authorize, execute, &mut proxy_repair::Native)
    }
    fn execute_runtime_with(
        &self,
        prepared: PreparedRepair,
        authorize: impl FnOnce(&str, &storage::LifecycleLock) -> Result<(), &'static str>,
        execute: impl FnOnce(String, &storage::LifecycleLock) -> Result<HelperResponse, &'static str>,
        runtime: &mut dyn Runtime,
    ) -> Result<(), &'static str> {
        let original_config = prepared.original_config.clone();
        self.execute_runtime_and_app_with(prepared, authorize, execute, runtime, |plan| {
            install::validate_installed_identity(self.edition, unsafe { libc::geteuid() })?;
            crate::guardian_repair::validate(self.edition)?;
            if encode(&config::load_read_only(&self.home)?)? != original_config {
                return Err("安装身份复核期间配置已改变；请重新预览");
            }
            proxy_repair::execute_app_open(plan)
        })
    }
    fn execute_runtime_and_app_with(
        &self,
        prepared: PreparedRepair,
        authorize: impl FnOnce(&str, &storage::LifecycleLock) -> Result<(), &'static str>,
        execute: impl FnOnce(String, &storage::LifecycleLock) -> Result<HelperResponse, &'static str>,
        runtime: &mut dyn Runtime,
        app_open: impl FnOnce(&AppOpenPlan) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        let v2 = self.home.join(".claude-switcher/v2");
        if !prepared.preview.can_execute {
            return Err("修复计划不可执行，请先恢复并重新预览");
        }
        if prepared.app_open_plan.is_some() && prepared.proposed_config.proxy_mode != "surge" {
            return Err("App 打开计划与代理模式不一致，请重新预览");
        }
        let system_plan = prepared.system_plan.ok_or("系统修复计划缺失，请重新预览")?;
        let lock = storage::LifecycleLock::try_acquire(&v2)
            .map_err(|_| "其他账号、修复或安装事务正在进行")?;
        if encode(&config::load_read_only(&self.home)?)? != prepared.original_config {
            return Err("地区配置已改变，请重新预览");
        }
        if recovery_pending(&self.home)? {
            return Err("上次修复尚未完成，请先恢复事务");
        }
        // The lock remains held across the native dialog and helper's independent verification.
        // Cancellation occurs before journal creation, preferences, proxy config or target writes.
        authorize(&prepared.plan_id, &lock).map_err(|_| "管理员授权取消或未通过；系统、代理、地区格式和语言均未修改，原配置已保留；请重新预览")?;
        if encode(&config::load_read_only(&self.home)?)? != prepared.original_config {
            return Err("授权期间地区配置已改变；尚未写入，请重新预览");
        }
        if system_plan.expires_at <= crate::checks::unix_now() {
            return Err("授权后修复计划已过期；尚未写入，请重新预览");
        }
        let runtime_state = if prepared.proposed_config.proxy_mode == "clash-verge" {
            Some(RuntimeState {
                adapter: proxy_repair::Adapter::ClashVerge,
                originally_running: runtime.running()?,
                reload_required: prepared
                    .user_plan
                    .as_ref()
                    .is_some_and(PreparedUserRepair::changes_proxy_files),
                phase: Phase::Intent,
            })
        } else {
            None
        };
        let user = self.user_coordinator(prepared.user_plan.is_some())?;
        let mut journal = RegionJournal {
            schema: 1,
            owner: unsafe { libc::geteuid() },
            edition: self.edition,
            stage: RepairStage::UserPending,
            original: serde_json::from_slice(&prepared.original_config)
                .map_err(|_| "原地区配置无效")?,
            proposed: prepared.proposed_config,
            plan: system_plan,
            user_required: prepared.user_plan.is_some(),
            receipt: None,
            runtime_state,
            app_open_plan: prepared.app_open_plan,
        };
        self.save_journal(&journal)?;
        // App dispatch has durable intent, but occurs before any user configuration write.
        // A crash or recovery never repeats this action and never stops this App.
        if let Some(plan) = &journal.app_open_plan {
            if app_open(plan).is_err() {
                // Dispatch may already have reached the OS. Only explicit recovery clears this intent.
                return Err(
                    "App 打开预检失败或结果未确认；未写入用户配置，已保留事务，请先恢复，再手动检查代理并重新预览",
                );
            }
            let current = config::load_read_only(&self.home).and_then(|value| encode(&value));
            if journal.plan.expires_at <= crate::checks::unix_now()
                || current.as_deref() != Ok(prepared.original_config.as_slice())
            {
                return Err(
                    "App 打开后计划过期或配置无法复核；未写入用户配置，已保留事务，请先恢复，再手动检查代理并重新预览",
                );
            }
        }
        let preparation = (|| {
            if let Some(state) = &journal.runtime_state
                && (state.reload_required || !state.originally_running)
            {
                runtime.stop()?;
                self.runtime_phase(&mut journal, Phase::Stopped)?;
            }
            if let (Some(coordinator), Some(plan)) = (user.as_ref(), prepared.user_plan) {
                journal.receipt = Some(
                    coordinator
                        .execute_with_lock(plan, &lock)
                        .map_err(|_| "用户配置修复中断")?,
                );
                self.save_journal(&journal)?;
            }
            if let Some(state) = &journal.runtime_state {
                if state.reload_required || !state.originally_running {
                    self.runtime_phase(&mut journal, Phase::StartingTarget)?;
                    runtime.start()?;
                }
                runtime.verify(
                    journal
                        .proposed
                        .country_target
                        .as_deref()
                        .ok_or("目标国家缺失")?,
                )?;
                self.runtime_phase(&mut journal, Phase::TargetReady)?;
            }
            if let (Some(user), Some(receipt)) = (user.as_ref(), journal.receipt.as_ref()) {
                user.verify_receipt_with_lock(receipt, &lock, false)
                    .map_err(|_| "代理启动后配置发生变化；未执行系统修改")?;
            }
            if journal.plan.expires_at <= crate::checks::unix_now() {
                return Err("代理验证后原授权计划已过期；请重新预览");
            }
            Ok(())
        })();
        if let Err(error) = preparation {
            if journal.runtime_state.is_none() {
                return Err("用户配置修复中断；请先恢复事务");
            }
            self.restore_runtime(&mut journal, user.as_ref(), &lock, runtime)?;
            return Err(error);
        }
        journal.stage = RepairStage::SystemPending;
        self.save_journal(&journal)?;
        // A missing reply cannot cancel root. Keep both journals until its result is known.
        let response = execute(prepared.plan_id, &lock)
            .map_err(|_| "系统修复结果未知；已保留事务。请使用恢复入口查询结果")?;
        match resolution(&journal, &response) {
            Resolution::Applied => self.commit_runtime(&mut journal, user.as_ref(), &lock, runtime),
            Resolution::Restored => {
                self.restore_runtime(&mut journal, user.as_ref(), &lock, runtime)?;
                Err("系统修复未完成，原用户配置已恢复；请重新预览")
            }
            Resolution::Unknown => Err("系统修复结果尚未确认；已保留事务，请先恢复"),
        }
    }
    fn runtime_phase(&self, journal: &mut RegionJournal, phase: Phase) -> Result<(), &'static str> {
        if let Some(state) = &mut journal.runtime_state {
            state.phase = phase;
        }
        self.save_journal(journal)
    }
    fn journal_path(&self) -> PathBuf {
        self.home.join(".claude-switcher/v2/region-repair.json")
    }
    fn has_region_journal(&self) -> Result<bool, &'static str> {
        match fs::symlink_metadata(self.journal_path()) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(_) => Err("无法读取地区修复状态"),
        }
    }
    fn save_journal(&self, journal: &RegionJournal) -> Result<(), &'static str> {
        storage::atomic_json(&self.journal_path(), journal)
            .map_err(|_| "地区修复记录保存失败；请保留现场并恢复")
    }
    fn read_journal(&self) -> Result<RegionJournal, &'static str> {
        let journal: RegionJournal = storage::read_json(&self.journal_path())
            .map_err(|_| "地区修复记录无法安全读取；请保留备份")?;
        journal.original.validate()?;
        journal.proposed.validate()?;
        if journal.runtime_state.is_some()
            && (journal.proposed.proxy_mode != "clash-verge"
                || journal.original.proxy_mode != "clash-verge")
        {
            return Err("代理运行记录与配置不一致");
        }
        if journal.app_open_plan.is_some()
            && (journal.proposed.proxy_mode != "surge"
                || journal.original.proxy_mode != "surge"
                || journal.runtime_state.is_some())
        {
            return Err("App 打开记录与代理配置不一致");
        }
        if journal.schema != 1
            || journal.owner != unsafe { libc::geteuid() }
            || journal.edition != self.edition
            || journal.plan.id.is_empty()
            || journal.plan.id.len() > 160
            || journal.plan.target.timezone
                != journal.proposed.timezone_target.as_deref().unwrap_or("")
            || journal.plan.target.network_service != journal.proposed.network_service
            || journal.plan.original.network_service != journal.plan.target.network_service
            || journal.plan.target.ipv6 != Ipv6Policy::Off
            || (journal.stage != RepairStage::UserPending
                && journal.user_required
                && journal.receipt.is_none())
        {
            return Err("地区修复记录与当前用户或目标不一致");
        }
        Ok(journal)
    }
    fn remove_journal(&self) -> Result<(), &'static str> {
        fs::remove_file(self.journal_path()).map_err(|_| "地区修复记录清理失败，请重试恢复")?;
        storage::sync_directory(&self.home.join(".claude-switcher/v2"))
            .map_err(|_| "地区修复记录同步未完成，请重试恢复")
    }
    fn user_coordinator(
        &self,
        required: bool,
    ) -> Result<Option<UserRepairCoordinator>, &'static str> {
        if required {
            UserRepairCoordinator::for_current_user(self.edition)
                .map(Some)
                .map_err(|_| "无法确认用户修复目录")
        } else {
            Ok(None)
        }
    }
    fn commit_runtime(
        &self,
        journal: &mut RegionJournal,
        user: Option<&UserRepairCoordinator>,
        lock: &storage::LifecycleLock,
        runtime: &mut dyn Runtime,
    ) -> Result<(), &'static str> {
        let current = encode(&config::load_read_only(&self.home)?)?;
        let target = encode(&journal.proposed)?;
        if current != encode(&journal.original)? && current != target {
            return Err("地区目标发生外部变化；请保留事务并核对冲突");
        }
        // Persist the decision before either local target or user journal is committed.
        journal.stage = RepairStage::CommitIntent;
        self.save_journal(journal)?;
        let config_path = self.home.join(".claude-switcher/v2/config.json");
        let missing = match fs::symlink_metadata(&config_path) {
            Ok(metadata) if metadata.is_file() => false,
            Ok(_) => return Err("已提交目标配置路径不安全；请保留事务"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(_) => return Err("无法确认已提交目标配置路径；请保留事务"),
        };
        // Legacy fallback can equal the target without a durable v2 policy for Guardian.
        if current != target || missing {
            storage::atomic_json(&config_path, &journal.proposed)
                .map_err(|_| "系统修复已完成；地区目标保存中断，请恢复事务继续提交")?;
        }
        if let (Some(user), Some(receipt)) = (user, journal.receipt.as_ref()) {
            user.commit_with_lock(receipt, lock)
                .map_err(|_| "系统与地区目标已提交；用户配置提交待恢复")?;
        }
        if journal.runtime_state.is_some() {
            let country = journal
                .proposed
                .country_target
                .as_deref()
                .ok_or("目标国家缺失")?;
            if runtime.verify(country).is_err() {
                // Applied is irreversible here: repair the target runtime, never original files.
                runtime
                    .stop()
                    .and_then(|_| runtime.start())
                    .and_then(|_| runtime.verify(country))
                    .map_err(|_| "系统与用户配置已提交；代理运行状态尚未恢复，已保留事务")?;
            }
        }
        if let (Some(user), Some(receipt)) = (user, journal.receipt.as_ref()) {
            user.verify_receipt_with_lock(receipt, lock, false)
                .map_err(|_| "系统已提交，但代理启动后配置发生变化；已保留事务")?;
        }
        self.remove_journal()
    }
    fn restore_runtime(
        &self,
        journal: &mut RegionJournal,
        user: Option<&UserRepairCoordinator>,
        lock: &storage::LifecycleLock,
        runtime: &mut dyn Runtime,
    ) -> Result<(), &'static str> {
        if journal.stage == RepairStage::CommitIntent {
            return Err("事务已有提交决议；请核对系统状态，不能回滚用户配置");
        }
        if encode(&config::load_read_only(&self.home)?)? != encode(&journal.original)? {
            return Err("地区目标发生外部变化；请保留事务并核对冲突");
        }
        let files_restored = journal
            .runtime_state
            .as_ref()
            .is_some_and(|state| state.phase == Phase::RestoringRuntime);
        if journal.runtime_state.is_some() && !files_restored {
            self.runtime_phase(journal, Phase::RestoringFiles)?;
            runtime
                .stop()
                .map_err(|_| "代理未退出，不能安全恢复文件；事务与备份已保留")?;
        }
        if !files_restored && let Some(user) = user {
            if let Some(receipt) = journal.receipt.as_ref() {
                user.restore_with_lock(receipt, lock)
                    .map_err(|_| "原用户配置恢复未完成；请保留事务并核对冲突")?;
            } else if let Some(receipt) = user
                .pending_receipt_with_lock(lock)
                .map_err(|_| "中断的用户配置记录无法验证")?
            {
                journal.receipt = Some(receipt.clone());
                self.save_journal(journal)?;
                user.restore_with_lock(&receipt, lock)
                    .map_err(|_| "中断的用户配置恢复未完成；请保留事务")?;
            }
        }
        if journal.runtime_state.is_some() {
            self.runtime_phase(journal, Phase::RestoringRuntime)?;
            let originally_running = journal.runtime_state.as_ref().unwrap().originally_running;
            // Retry always stops first, so a process started before a crash cannot be mistaken
            // for confirmation that the restored files were loaded.
            runtime
                .stop()
                .map_err(|_| "原文件已恢复，代理运行状态待恢复；事务已保留")?;
            if let (Some(user), Some(receipt)) = (user, journal.receipt.as_ref()) {
                user.verify_receipt_with_lock(receipt, lock, true)
                    .map_err(|_| "原配置曾恢复，但文件已改变；未重新启动代理，事务已保留")?;
            }
            if originally_running {
                runtime
                    .start()
                    .map_err(|_| "原文件已恢复，代理启动待恢复；事务已保留")?;
                if !runtime.running()? {
                    return Err("原文件已恢复，代理启动未确认；事务已保留");
                }
            }
        }
        if let (Some(user), Some(receipt)) = (user, journal.receipt.as_ref()) {
            user.verify_receipt_with_lock(receipt, lock, true)
                .map_err(|_| "原配置曾恢复，但代理启动后文件发生变化；事务已保留，请核对冲突")?;
        }
        self.remove_journal()
    }
    pub fn recover_user_repair(&self) -> Result<bool, &'static str> {
        #[cfg(target_os = "macos")]
        {
            self.recover_validated_finishing_with(
                || {
                    install::validate_installed_identity(self.edition, unsafe { libc::geteuid() })
                        .map(|_| ())
                },
                switcher_helper::native_authorization::Authorization::acquire,
                |plan_id, authorization| {
                    let status = install::repair_status(plan_id.clone())?;
                    if status.code == "repair_recovery_required" {
                        install::recover_repair_authorized(plan_id, authorization)
                    } else {
                        Ok(status)
                    }
                },
                || crate::guardian_repair::restore_after_commit(self.edition),
            )
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err("macos_required")
        }
    }
    #[cfg(test)]
    fn recover_validated_with<A>(
        &self,
        identity: impl FnOnce() -> Result<(), &'static str>,
        authorize: impl FnOnce() -> Result<A, &'static str>,
        status: impl FnOnce(String, &A) -> Result<HelperResponse, &'static str>,
    ) -> Result<bool, &'static str> {
        self.recover_validated_finishing_with(identity, authorize, status, || Ok(()))
    }
    fn recover_validated_finishing_with<A>(
        &self,
        identity: impl FnOnce() -> Result<(), &'static str>,
        authorize: impl FnOnce() -> Result<A, &'static str>,
        status: impl FnOnce(String, &A) -> Result<HelperResponse, &'static str>,
        after_commit: impl FnOnce() -> Result<(), &'static str>,
    ) -> Result<bool, &'static str> {
        identity().map_err(|_| "正式系统组件尚未安装并验证")?;
        let lock = storage::LifecycleLock::try_acquire(&self.home.join(".claude-switcher/v2"))
            .map_err(|_| "其他账号、修复或安装事务正在进行")?;
        let outcome = self.recover_authorized_with(&lock, authorize, status)?;
        drop(lock);
        if outcome == RecoveryOutcome::Committed {
            after_commit()?;
        }
        Ok(outcome != RecoveryOutcome::NoWork)
    }
    fn recover_authorized_with<A>(
        &self,
        lock: &storage::LifecycleLock,
        authorize: impl FnOnce() -> Result<A, &'static str>,
        status: impl FnOnce(String, &A) -> Result<HelperResponse, &'static str>,
    ) -> Result<RecoveryOutcome, &'static str> {
        let authorization =
            authorize().map_err(|_| "恢复授权取消或未通过；全部事务和备份已保留")?;
        self.recover_outcome_with(lock, |plan_id| status(plan_id, &authorization))
    }
    #[cfg(test)]
    fn recover_with(
        &self,
        lock: &storage::LifecycleLock,
        status: impl FnOnce(String) -> Result<HelperResponse, &'static str>,
    ) -> Result<bool, &'static str> {
        self.recover_outcome_with(lock, status)
            .map(|outcome| outcome != RecoveryOutcome::NoWork)
    }
    fn recover_outcome_with(
        &self,
        lock: &storage::LifecycleLock,
        status: impl FnOnce(String) -> Result<HelperResponse, &'static str>,
    ) -> Result<RecoveryOutcome, &'static str> {
        self.recover_runtime_with(lock, status, &mut proxy_repair::Native)
    }
    fn recover_runtime_with(
        &self,
        lock: &storage::LifecycleLock,
        status: impl FnOnce(String) -> Result<HelperResponse, &'static str>,
        runtime: &mut dyn Runtime,
    ) -> Result<RecoveryOutcome, &'static str> {
        if !self.has_region_journal()? {
            let user_path = self
                .home
                .join(".claude-switcher/v2/user-repairs/journal.json");
            match fs::symlink_metadata(&user_path) {
                Ok(_) => {
                    // Do not let the user coordinator's existence check hide an unsafe marker.
                    storage::read_json::<serde_json::Value>(&user_path)
                        .map_err(|_| "用户配置修复记录无法安全读取；请保留事务与备份")?;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err("无法读取用户配置修复状态"),
            }
            return UserRepairCoordinator::for_current_user(self.edition)
                .map_err(|_| "无法确认用户修复目录")?
                .recover_with_lock(lock)
                .map(|recovered| {
                    if recovered {
                        RecoveryOutcome::Restored
                    } else {
                        RecoveryOutcome::NoWork
                    }
                })
                .map_err(|_| "用户配置恢复未完成；请保留事务与备份并核对冲突");
        }
        let mut journal = self.read_journal()?;
        let user = self.user_coordinator(journal.user_required)?;
        if journal.stage == RepairStage::UserPending {
            self.restore_runtime(&mut journal, user.as_ref(), lock, runtime)?;
            return Ok(RecoveryOutcome::Restored);
        }
        let response = status(journal.plan.id.clone())
            .map_err(|_| "系统组件仍在执行或结果未知；已保留事务，请稍后重试恢复")?;
        match resolution(&journal, &response) {
            Resolution::Applied => {
                self.commit_runtime(&mut journal, user.as_ref(), lock, runtime)?;
                Ok(RecoveryOutcome::Committed)
            }
            Resolution::Restored => {
                self.restore_runtime(&mut journal, user.as_ref(), lock, runtime)?;
                Ok(RecoveryOutcome::Restored)
            }
            Resolution::Unknown => Err("系统修复尚未得到确定结果；已保留全部事务与备份"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicBool, Ordering},
    };
    use switcher_domain::system::{RepairPlan, SystemState};

    fn fixture() -> (PathBuf, RegionCoordinator) {
        let home = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-region-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        storage::create_private_directory(&home).unwrap();
        storage::create_private_directory(&home.join(".claude-switcher")).unwrap();
        storage::create_private_directory(&home.join(".claude-switcher/v2")).unwrap();
        (
            home.clone(),
            RegionCoordinator {
                home,
                edition: Edition::Personal,
            },
        )
    }
    pub(super) fn preview(target: RegionPolicy) -> Result<HelperResponse, &'static str> {
        let mut response = HelperResponse::status("preview", true);
        response.plan = Some(RepairPlan {
            id: "local-test-plan".into(),
            original: SystemState {
                timezone: "Etc/UTC".into(),
                network_service: "Wi-Fi".into(),
                ipv6: Ipv6Policy::Automatic,
            },
            target,
            expires_at: crate::checks::unix_now() + 60,
        });
        Ok(response)
    }
    pub(super) fn terminal(plan: &RepairPlan, applied: bool) -> HelperResponse {
        let mut result = HelperResponse::status(
            if applied {
                "repair_applied"
            } else {
                "repair_not_started"
            },
            applied,
        );
        result.plan = Some(plan.clone());
        result.state = Some(if applied {
            SystemState {
                timezone: plan.target.timezone.clone(),
                network_service: plan.target.network_service.clone(),
                ipv6: plan.target.ipv6,
            }
        } else {
            plan.original.clone()
        });
        result
    }
    #[test]
    fn authorization_cancelled_or_rejected_preserves_all_config_and_creates_no_journal() {
        for error in [
            "authorization_cancelled",
            "authorization_invalid",
            "plan_expired",
            "system_changed",
        ] {
            let (home, coordinator) = fixture();
            let original = encode(&config::load_read_only(&home).unwrap()).unwrap();
            let prepared = coordinator
                .preview_with(Some(RegionPreset::Jp), preview)
                .unwrap();
            let result = coordinator.execute_authorized_with(
                prepared,
                |_, lock| {
                    assert!(lock.protects(&home.join(".claude-switcher/v2")));
                    Err(error)
                },
                |_, _| panic!("denial must not reach apply"),
            );
            assert!(result.unwrap_err().contains("原配置已保留"));
            assert_eq!(
                encode(&config::load_read_only(&home).unwrap()).unwrap(),
                original
            );
            assert!(!coordinator.journal_path().exists());
            assert!(!home.join(".claude-switcher/v2/user-repairs").exists());
            fs::remove_dir_all(home).unwrap();
        }
    }
    #[test]
    fn config_changed_during_authorization_never_creates_repair_journal() {
        let (home, coordinator) = fixture();
        let prepared = coordinator
            .preview_with(Some(RegionPreset::Jp), preview)
            .unwrap();
        let result = coordinator.execute_authorized_with(
            prepared,
            |_, _| {
                storage::atomic_json(
                    &home.join(".claude-switcher/v2/config.json"),
                    &CheckConfig {
                        country_target: Some("US".into()),
                        ..Default::default()
                    },
                )
                .unwrap();
                Ok(())
            },
            |_, _| panic!("changed plan must not reach apply"),
        );
        assert!(result.is_err());
        assert!(!coordinator.journal_path().exists());
        assert_eq!(
            config::load_read_only(&home)
                .unwrap()
                .country_target
                .as_deref(),
            Some("US")
        );
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn failed_apply_does_not_commit_country_or_timezone() {
        let (home, coordinator) = fixture();
        let plan = coordinator
            .preview_with(Some(RegionPreset::Jp), preview)
            .unwrap();
        assert!(
            coordinator
                .execute_with(plan, |_, _| Ok(HelperResponse::status("cancelled", false)))
                .is_err()
        );
        assert!(!home.join(".claude-switcher/v2/config.json").exists());
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn changed_config_and_busy_lock_never_call_helper() {
        let (home, coordinator) = fixture();
        let plan = coordinator
            .preview_with(Some(RegionPreset::Jp), preview)
            .unwrap();
        let path = home.join(".claude-switcher/v2/config.json");
        let config = CheckConfig {
            country_target: Some("US".into()),
            ..Default::default()
        };
        storage::atomic_json(&path, &config).unwrap();
        let called = AtomicBool::new(false);
        assert!(
            coordinator
                .execute_with(plan, |_, _| {
                    called.store(true, Ordering::SeqCst);
                    Ok(HelperResponse::status("applied", true))
                })
                .is_err()
        );
        assert!(!called.load(Ordering::SeqCst));
        let plan = coordinator
            .preview_with(Some(RegionPreset::Jp), preview)
            .unwrap();
        let lock = storage::LifecycleLock::try_acquire(&home.join(".claude-switcher/v2")).unwrap();
        assert!(
            coordinator
                .execute_with(plan, |_, _| {
                    called.store(true, Ordering::SeqCst);
                    Ok(HelperResponse::status("applied", true))
                })
                .is_err()
        );
        assert!(!called.load(Ordering::SeqCst));
        drop(lock);
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn successful_apply_commits_new_target_and_clears_egress_continuity() {
        let (home, coordinator) = fixture();
        let plan = coordinator
            .preview_with(Some(RegionPreset::Tw), preview)
            .unwrap();
        assert_eq!(plan.preview.original.timezone, "Etc/UTC");
        let response = terminal(plan.system_plan.as_ref().unwrap(), true);
        coordinator
            .execute_with(plan, |_, lock| {
                assert!(lock.protects(&home.join(".claude-switcher/v2")));
                assert!(
                    storage::LifecycleLock::try_acquire(&home.join(".claude-switcher/v2")).is_err()
                );
                Ok(response)
            })
            .unwrap();
        let saved = config::load_read_only(&home).unwrap();
        assert_eq!(saved.country_target.as_deref(), Some("TW"));
        assert_eq!(saved.timezone_target.as_deref(), Some("Asia/Taipei"));
        assert!(saved.previous_egress_hash.is_none());
        assert!(serde_json::from_str::<RegionPreset>("\"$(touch bad)\"").is_err());
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn lost_root_reply_keeps_journal_then_verified_status_commits() {
        let (home, coordinator) = fixture();
        let plan = coordinator
            .preview_with(Some(RegionPreset::Jp), preview)
            .unwrap();
        let result = terminal(plan.system_plan.as_ref().unwrap(), true);
        assert!(
            coordinator
                .execute_with(plan, |_, _| Err("helper_outcome_unknown"))
                .is_err()
        );
        assert!(recovery_pending(&home).unwrap());
        assert!(!home.join(".claude-switcher/v2/config.json").exists());
        let lock = storage::LifecycleLock::try_acquire(&home.join(".claude-switcher/v2")).unwrap();
        assert!(coordinator.recover_with(&lock, |_| Ok(result)).unwrap());
        assert_eq!(
            config::load_read_only(&home)
                .unwrap()
                .country_target
                .as_deref(),
            Some("JP")
        );
        assert!(!recovery_pending(&home).unwrap());
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn verified_no_start_clears_journal_without_changing_config() {
        let (home, coordinator) = fixture();
        let plan = coordinator
            .preview_with(Some(RegionPreset::Jp), preview)
            .unwrap();
        let result = terminal(plan.system_plan.as_ref().unwrap(), false);
        assert!(coordinator.execute_with(plan, |_, _| Ok(result)).is_err());
        assert!(!recovery_pending(&home).unwrap());
        assert!(!home.join(".claude-switcher/v2/config.json").exists());
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn commit_intent_survives_config_rename_and_never_rolls_back() {
        let (home, coordinator) = fixture();
        let plan = coordinator
            .preview_with(Some(RegionPreset::Tw), preview)
            .unwrap();
        let applied = terminal(plan.system_plan.as_ref().unwrap(), true);
        let restored = terminal(plan.system_plan.as_ref().unwrap(), false);
        assert!(coordinator.execute_with(plan, |_, _| Err("lost")).is_err());
        let mut journal = coordinator.read_journal().unwrap();
        journal.stage = RepairStage::CommitIntent;
        coordinator.save_journal(&journal).unwrap();
        storage::atomic_json(
            &home.join(".claude-switcher/v2/config.json"),
            &journal.proposed,
        )
        .unwrap();
        let lock = storage::LifecycleLock::try_acquire(&home.join(".claude-switcher/v2")).unwrap();
        assert!(coordinator.recover_with(&lock, |_| Ok(restored)).is_err());
        assert!(recovery_pending(&home).unwrap());
        assert_eq!(
            config::load_read_only(&home)
                .unwrap()
                .country_target
                .as_deref(),
            Some("TW")
        );
        assert!(coordinator.recover_with(&lock, |_| Ok(applied)).unwrap());
        assert!(!recovery_pending(&home).unwrap());
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn unknown_wrong_plan_and_external_config_keep_recovery_blocked() {
        let (home, coordinator) = fixture();
        let plan = coordinator
            .preview_with(Some(RegionPreset::Jp), preview)
            .unwrap();
        let mut response = terminal(plan.system_plan.as_ref().unwrap(), true);
        assert!(coordinator.execute_with(plan, |_, _| Err("lost")).is_err());
        let lock = storage::LifecycleLock::try_acquire(&home.join(".claude-switcher/v2")).unwrap();
        assert!(coordinator.recover_with(&lock, |_| Err("busy")).is_err());
        response.plan.as_mut().unwrap().id = "another-plan".into();
        assert!(coordinator.recover_with(&lock, |_| Ok(response)).is_err());
        let journal = coordinator.read_journal().unwrap();
        storage::atomic_json(
            &home.join(".claude-switcher/v2/config.json"),
            &CheckConfig {
                country_target: Some("US".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            coordinator
                .recover_with(&lock, |_| Ok(terminal(&journal.plan, true)))
                .is_err()
        );
        assert_eq!(
            config::load_read_only(&home)
                .unwrap()
                .country_target
                .as_deref(),
            Some("US")
        );
        assert!(recovery_pending(&home).unwrap());
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn interruption_before_root_dispatch_recovers_without_calling_root() {
        let (home, coordinator) = fixture();
        let plan = coordinator
            .preview_with(Some(RegionPreset::Jp), preview)
            .unwrap();
        let journal = RegionJournal {
            schema: 1,
            owner: unsafe { libc::geteuid() },
            edition: Edition::Personal,
            stage: RepairStage::UserPending,
            original: serde_json::from_slice(&plan.original_config).unwrap(),
            proposed: plan.proposed_config,
            plan: plan.system_plan.unwrap(),
            user_required: false,
            receipt: None,
            runtime_state: None,
            app_open_plan: None,
        };
        coordinator.save_journal(&journal).unwrap();
        let lock = storage::LifecycleLock::try_acquire(&home.join(".claude-switcher/v2")).unwrap();
        assert!(
            coordinator
                .recover_with(&lock, |_| panic!("root was not dispatched"))
                .unwrap()
        );
        assert!(!recovery_pending(&home).unwrap());
        fs::remove_dir_all(home).unwrap();
    }

    pub(super) fn synthetic_app_plan() -> AppOpenPlan {
        let now = crate::checks::unix_now();
        serde_json::from_value(serde_json::json!({
            "issued_at": now, "expires_at": now + 120,
            "observation": { "files": [], "pid": null }
        }))
        .unwrap()
    }
    fn surge_plan(coordinator: &RegionCoordinator, home: &Path) -> PreparedRepair {
        storage::atomic_json(
            &home.join(".claude-switcher/v2/config.json"),
            &CheckConfig {
                proxy_mode: "surge".into(),
                country_target: Some("JP".into()),
                timezone_target: Some("Asia/Tokyo".into()),
                locale_target: Some("ja_JP".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let mut prepared = coordinator
            .preview_with(Some(RegionPreset::Us), preview)
            .unwrap();
        coordinator
            .preview_app_open_with(&mut prepared, |mode| {
                assert_eq!(mode, ProxyRepairMode::Surge);
                Ok(Some(synthetic_app_plan()))
            })
            .unwrap();
        prepared
    }
    #[test]
    fn app_open_preview_binds_deadline_and_missing_app_does_not_block_other_repairs() {
        let (home, coordinator) = fixture();
        let prepared = surge_plan(&coordinator, &home);
        let bound = serde_json::to_value(prepared.app_open_plan.as_ref().unwrap()).unwrap();
        assert_eq!(
            bound["expires_at"],
            prepared.system_plan.as_ref().unwrap().expires_at
        );
        let row = prepared
            .preview
            .user_changes
            .iter()
            .find(|row| row.id == "proxy_app_open")
            .unwrap();
        assert_eq!(row.state, "ready");
        assert!(row.target.contains("/Applications/Surge.app"));
        assert!(row.message.contains("仅打开"));
        assert!(row.message.contains("不会连接 VPN"));
        let mut missing = coordinator
            .preview_with(Some(RegionPreset::Us), preview)
            .unwrap();
        coordinator
            .preview_app_open_with(&mut missing, |_| Err(proxy_repair::APP_OPEN_MANUAL))
            .unwrap();
        assert!(missing.preview.can_execute);
        assert!(missing.app_open_plan.is_none());
        assert_eq!(missing.preview.user_changes[0].state, "unsupported");
        for (mode, opens) in [
            ("auto", false),
            ("0dcloud", false),
            ("sing-box", false),
            ("shadowrocket", true),
            ("clashx", true),
        ] {
            missing.proposed_config.proxy_mode = mode.into();
            missing.preview.user_changes.clear();
            coordinator
                .preview_app_open_with(&mut missing, proxy_repair::prepare_app_open)
                .unwrap();
            assert_eq!(missing.app_open_plan.is_some(), opens);
            assert_eq!(!missing.preview.user_changes.is_empty(), opens);
        }
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn app_open_denial_expiry_config_change_and_mode_mismatch_never_dispatch() {
        for reason in ["denied", "expired", "changed", "mode"] {
            let (home, coordinator) = fixture();
            let mut prepared = surge_plan(&coordinator, &home);
            if reason == "expired" {
                prepared.system_plan.as_mut().unwrap().expires_at = 0;
            }
            if reason == "mode" {
                prepared.proposed_config.proxy_mode = "auto".into();
            }
            let result = coordinator.execute_runtime_and_app_with(
                prepared,
                |_, _| {
                    if reason == "denied" {
                        return Err("cancelled");
                    }
                    if reason == "changed" {
                        let mut changed = config::load_read_only(&home).unwrap();
                        changed.proxy_mode = "auto".into();
                        storage::atomic_json(
                            &home.join(".claude-switcher/v2/config.json"),
                            &changed,
                        )
                        .unwrap();
                    }
                    Ok(())
                },
                |_, _| panic!("rejected plan must not reach root"),
                &mut proxy_repair::Native,
                |_| panic!("rejected plan must not dispatch App"),
            );
            assert!(result.is_err());
            assert!(!coordinator.journal_path().exists());
            assert!(!home.join(".claude-switcher/v2/user-repairs").exists());
            fs::remove_dir_all(home).unwrap();
        }
    }
    #[test]
    fn app_open_intent_precedes_dispatch_and_recovery_never_replays_it() {
        let (home, coordinator) = fixture();
        let prepared = surge_plan(&coordinator, &home);
        let binding = prepared.app_open_plan.clone().unwrap();
        let applied = terminal(prepared.system_plan.as_ref().unwrap(), true);
        coordinator
            .execute_runtime_and_app_with(
                prepared,
                |_, _| Ok(()),
                |_, _| Err("lost root reply"),
                &mut proxy_repair::Native,
                |actual| {
                    assert_eq!(actual, &binding);
                    let journal = coordinator.read_journal().unwrap();
                    assert_eq!(journal.app_open_plan.as_ref(), Some(&binding));
                    assert!(journal.runtime_state.is_none());
                    assert_eq!(journal.stage, RepairStage::UserPending);
                    assert!(
                        storage::LifecycleLock::try_acquire(&home.join(".claude-switcher/v2"))
                            .is_err()
                    );
                    Ok(())
                },
            )
            .unwrap_err();
        let journal = coordinator.read_journal().unwrap();
        assert_eq!(journal.app_open_plan, Some(binding));
        let lock = storage::LifecycleLock::try_acquire(&home.join(".claude-switcher/v2")).unwrap();
        // Native runtime is disabled in unit tests: any stop/start/replay would fail.
        assert!(coordinator.recover_with(&lock, |_| Ok(applied)).unwrap());
        assert!(!coordinator.journal_path().exists());
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn app_open_rejection_and_unknown_result_keep_intent_until_explicit_recovery() {
        for dispatched in [false, true] {
            let (home, coordinator) = fixture();
            let prepared = surge_plan(&coordinator, &home);
            let binding = prepared.app_open_plan.clone();
            let before = encode(&config::load_read_only(&home).unwrap()).unwrap();
            let calls = std::cell::Cell::new(0);
            let opened = std::cell::Cell::new(false);
            let result = coordinator.execute_runtime_and_app_with(
                prepared,
                |_, _| Ok(()),
                |_, _| panic!("failed App dispatch must precede root"),
                &mut proxy_repair::Native,
                |_| {
                    calls.set(calls.get() + 1);
                    opened.set(dispatched);
                    Err(if dispatched {
                        "launch outcome unknown"
                    } else {
                        "preflight rejected"
                    })
                },
            );
            assert!(result.unwrap_err().contains("请先恢复"));
            assert_eq!(opened.get(), dispatched);
            assert_eq!(
                encode(&config::load_read_only(&home).unwrap()).unwrap(),
                before
            );
            let journal = coordinator.read_journal().unwrap();
            assert_eq!(journal.stage, RepairStage::UserPending);
            assert_eq!(journal.app_open_plan, binding);
            assert!(journal.runtime_state.is_none());
            assert!(!home.join(".claude-switcher/v2/user-repairs").exists());
            let lock =
                storage::LifecycleLock::try_acquire(&home.join(".claude-switcher/v2")).unwrap();
            // Native stop/start rejects all calls in tests. Recovery must only clear the intent.
            assert!(
                coordinator
                    .recover_with(&lock, |_| panic!("UserPending must not call root"))
                    .unwrap()
            );
            assert_eq!(calls.get(), 1);
            assert_eq!(
                opened.get(),
                dispatched,
                "recovery must not stop an opened App"
            );
            assert!(!recovery_pending(&home).unwrap());
            assert_eq!(
                encode(&config::load_read_only(&home).unwrap()).unwrap(),
                before
            );
            drop(lock);
            fs::remove_dir_all(home).unwrap();
        }
    }
    #[test]
    fn app_open_post_dispatch_expiry_and_config_conflicts_keep_intent() {
        for reason in ["expired", "changed", "unreadable"] {
            let (home, coordinator) = fixture();
            let mut prepared = surge_plan(&coordinator, &home);
            if reason == "expired" {
                prepared.system_plan.as_mut().unwrap().expires_at = crate::checks::unix_now() + 1;
            }
            let path = home.join(".claude-switcher/v2/config.json");
            let original = config::load_read_only(&home).unwrap();
            let calls = std::cell::Cell::new(0);
            let result = coordinator.execute_runtime_and_app_with(
                prepared,
                |_, _| Ok(()),
                |_, _| panic!("post-open conflict must precede root"),
                &mut proxy_repair::Native,
                |_| {
                    calls.set(calls.get() + 1);
                    match reason {
                        "expired" => std::thread::sleep(std::time::Duration::from_secs(2)),
                        "changed" => {
                            let mut changed = original.clone();
                            changed.network_service = "Ethernet".into();
                            storage::atomic_json(&path, &changed).unwrap();
                        }
                        _ => storage::atomic_write(&path, b"{").unwrap(),
                    }
                    Ok(())
                },
            );
            assert!(result.unwrap_err().contains("请先恢复"));
            assert_eq!(
                coordinator.read_journal().unwrap().stage,
                RepairStage::UserPending
            );
            let lock =
                storage::LifecycleLock::try_acquire(&home.join(".claude-switcher/v2")).unwrap();
            if reason != "expired" {
                assert!(
                    coordinator
                        .recover_with(&lock, |_| panic!("must not call root"))
                        .is_err()
                );
                assert!(recovery_pending(&home).unwrap());
                // Resolve only the test's injected external conflict, then retry explicit recovery.
                storage::atomic_json(&path, &original).unwrap();
            }
            assert!(
                coordinator
                    .recover_with(&lock, |_| panic!("UserPending must not call root"))
                    .unwrap()
            );
            assert_eq!(calls.get(), 1);
            assert!(!recovery_pending(&home).unwrap());
            assert_eq!(
                encode(&config::load_read_only(&home).unwrap()).unwrap(),
                encode(&original).unwrap()
            );
            drop(lock);
            fs::remove_dir_all(home).unwrap();
        }
    }
    #[test]
    fn first_same_legacy_target_commit_materializes_guardian_policy() {
        let (home, coordinator) = fixture();
        let legacy_dir = home.join(".local/bin");
        fs::create_dir_all(&legacy_dir).unwrap();
        fs::write(legacy_dir.join("claude-switch.sh"), b"COUNTRY_TARGET=\"US\"\nTZ_TARGET=\"America/Los_Angeles\"\nLOCALE_TARGET=\"en_US\"\nPROXY_MODE=\"auto\"\n").unwrap();
        let before = config::load_read_only(&home).unwrap();
        let plan = coordinator.preview_with(None, preview).unwrap();
        assert_eq!(
            encode(&before).unwrap(),
            encode(&plan.proposed_config).unwrap()
        );
        assert!(!home.join(".claude-switcher/v2/config.json").exists());
        let applied = terminal(plan.system_plan.as_ref().unwrap(), true);
        coordinator.execute_with(plan, |_, _| Ok(applied)).unwrap();
        let snapshot =
            crate::guardian::PolicySnapshot::load(&home.join(".claude-switcher/v2")).unwrap();
        assert_eq!(encode(&snapshot.config).unwrap(), encode(&before).unwrap());
        assert!(!coordinator.journal_path().exists());
        fs::remove_dir_all(home).unwrap();
    }
}

#[cfg(all(test, target_os = "macos"))]
mod combined_tests {
    use super::*;
    use crate::user_repairs::{tests::Fixture, with_test_home};

    const USER_FILES: [&str; 3] = [
        "Library/Preferences/.GlobalPreferences.plist",
        "Library/Preferences/com.anthropic.claudefordesktop.plist",
        "Library/Application Support/io.github.clash-verge-rev.clash-verge-rev/verge.yaml",
    ];
    fn images(home: &Path) -> Vec<Vec<u8>> {
        USER_FILES
            .iter()
            .map(|name| fs::read(home.join(name)).unwrap())
            .collect()
    }
    fn repair_backup_images(directory: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
        let mut images = std::collections::BTreeMap::new();
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_symlink() {
                continue; // Marker targets are checked separately without following them.
            }
            if path.is_dir() {
                images.extend(repair_backup_images(&path));
            } else {
                images.insert(path.clone(), fs::read(path).unwrap());
            }
        }
        images
    }
    fn coordinator(fixture: &Fixture) -> RegionCoordinator {
        RegionCoordinator {
            home: fixture.root.clone(),
            edition: Edition::Personal,
        }
    }
    fn prepared(region: &RegionCoordinator, fixture: &Fixture) -> PreparedRepair {
        let mut plan = region
            .preview_with(Some(RegionPreset::Us), super::tests::preview)
            .unwrap();
        let user = fixture.coordinator.preview(Fixture::target()).unwrap();
        assert!(user.preview.can_execute);
        plan.user_plan = Some(user);
        plan
    }
    fn pending_fixture(region: &RegionCoordinator, fixture: &Fixture, region_journal: bool) {
        if region_journal {
            let plan = prepared(region, fixture);
            assert!(region.execute_with(plan, |_, _| Err("unknown")).is_err());
        } else {
            let plan = fixture.coordinator.preview(Fixture::target()).unwrap();
            let lock =
                storage::LifecycleLock::try_acquire(&fixture.root.join(".claude-switcher/v2"))
                    .unwrap();
            fixture.coordinator.execute_with_lock(plan, &lock).unwrap();
        }
    }
    #[test]
    fn app_open_runs_before_user_preferences_and_failure_preserves_them() {
        for fail_app in [false, true] {
            let fixture = Fixture::new();
            let region = coordinator(&fixture);
            let original = images(&fixture.root);
            with_test_home(&fixture.root, None, || {
                storage::atomic_json(
                    &fixture.root.join(".claude-switcher/v2/config.json"),
                    &CheckConfig {
                        proxy_mode: "surge".into(),
                        country_target: Some("JP".into()),
                        timezone_target: Some("Asia/Tokyo".into()),
                        locale_target: Some("ja_JP".into()),
                        ..Default::default()
                    },
                )
                .unwrap();
                let mut plan = region
                    .preview_with(Some(RegionPreset::Us), super::tests::preview)
                    .unwrap();
                plan.user_plan = Some(
                    fixture
                        .coordinator
                        .preview(UserRepairTarget {
                            locale: UserLocale::Us,
                            proxy: ProxyRepairMode::Surge,
                        })
                        .unwrap(),
                );
                region
                    .preview_app_open_with(&mut plan, |_| {
                        Ok(Some(super::tests::synthetic_app_plan()))
                    })
                    .unwrap();
                let binding = plan.app_open_plan.clone();
                let applied = super::tests::terminal(plan.system_plan.as_ref().unwrap(), true);
                let result = region.execute_runtime_and_app_with(
                    plan,
                    |_, _| Ok(()),
                    |_, _| {
                        assert!(!fail_app, "failed App must block root dispatch");
                        let after = images(&fixture.root);
                        assert_ne!(after[0], original[0]);
                        assert_ne!(after[1], original[1]);
                        assert_eq!(
                            after[2], original[2],
                            "Surge never writes Clash configuration"
                        );
                        Ok(applied)
                    },
                    &mut proxy_repair::Native,
                    |actual| {
                        assert_eq!(Some(actual), binding.as_ref());
                        assert_eq!(images(&fixture.root), original);
                        assert!(region.read_journal().unwrap().app_open_plan.is_some());
                        if fail_app {
                            Err(proxy_repair::APP_OPEN_MANUAL)
                        } else {
                            Ok(())
                        }
                    },
                );
                assert_eq!(result.is_err(), fail_app);
                if fail_app {
                    assert_eq!(images(&fixture.root), original);
                    assert!(recovery_pending(&fixture.root).unwrap());
                    let lock = storage::LifecycleLock::try_acquire(
                        &fixture.root.join(".claude-switcher/v2"),
                    )
                    .unwrap();
                    assert!(
                        region
                            .recover_with(&lock, |_| panic!("UserPending must not call root"))
                            .unwrap()
                    );
                    assert_eq!(images(&fixture.root), original);
                }
                assert!(!recovery_pending(&fixture.root).unwrap());
            });
        }
    }
    #[test]
    fn repair_preview_accepts_missing_or_stale_guardian_health() {
        for failure in ["guardian_missing", "guardian_stale"] {
            let fixture = Fixture::new();
            let region = coordinator(&fixture);
            with_test_home(&fixture.root, None, || {
                let preview = region
                    .preview_validated_with(
                        Some(RegionPreset::Us),
                        || Ok(()),
                        || Err(failure),
                        super::tests::preview,
                    )
                    .unwrap();
                assert!(preview.preview.can_execute);
                assert!(
                    preview
                        .preview
                        .user_changes
                        .iter()
                        .any(|item| item.id == "guardian_service" && item.state == "ready")
                );
            });
        }
    }
    #[test]
    fn recovered_commit_finishes_guardian_only_after_releasing_lock() {
        for applied in [false, true] {
            for guardian_ok in [false, true] {
                let fixture = Fixture::new();
                let region = coordinator(&fixture);
                with_test_home(&fixture.root, None, || {
                    let plan = prepared(&region, &fixture);
                    assert!(region.execute_with(plan, |_, _| Err("lost_reply")).is_err());
                    let journal = region.read_journal().unwrap();
                    let called = std::cell::Cell::new(false);
                    let result = region.recover_validated_finishing_with(
                        || Ok(()),
                        || Ok(()),
                        |_, _| Ok(super::tests::terminal(&journal.plan, applied)),
                        || {
                            called.set(true);
                            let _lock = storage::LifecycleLock::try_acquire(
                                &fixture.root.join(".claude-switcher/v2"),
                            )
                            .unwrap();
                            assert!(!recovery_pending(&fixture.root).unwrap());
                            if guardian_ok {
                                Ok(())
                            } else {
                                Err("系统和用户配置已提交，但守护服务尚未恢复")
                            }
                        },
                    );
                    assert_eq!(called.get(), applied);
                    assert_eq!(result.is_ok(), !applied || guardian_ok);
                    assert!(!recovery_pending(&fixture.root).unwrap());
                    assert_eq!(
                        config::load_read_only(&fixture.root)
                            .unwrap()
                            .country_target,
                        if applied {
                            journal.proposed.country_target
                        } else {
                            journal.original.country_target
                        }
                    );
                });
            }
        }
    }
    #[test]
    fn committed_repair_releases_lock_before_guardian_wait() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        with_test_home(&fixture.root, None, || {
            let plan = prepared(&region, &fixture);
            let system = plan.system_plan.clone().unwrap();
            region
                .execute_with(plan, |_, _| Ok(super::tests::terminal(&system, true)))
                .unwrap();
            assert!(!recovery_pending(&fixture.root).unwrap());
            let _lock =
                storage::LifecycleLock::try_acquire(&fixture.root.join(".claude-switcher/v2"))
                    .unwrap();
        });
    }
    fn health_fixture(home: &Path, degraded: bool) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            home.join(".claude-switcher/v2"),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        let now = crate::checks::unix_now();
        storage::atomic_json(
            &home.join(".claude-switcher/v2/guardian-health.json"),
            &crate::guardian::GuardianHealth {
                owner_uid: unsafe { libc::geteuid() },
                pid: std::process::id() as i32,
                heartbeat_at: now,
                started_at: now,
                policy: CheckConfig::default(),
                policy_fingerprint: String::new(),
                degraded,
            },
        )
        .unwrap();
    }
    fn installation_health(home: &Path) -> Result<(), &'static str> {
        crate::guardian::read_health(
            &home.join(".claude-switcher/v2/guardian-health.json"),
            unsafe { libc::geteuid() },
            crate::checks::unix_now(),
        )
        .map(|_| ())
    }
    #[test]
    fn recovery_entrypoints_accept_pending_journals_even_with_degraded_health() {
        for region_journal in [false, true] {
            for degraded in [false, true] {
                let fixture = Fixture::new();
                let region = coordinator(&fixture);
                with_test_home(&fixture.root, None, || {
                    let original = images(&fixture.root);
                    pending_fixture(&region, &fixture, region_journal);
                    health_fixture(&fixture.root, degraded);
                    if !region_journal {
                        storage::atomic_write(
                            &fixture.root.join(".claude-switcher/v2/config.json"),
                            b"{",
                        )
                        .unwrap();
                    }
                    let health = || installation_health(&fixture.root);
                    // The unchanged launch health gate still fails for either journal type.
                    assert_eq!(
                        health(),
                        Err(if degraded {
                            "guardian_degraded"
                        } else {
                            "guardian_repair_pending"
                        })
                    );
                    let pending = region
                        .preview_validated_with(
                            Some(RegionPreset::Us),
                            || Ok(()),
                            health,
                            |_| panic!("pending recovery must not request a new root plan"),
                        )
                        .unwrap();
                    assert!(pending.preview.recovery_required);
                    assert!(!pending.preview.can_execute);
                    if !region_journal {
                        assert!(pending.preview.message.contains("configuration_invalid"));
                        assert!(pending.preview.message.contains("无法读取"));
                        assert_eq!(pending.preview.network_service, "未确认");
                        for view in [&pending.preview.original, &pending.preview.target] {
                            assert_eq!(view.country, "未确认");
                            assert_eq!(view.timezone, "未确认");
                            assert_eq!(view.locale, "未确认");
                        }
                        assert_eq!(
                            fs::read(fixture.root.join(".claude-switcher/v2/config.json")).unwrap(),
                            b"{"
                        );
                    }
                    assert!(
                        region
                            .recover_validated_with(
                                || Ok(()),
                                || Ok(()),
                                |_, _| {
                                    let journal = region.read_journal().unwrap();
                                    Ok(super::tests::terminal(&journal.plan, false))
                                }
                            )
                            .unwrap()
                    );
                    assert!(!recovery_pending(&fixture.root).unwrap());
                    assert_eq!(images(&fixture.root), original);
                });
            }
        }
    }
    #[test]
    fn recovery_entrypoint_rejections_preserve_journals_and_backups() {
        for region_journal in [false, true] {
            for failure in ["identity", "corrupt", "cancel", "unknown"] {
                if !region_journal && failure == "unknown" {
                    continue;
                }
                let fixture = Fixture::new();
                let region = coordinator(&fixture);
                with_test_home(&fixture.root, None, || {
                    pending_fixture(&region, &fixture, region_journal);
                    health_fixture(&fixture.root, true);
                    let user_path = fixture
                        .root
                        .join(".claude-switcher/v2/user-repairs/journal.json");
                    if failure == "corrupt" {
                        let corrupt_path = if region_journal {
                            region.journal_path()
                        } else {
                            user_path.clone()
                        };
                        storage::atomic_write(&corrupt_path, b"{").unwrap();
                        storage::atomic_write(
                            &fixture.root.join(".claude-switcher/v2/config.json"),
                            b"{",
                        )
                        .unwrap();
                    }
                    let before = images(&fixture.root);
                    let journal = fs::read(region.journal_path()).ok();
                    let user_journal = fs::read(&user_path).unwrap();
                    let backup_directory = fixture.root.join(".claude-switcher/v2/user-repairs");
                    let backups = repair_backup_images(&backup_directory);
                    let identity = || {
                        if failure == "identity" {
                            Err("installed_owner_or_edition")
                        } else {
                            Ok(())
                        }
                    };
                    let health = || installation_health(&fixture.root);
                    if failure == "identity" {
                        assert!(
                            region
                                .preview_validated_with(
                                    Some(RegionPreset::Us),
                                    identity,
                                    health,
                                    |_| panic!("invalid recovery must not request a plan")
                                )
                                .is_err()
                        );
                    } else if failure == "corrupt" {
                        let preview = region
                            .preview_validated_with(
                                Some(RegionPreset::Us),
                                identity,
                                health,
                                |_| panic!("corrupt recovery must not request a plan"),
                            )
                            .unwrap();
                        assert!(preview.preview.recovery_required);
                        assert!(!preview.preview.can_execute);
                        assert!(preview.preview.message.contains("验证"));
                        assert!(preview.system_plan.is_none());
                        assert!(preview.preview.message.contains("configuration_invalid"));
                        assert!(preview.preview.message.contains("无法读取"));
                        assert_eq!(preview.preview.network_service, "未确认");
                        assert_eq!(preview.preview.original.country, "未确认");
                        assert_eq!(preview.preview.target.country, "未确认");
                    }
                    assert!(region.recover_validated_with(identity, || {
                        if failure == "cancel" { Err("authorization_cancelled") } else { Ok(()) }
                    }, |_, _| {
                        assert_eq!(failure, "unknown", "identity, corrupt journal or cancellation must not reach root status");
                        Err("helper_outcome_unknown")
                    }).is_err());
                    assert_eq!(images(&fixture.root), before);
                    assert_eq!(fs::read(region.journal_path()).ok(), journal);
                    assert_eq!(fs::read(user_path).unwrap(), user_journal);
                    assert_eq!(repair_backup_images(&backup_directory), backups);
                });
            }
        }
    }
    #[test]
    fn dangling_journal_markers_never_trigger_user_only_recovery() {
        use std::os::unix::fs::symlink;
        for region_marker in [true, false] {
            let fixture = Fixture::new();
            let region = coordinator(&fixture);
            with_test_home(&fixture.root, None, || {
                pending_fixture(&region, &fixture, region_marker);
                health_fixture(&fixture.root, true);
                let user_path = fixture
                    .root
                    .join(".claude-switcher/v2/user-repairs/journal.json");
                let marker = if region_marker {
                    region.journal_path()
                } else {
                    user_path.clone()
                };
                let saved = marker.with_extension("saved");
                fs::rename(&marker, &saved).unwrap();
                symlink("missing-journal", &marker).unwrap();
                let before = images(&fixture.root);
                let saved_bytes = fs::read(&saved).unwrap();
                let user_bytes = region_marker.then(|| fs::read(&user_path).unwrap());
                let backups =
                    repair_backup_images(&fixture.root.join(".claude-switcher/v2/user-repairs"));
                assert!(recovery_pending(&fixture.root).unwrap());
                let preview = region
                    .preview_validated_with(
                        None,
                        || Ok(()),
                        || panic!("pending health"),
                        |_| panic!("pending root plan"),
                    )
                    .unwrap();
                assert!(preview.preview.recovery_required);
                assert!(!preview.preview.can_execute);
                assert!(preview.system_plan.is_none());
                assert!(
                    region
                        .recover_validated_with(
                            || Ok(()),
                            || Ok(()),
                            |_, _| panic!("invalid marker must not query root")
                        )
                        .is_err()
                );
                assert_eq!(images(&fixture.root), before);
                assert_eq!(
                    fs::read_link(&marker).unwrap(),
                    PathBuf::from("missing-journal")
                );
                assert_eq!(fs::read(&saved).unwrap(), saved_bytes);
                if let Some(bytes) = user_bytes {
                    assert_eq!(fs::read(&user_path).unwrap(), bytes);
                }
                assert_eq!(
                    repair_backup_images(&fixture.root.join(".claude-switcher/v2/user-repairs")),
                    backups
                );
            });
        }
    }
    #[test]
    fn new_repair_preview_still_requires_installed_identity() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        health_fixture(&fixture.root, true);
        assert!(
            region
                .preview_validated_with(
                    Some(RegionPreset::Us),
                    || Err("installed_owner_or_edition"),
                    || panic!("invalid identity must not inspect health"),
                    |_| panic!("invalid identity must not request a new plan")
                )
                .is_err()
        );
        assert!(!recovery_pending(&fixture.root).unwrap());
    }
    #[test]
    fn cancelled_recovery_preserves_pending_files_and_both_journals() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        with_test_home(&fixture.root, None, || {
            let plan = prepared(&region, &fixture);
            assert!(
                region
                    .execute_with(plan, |_, _| Err("outcome_unknown"))
                    .is_err()
            );
            let pending = images(&fixture.root);
            let region_journal = std::fs::read(region.journal_path()).unwrap();
            let user_path = fixture
                .root
                .join(".claude-switcher/v2/user-repairs/journal.json");
            let user_journal = std::fs::read(&user_path).unwrap();
            let lock =
                storage::LifecycleLock::try_acquire(&fixture.root.join(".claude-switcher/v2"))
                    .unwrap();
            assert!(
                region
                    .recover_authorized_with(
                        &lock,
                        || Err::<(), _>("authorization_cancelled"),
                        |_, _| panic!("cancel must not dispatch recovery")
                    )
                    .is_err()
            );
            assert_eq!(images(&fixture.root), pending);
            assert_eq!(
                std::fs::read(region.journal_path()).unwrap(),
                region_journal
            );
            assert_eq!(std::fs::read(user_path).unwrap(), user_journal);
        });
    }
    #[test]
    fn combined_authorization_denial_and_expiry_make_zero_user_writes() {
        for expired in [false, true] {
            let fixture = Fixture::new();
            let region = coordinator(&fixture);
            let original = images(&fixture.root);
            with_test_home(&fixture.root, None, || {
                let mut plan = prepared(&region, &fixture);
                if expired {
                    plan.system_plan.as_mut().unwrap().expires_at = 0;
                }
                assert!(
                    region
                        .execute_authorized_with(
                            plan,
                            |_, _| {
                                if expired {
                                    Ok(())
                                } else {
                                    Err("authorization_cancelled")
                                }
                            },
                            |_, _| panic!("denied/expired authorization must not reach apply")
                        )
                        .is_err()
                );
                assert_eq!(images(&fixture.root), original);
                assert!(!recovery_pending(&fixture.root).unwrap());
                assert!(
                    !fixture
                        .root
                        .join(".claude-switcher/v2/config.json")
                        .exists()
                );
            });
        }
    }
    #[test]
    fn combined_some_plan_commits_user_and_system_under_one_lock() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        let original = images(&fixture.root);
        with_test_home(&fixture.root, None, || {
            let plan = prepared(&region, &fixture);
            let response = super::tests::terminal(plan.system_plan.as_ref().unwrap(), true);
            region
                .execute_with(plan, |_, lock| {
                    assert!(
                        images(&fixture.root)
                            .iter()
                            .zip(&original)
                            .all(|(after, before)| after != before)
                    );
                    assert!(lock.protects(&fixture.root.join(".claude-switcher/v2")));
                    assert!(recovery_pending(&fixture.root).unwrap());
                    Ok(response)
                })
                .unwrap();
            assert!(
                images(&fixture.root)
                    .iter()
                    .zip(&original)
                    .all(|(after, before)| after != before)
            );
            assert!(!recovery_pending(&fixture.root).unwrap());
            assert_eq!(
                config::load_read_only(&fixture.root)
                    .unwrap()
                    .country_target
                    .as_deref(),
                Some("US")
            );
        });
    }
    #[test]
    fn combined_confirmed_root_failure_or_cancel_restores_complete_user_files() {
        for code in ["repair_restored", "repair_not_started"] {
            let fixture = Fixture::new();
            let region = coordinator(&fixture);
            let original = images(&fixture.root);
            with_test_home(&fixture.root, None, || {
                let plan = prepared(&region, &fixture);
                let mut response =
                    super::tests::terminal(plan.system_plan.as_ref().unwrap(), false);
                response.code = code.into();
                response.ok = false;
                assert!(
                    region
                        .execute_with(plan, |_, _| {
                            assert!(
                                images(&fixture.root)
                                    .iter()
                                    .zip(&original)
                                    .all(|(after, before)| after != before)
                            );
                            Ok(response)
                        })
                        .is_err()
                );
                assert_eq!(images(&fixture.root), original);
                assert!(!recovery_pending(&fixture.root).unwrap());
                assert!(
                    !fixture
                        .root
                        .join(".claude-switcher/v2/config.json")
                        .exists()
                );
            });
        }
    }
    #[test]
    fn combined_unknown_retains_both_journals_until_verified_recovery() {
        for applied in [false, true] {
            let fixture = Fixture::new();
            let region = coordinator(&fixture);
            let original = images(&fixture.root);
            let response = with_test_home(&fixture.root, None, || {
                let plan = prepared(&region, &fixture);
                let response = super::tests::terminal(plan.system_plan.as_ref().unwrap(), applied);
                assert!(
                    region
                        .execute_with(plan, |_, _| Err("helper_outcome_unknown"))
                        .is_err()
                );
                response
            });
            assert!(
                images(&fixture.root)
                    .iter()
                    .zip(&original)
                    .all(|(after, before)| after != before)
            );
            for file in ["region-repair.json", "user-repairs/journal.json"] {
                assert!(
                    fixture
                        .root
                        .join(".claude-switcher/v2")
                        .join(file)
                        .is_file()
                );
            }
            assert!(recovery_pending(&fixture.root).unwrap());
            assert!(
                crate::accounts::ensure_no_pending_repairs(&fixture.root.join(".claude-switcher"))
                    .is_err()
            );
            assert!(
                !fixture
                    .root
                    .join(".claude-switcher/v2/config.json")
                    .exists()
            );
            with_test_home(&fixture.root, None, || {
                let next = region
                    .preview_with(Some(RegionPreset::Us), super::tests::preview)
                    .unwrap();
                assert!(
                    region
                        .execute_with(next, |_, _| panic!(
                            "pending transaction must reject helper dispatch"
                        ))
                        .is_err()
                );
                let lock =
                    storage::LifecycleLock::try_acquire(&fixture.root.join(".claude-switcher/v2"))
                        .unwrap();
                assert!(
                    region
                        .recover_with(&lock, |_| Err("still_unknown"))
                        .is_err()
                );
                assert!(recovery_pending(&fixture.root).unwrap());
                assert!(region.recover_with(&lock, |_| Ok(response)).unwrap());
            });
            assert!(!recovery_pending(&fixture.root).unwrap());
            assert_eq!(images(&fixture.root) == original, !applied);
            assert_eq!(
                fixture
                    .root
                    .join(".claude-switcher/v2/config.json")
                    .exists(),
                applied
            );
        }
    }
    #[test]
    fn combined_user_write_failure_never_calls_root_and_recovers_region_journal() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        let original = images(&fixture.root);
        with_test_home(&fixture.root, Some(2), || {
            let plan = prepared(&region, &fixture);
            assert!(
                region
                    .execute_with(plan, |_, _| panic!(
                        "user failure must precede helper dispatch"
                    ))
                    .is_err()
            );
            assert_eq!(images(&fixture.root), original);
            assert!(recovery_pending(&fixture.root).unwrap());
            let lock =
                storage::LifecycleLock::try_acquire(&fixture.root.join(".claude-switcher/v2"))
                    .unwrap();
            assert!(
                region
                    .recover_with(&lock, |_| panic!("UserPending must not query root"))
                    .unwrap()
            );
            assert!(!recovery_pending(&fixture.root).unwrap());
        });
    }

    struct FakeRuntime {
        home: PathBuf,
        original: Vec<Vec<u8>>,
        running: bool,
        stops: usize,
        starts: usize,
        verifies: usize,
        fail_stop: Option<usize>,
        fail_start: Option<usize>,
        fail_verify: Option<usize>,
        delay_verify: bool,
        mutate_on_start: Option<usize>,
    }
    impl FakeRuntime {
        fn new(fixture: &Fixture, running: bool) -> Self {
            Self {
                home: fixture.root.clone(),
                original: images(&fixture.root),
                running,
                stops: 0,
                starts: 0,
                verifies: 0,
                fail_stop: None,
                fail_start: None,
                fail_verify: None,
                delay_verify: false,
                mutate_on_start: None,
            }
        }
        fn journal(&self) -> RegionJournal {
            storage::read_json(&self.home.join(".claude-switcher/v2/region-repair.json")).unwrap()
        }
    }
    impl Runtime for FakeRuntime {
        fn running(&mut self) -> Result<bool, &'static str> {
            Ok(self.running)
        }
        fn stop(&mut self) -> Result<(), &'static str> {
            let journal = self.journal(); // every mutation must already have durable intent
            let phase = journal.runtime_state.unwrap().phase;
            if phase == Phase::Intent {
                assert_eq!(images(&self.home), self.original);
            }
            self.stops += 1;
            if self.fail_stop == Some(self.stops) {
                return Err("injected stop failure");
            }
            self.running = false;
            Ok(())
        }
        fn start(&mut self) -> Result<(), &'static str> {
            let phase = self.journal().runtime_state.unwrap().phase;
            assert!(!self.running);
            if phase == Phase::RestoringRuntime {
                assert_eq!(images(&self.home), self.original);
            } else {
                assert_ne!(images(&self.home), self.original);
            }
            self.starts += 1;
            if self.fail_start == Some(self.starts) {
                return Err("injected start failure");
            }
            self.running = true;
            if self.mutate_on_start == Some(self.starts) {
                fs::write(
                    self.home.join(USER_FILES[2]),
                    b"external: rewrite\nenable_tun_mode: true\n",
                )
                .unwrap();
            }
            Ok(())
        }
        fn verify(&mut self, country: &str) -> Result<(), &'static str> {
            assert_eq!(country, "US");
            self.verifies += 1;
            if self.delay_verify {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            if self.fail_verify == Some(self.verifies) {
                Err("country or TUN mismatch")
            } else if self.running {
                Ok(())
            } else {
                Err("not running")
            }
        }
    }
    fn runtime_plan(region: &RegionCoordinator, fixture: &Fixture) -> PreparedRepair {
        storage::atomic_json(
            &fixture.root.join(".claude-switcher/v2/config.json"),
            &CheckConfig {
                proxy_mode: "clash-verge".into(),
                country_target: Some("JP".into()),
                timezone_target: Some("Asia/Tokyo".into()),
                locale_target: Some("ja_JP".into()),
                ..Default::default()
            },
        )
        .unwrap();
        prepared(region, fixture)
    }
    #[test]
    fn runtime_authorization_denied_has_no_intent_or_process_changes() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        with_test_home(&fixture.root, None, || {
            let plan = runtime_plan(&region, &fixture);
            let mut fake = FakeRuntime::new(&fixture, true);
            assert!(
                region
                    .execute_runtime_with(plan, |_, _| Err("cancelled"), |_, _| panic!(), &mut fake)
                    .is_err()
            );
            assert_eq!((fake.stops, fake.starts), (0, 0));
            assert_eq!(images(&fixture.root), fake.original);
            assert!(!region.journal_path().exists());
        });
    }
    #[test]
    fn runtime_start_and_reload_are_durable_before_root_dispatch() {
        for running in [true, false] {
            let fixture = Fixture::new();
            let region = coordinator(&fixture);
            with_test_home(&fixture.root, None, || {
                let plan = runtime_plan(&region, &fixture);
                let applied = super::tests::terminal(plan.system_plan.as_ref().unwrap(), true);
                let mut fake = FakeRuntime::new(&fixture, running);
                region
                    .execute_runtime_with(
                        plan,
                        |_, _| Ok(()),
                        |_, _| {
                            assert_eq!(
                                region.read_journal().unwrap().runtime_state.unwrap().phase,
                                Phase::TargetReady
                            );
                            Ok(applied)
                        },
                        &mut fake,
                    )
                    .unwrap();
                assert_eq!(fake.starts, 1);
                assert!(fake.running);
                assert!(!region.journal_path().exists());
            });
        }
    }
    #[test]
    fn runtime_country_failure_restores_files_and_original_running_state() {
        for running in [true, false] {
            let fixture = Fixture::new();
            let region = coordinator(&fixture);
            with_test_home(&fixture.root, None, || {
                let plan = runtime_plan(&region, &fixture);
                let mut fake = FakeRuntime::new(&fixture, running);
                fake.fail_verify = Some(1);
                assert!(
                    region
                        .execute_runtime_with(
                            plan,
                            |_, _| Ok(()),
                            |_, _| panic!("country mismatch must block root"),
                            &mut fake
                        )
                        .is_err()
                );
                assert_eq!(images(&fixture.root), fake.original);
                assert_eq!(fake.running, running);
                assert!(!region.journal_path().exists());
            });
        }
    }
    #[test]
    fn runtime_target_start_failure_restores_original_files_before_restart() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        with_test_home(&fixture.root, None, || {
            let plan = runtime_plan(&region, &fixture);
            let mut fake = FakeRuntime::new(&fixture, true);
            fake.fail_start = Some(1);
            assert!(
                region
                    .execute_runtime_with(plan, |_, _| Ok(()), |_, _| panic!(), &mut fake)
                    .is_err()
            );
            assert_eq!(fake.starts, 2);
            assert!(fake.running);
            assert_eq!(images(&fixture.root), fake.original);
            assert!(!region.journal_path().exists());
        });
    }
    #[test]
    fn runtime_stop_failure_never_writes_target_files() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        with_test_home(&fixture.root, None, || {
            let plan = runtime_plan(&region, &fixture);
            let mut fake = FakeRuntime::new(&fixture, true);
            fake.fail_stop = Some(1);
            assert!(
                region
                    .execute_runtime_with(plan, |_, _| Ok(()), |_, _| panic!(), &mut fake)
                    .is_err()
            );
            assert_eq!(images(&fixture.root), fake.original);
            assert_eq!(fake.verifies, 0);
        });
    }
    #[test]
    fn runtime_restored_files_with_failed_restart_keep_recoverable_journal() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        with_test_home(&fixture.root, None, || {
            let plan = runtime_plan(&region, &fixture);
            let mut fake = FakeRuntime::new(&fixture, true);
            fake.fail_verify = Some(1);
            fake.fail_start = Some(2);
            let error = region
                .execute_runtime_with(plan, |_, _| Ok(()), |_, _| panic!(), &mut fake)
                .unwrap_err();
            assert!(error.contains("原文件已恢复"));
            assert_eq!(images(&fixture.root), fake.original);
            assert_eq!(
                region.read_journal().unwrap().runtime_state.unwrap().phase,
                Phase::RestoringRuntime
            );
            let lock =
                storage::LifecycleLock::try_acquire(&fixture.root.join(".claude-switcher/v2"))
                    .unwrap();
            assert!(matches!(
                region
                    .recover_runtime_with(&lock, |_| panic!("root never dispatched"), &mut fake)
                    .unwrap(),
                RecoveryOutcome::Restored
            ));
            assert!(fake.running);
            assert!(!region.journal_path().exists());
        });
    }
    #[test]
    fn runtime_plan_expiry_after_verification_never_dispatches_root() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        with_test_home(&fixture.root, None, || {
            let mut plan = runtime_plan(&region, &fixture);
            plan.system_plan.as_mut().unwrap().expires_at = crate::checks::unix_now() + 2;
            let mut fake = FakeRuntime::new(&fixture, false);
            fake.delay_verify = true;
            assert!(
                region
                    .execute_runtime_with(
                        plan,
                        |_, _| Ok(()),
                        |_, _| panic!("expired root plan"),
                        &mut fake
                    )
                    .unwrap_err()
                    .contains("过期")
            );
            assert_eq!(images(&fixture.root), fake.original);
            assert!(!fake.running);
            assert!(!region.journal_path().exists());
        });
    }
    #[test]
    fn runtime_applied_recovery_keeps_target_files_when_restart_fails() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        with_test_home(&fixture.root, None, || {
            let plan = runtime_plan(&region, &fixture);
            let applied = super::tests::terminal(plan.system_plan.as_ref().unwrap(), true);
            let mut fake = FakeRuntime::new(&fixture, true);
            assert!(
                region
                    .execute_runtime_with(
                        plan,
                        |_, _| Ok(()),
                        |_, _| Err("lost root reply"),
                        &mut fake
                    )
                    .is_err()
            );
            let target_files = images(&fixture.root);
            fake.fail_verify = Some(2);
            fake.fail_start = Some(2);
            let lock =
                storage::LifecycleLock::try_acquire(&fixture.root.join(".claude-switcher/v2"))
                    .unwrap();
            let error = region
                .recover_runtime_with(&lock, |_| Ok(applied.clone()), &mut fake)
                .err()
                .unwrap();
            assert!(error.contains("已提交"));
            assert_eq!(images(&fixture.root), target_files);
            assert_eq!(
                config::load_read_only(&fixture.root)
                    .unwrap()
                    .country_target
                    .as_deref(),
                Some("US")
            );
            assert_eq!(
                region.read_journal().unwrap().stage,
                RepairStage::CommitIntent
            );
            assert!(matches!(
                region
                    .recover_runtime_with(&lock, |_| Ok(applied), &mut fake)
                    .unwrap(),
                RecoveryOutcome::Committed
            ));
            assert_eq!(images(&fixture.root), target_files);
            assert!(!region.journal_path().exists());
        });
    }

    #[test]
    fn runtime_external_rewrite_after_start_never_dispatches_root_or_overwrites_conflict() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        with_test_home(&fixture.root, None, || {
            let plan = runtime_plan(&region, &fixture);
            let mut fake = FakeRuntime::new(&fixture, true);
            fake.mutate_on_start = Some(1);
            assert!(
                region
                    .execute_runtime_with(
                        plan,
                        |_, _| Ok(()),
                        |_, _| panic!("app rewrite must block root"),
                        &mut fake
                    )
                    .is_err()
            );
            assert_eq!(
                fs::read(fixture.root.join(USER_FILES[2])).unwrap(),
                b"external: rewrite\nenable_tun_mode: true\n"
            );
            assert!(region.journal_path().exists());
            assert_eq!(
                region.read_journal().unwrap().stage,
                RepairStage::UserPending
            );
        });
    }
    #[test]
    fn runtime_original_restart_rewrite_does_not_report_restoration_complete() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        with_test_home(&fixture.root, None, || {
            let plan = runtime_plan(&region, &fixture);
            let mut fake = FakeRuntime::new(&fixture, true);
            fake.fail_verify = Some(1);
            fake.mutate_on_start = Some(2);
            let error = region
                .execute_runtime_with(plan, |_, _| Ok(()), |_, _| panic!(), &mut fake)
                .unwrap_err();
            assert!(error.contains("文件发生变化"));
            assert!(region.journal_path().exists());
        });
    }

    #[test]
    fn user_pending_dangling_receipt_preserves_region_recovery() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        with_test_home(&fixture.root, None, || {
            pending_fixture(&region, &fixture, true);
            let mut journal = region.read_journal().unwrap();
            journal.stage = RepairStage::UserPending;
            journal.receipt = None;
            journal.runtime_state = None;
            region.save_journal(&journal).unwrap();
            let user_journal = fixture
                .root
                .join(".claude-switcher/v2/user-repairs/journal.json");
            let saved = user_journal.with_extension("saved");
            fs::rename(&user_journal, &saved).unwrap();
            std::os::unix::fs::symlink("missing-journal", &user_journal).unwrap();
            let region_before = fs::read(region.journal_path()).unwrap();
            let saved_before = fs::read(&saved).unwrap();
            let files_before = images(&fixture.root);
            let lock =
                storage::LifecycleLock::try_acquire(&fixture.root.join(".claude-switcher/v2"))
                    .unwrap();
            let mut runtime = FakeRuntime::new(&fixture, false);
            assert!(
                region
                    .recover_runtime_with(&lock, |_| panic!("unexpected root status"), &mut runtime)
                    .is_err()
            );
            assert_eq!(fs::read(region.journal_path()).unwrap(), region_before);
            assert_eq!(fs::read(&saved).unwrap(), saved_before);
            assert_eq!(
                fs::read_link(&user_journal).unwrap(),
                PathBuf::from("missing-journal")
            );
            assert_eq!(images(&fixture.root), files_before);
            assert_eq!((runtime.stops, runtime.starts), (0, 0));
        });
    }

    #[test]
    fn runtime_crash_before_root_dispatch_restores_without_root_status() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        with_test_home(&fixture.root, None, || {
            let plan = runtime_plan(&region, &fixture);
            let mut fake = FakeRuntime::new(&fixture, true);
            assert!(
                region
                    .execute_runtime_with(plan, |_, _| Ok(()), |_, _| Err("lost reply"), &mut fake)
                    .is_err()
            );
            // Recreate the durable checkpoint immediately after start and before root dispatch.
            let mut journal = region.read_journal().unwrap();
            journal.stage = RepairStage::UserPending;
            journal.runtime_state.as_mut().unwrap().phase = Phase::StartingTarget;
            region.save_journal(&journal).unwrap();
            let lock =
                storage::LifecycleLock::try_acquire(&fixture.root.join(".claude-switcher/v2"))
                    .unwrap();
            assert!(matches!(
                region
                    .recover_runtime_with(&lock, |_| panic!("root never dispatched"), &mut fake)
                    .unwrap(),
                RecoveryOutcome::Restored
            ));
            assert_eq!(images(&fixture.root), fake.original);
            assert!(fake.running);
            assert!(!region.journal_path().exists());
        });
    }
    #[test]
    fn legacy_journal_without_runtime_state_remains_readable() {
        let fixture = Fixture::new();
        let region = coordinator(&fixture);
        with_test_home(&fixture.root, None, || {
            pending_fixture(&region, &fixture, true);
            let mut value: serde_json::Value = storage::read_json(&region.journal_path()).unwrap();
            value.as_object_mut().unwrap().remove("runtime_state");
            value.as_object_mut().unwrap().remove("app_open_plan");
            storage::atomic_json(&region.journal_path(), &value).unwrap();
            let journal = region.read_journal().unwrap();
            assert!(journal.runtime_state.is_none());
            assert!(journal.app_open_plan.is_none());
        });
    }
}
