//! Desktop and guardian share this fixed-action entry and the account lifecycle lock.
//! No production release source or signing identity has been provisioned. This module
//! therefore makes zero update requests and cannot install an unauthenticated cache.
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::{Path, PathBuf},
};
use switcher_core::{
    signed,
    storage::{self, LifecycleLock},
    time::{self, ClockSample, ClockState},
    updates::{self, UpdateSchedule, UpdateState},
};
use switcher_domain::{
    Edition,
    desktop::{OperationOutcome, UpdateAction, UpdateView},
};

const SOURCE_LOCAL: &str = "源码构建版本；升级请拉取源码、重新构建并运行本地安装脚本。";

/// Locked upstream source checked after AnySearch discovery on 2026-10-22:
/// https://raw.githubusercontent.com/tauri-apps/plugins-workspace/updater-v2.14.0/plugins/updater/src/config.rs
/// https://raw.githubusercontent.com/tauri-apps/plugins-workspace/updater-v2.14.0/plugins/updater/src/updater.rs
/// Keep endpoints empty until a release pins the real source. Do not set a comparator.
/// `restore_update` requires a signed version; `install` verifies the supplied bytes again.
pub fn strict_updater_config(pubkey: String) -> tauri_plugin_updater::Config {
    use base64::Engine;
    tauri_plugin_updater::Config {
        pubkey: base64::engine::general_purpose::STANDARD.encode(pubkey),
        require_signed_version: true,
        allow_downgrades: false,
        ..Default::default()
    }
}

/// Tauri's plugin configuration is deserialized from the application context.
/// The locked upstream Config implements Deserialize but intentionally not Serialize.
pub fn strict_updater_settings(pubkey: String) -> serde_json::Value {
    let config = strict_updater_config(pubkey);
    serde_json::json!({
        "pubkey": config.pubkey,
        "requireSignedVersion": config.require_signed_version,
        "allowDowngrades": config.allow_downgrades,
        "endpoints": [],
        "dangerousInsecureTransportProtocol": false,
        "dangerousAcceptInvalidCerts": false,
        "dangerousAcceptInvalidHostnames": false,
    })
}

/// The only production release-material entry. Provision reviewed pins at build time.
/// Both the platform plugin and the core runtime must consume this same value.
pub fn compiled_release_pins() -> Option<updates::ValidatedReleasePins> {
    None
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Persistent {
    schema_version: u32,
    edition: Edition,
    schedule: UpdateSchedule,
    #[serde(default, skip_serializing)]
    clock: ClockState,
    updates: UpdateState,
    status: String,
    message: String,
}
impl Persistent {
    fn new(edition: Edition, now: u64) -> Self {
        Self {
            schema_version: 1,
            edition,
            schedule: UpdateSchedule::new(edition, now),
            clock: ClockState::default(),
            updates: UpdateState::default(),
            status: "wait_materials".into(),
            message: "尚未检查签名更新通道。".into(),
        }
    }
}

pub struct UpdateRuntime {
    root: PathBuf,
    edition: Edition,
    pins: Option<updates::ValidatedReleasePins>,
    sample_clock: fn() -> Result<ClockSample, &'static str>,
}
impl UpdateRuntime {
    /// `root` is the switcher data root, not an account directory or user-provided URL.
    pub fn new(root: PathBuf, edition: Edition) -> Self {
        // Real release pins must be reviewed and compiled into this edition.
        // Credentials, IPC parameters and writable configuration cannot select them.
        if let Some(pins) = compiled_release_pins() {
            Self::with_release_pins(root, edition, pins)
        } else {
            Self {
                root,
                edition,
                pins: None,
                sample_clock: time::sample,
            }
        }
    }
    /// Only the desktop's reviewed build configuration may call this constructor.
    /// ValidatedReleasePins cannot arrive through serde or any registered command.
    pub(crate) fn with_release_pins(
        root: PathBuf,
        edition: Edition,
        pins: updates::ValidatedReleasePins,
    ) -> Self {
        Self {
            root,
            edition,
            pins: Some(pins),
            sample_clock: time::sample,
        }
    }
    #[cfg(test)]
    pub(crate) fn with_clock(
        mut self,
        sample_clock: fn() -> Result<ClockSample, &'static str>,
    ) -> Self {
        self.sample_clock = sample_clock;
        self
    }

    fn v2(&self) -> PathBuf {
        self.root.join("v2")
    }
    fn directory(&self) -> PathBuf {
        self.v2().join(match self.edition {
            Edition::Personal => "personal",
            Edition::Public => "public",
        })
    }
    fn state_path(&self) -> PathBuf {
        self.directory().join("updates.json")
    }
    fn lock(&self) -> io::Result<LifecycleLock> {
        storage::prepare_data_root(&self.root)?;
        storage::create_private_directory(&self.v2())?;
        let lock = LifecycleLock::try_acquire(&self.v2())?;
        storage::create_private_directory(&self.directory())?;
        Ok(lock)
    }
    fn load(&self, sample: &ClockSample) -> io::Result<Persistent> {
        let mut state = match storage::read_private_bytes(&self.state_path(), updates::STATE_LIMIT)
        {
            Ok(bytes) => {
                let state: Persistent =
                    signed::strict_json(&bytes, updates::STATE_LIMIT).map_err(io::Error::other)?;
                if state.schema_version != 1
                    || state.edition != self.edition
                    || state.schedule.edition != self.edition
                    || state.status.len() > 64
                    || state.message.len() > 2048
                {
                    return Err(io::Error::other("update_state_context"));
                }
                // Apply the same bounded replay/failure validation as core restoration.
                UpdateState::decode(&state.updates.encode().map_err(io::Error::other)?)
                    .map_err(io::Error::other)?;
                state
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Persistent::new(self.edition, utc(sample)?)
            }
            Err(error) => return Err(error),
        };
        state.clock =
            match storage::read_private_bytes(&self.directory().join("clock.json"), 64 * 1024) {
                Ok(bytes) => signed::strict_json(&bytes, 64 * 1024).map_err(io::Error::other)?,
                Err(error) if error.kind() == io::ErrorKind::NotFound => state.clock,
                Err(error) => return Err(error),
            };
        state.clock.observe_passive(sample.clone());
        // The lifecycle lock proves no updater is currently working in another process.
        state.schedule.recover_process();
        if state.updates.recover_interrupted() {
            state.status = "recovery_required".into();
            state.message = "检测到中断的更新事务。请核验已安装组件及兼容回退工件。".into();
        }
        Ok(state)
    }
    fn save(&self, state: &Persistent) -> io::Result<()> {
        let bytes = serde_json::to_vec(state).map_err(io::Error::other)?;
        if bytes.len() > updates::STATE_LIMIT {
            return Err(io::Error::other("update_state_too_large"));
        }
        write_changed(&self.state_path(), &bytes)?;
        write_changed(
            &self.directory().join("clock.json"),
            &serde_json::to_vec(&state.clock).map_err(io::Error::other)?,
        )
    }
    pub fn snapshot(&self) -> UpdateView {
        let result = (|| -> io::Result<UpdateView> {
            let _lock = self.lock()?;
            let sample = (self.sample_clock)().map_err(io::Error::other)?;
            let state = self.load(&sample)?;
            self.save(&state)?;
            let mut result = view(&state);
            if self.pins.is_none() {
                source_local_view(&mut result);
            }
            let installed = self.pins.as_ref().and_then(|_| {
                switcher_core::install::validate_installed(self.edition, unsafe { libc::geteuid() })
                    .ok()
            });
            if let Some(pins) = &self.pins
                && let Some(record) = &installed
                && pinned_installation(Ok(record.clone()), self.edition).is_ok()
                && let (Ok(app), Ok(helper)) = (
                    updates::ValidatedReleasePins::version(env!("CARGO_PKG_VERSION")),
                    updates::ValidatedReleasePins::version(&record.helper_version),
                )
                && let Ok(candidate) = pins.restore(
                    &state.updates,
                    &app,
                    Some(&helper),
                    utc(&sample)?,
                    state.clock.trusted(),
                )
            {
                result.candidate_version = Some(candidate.metadata().version.to_string());
                result.can_download = state.updates.candidate.as_ref().is_some_and(|pending| {
                    matches!(
                        pending.stage,
                        updates::InstallStage::Authenticated
                            | updates::InstallStage::Downloaded
                            | updates::InstallStage::Rejected
                    )
                });
                result.can_install =
                    state.clock.trusted()
                        && state.updates.candidate.as_ref().is_some_and(|pending| {
                            pending.stage == updates::InstallStage::Downloaded
                        });
            }
            if let Some(record) = &installed
                && record.is_source_local()
            {
                source_local_view(&mut result);
            }
            Ok(result)
        })();
        result.unwrap_or_else(|error| UpdateView {
            paused: None,
            state: error_code(&error).into(),
            message: error_message(&error),
            next_check_at: None,
            candidate_version: None,
            can_download: false,
            can_install: false,
        })
    }
    /// UI actions are individually initiated by the local user. Consent is never persisted.
    pub fn act(&self, action: UpdateAction) -> OperationOutcome {
        match action {
            UpdateAction::Check => self.run_task(updates::UpdateAction::Check, true),
            UpdateAction::Download => self.run_task(updates::UpdateAction::Download, true),
            UpdateAction::Install => self.run_task(updates::UpdateAction::Install, true),
            _ => self.change_schedule(action),
        }
    }
    /// Guardian/CLI use an enum. This entry never accepts a URL, candidate path, or shell text.
    pub fn run_task(&self, action: updates::UpdateAction, manual: bool) -> OperationOutcome {
        let result = (|| -> io::Result<OperationOutcome> {
            let lock = self.lock()?;
            let sample = (self.sample_clock)().map_err(io::Error::other)?;
            self.run_locked(action, manual, &sample, &lock)
        })();
        result.unwrap_or_else(outcome_error)
    }
    fn run_locked(
        &self,
        action: updates::UpdateAction,
        manual: bool,
        sample: &ClockSample,
        lock: &LifecycleLock,
    ) -> io::Result<OperationOutcome> {
        if action == updates::UpdateAction::Install
            && let Err(reason) = updates::ensure_installation_idle(&self.v2(), lock)
        {
            let message = match reason {
                "account_recovery_pending" => "账号事务尚未恢复完成，暂不能准备或安装更新。",
                "account_recovery_state_invalid" => {
                    "账号事务状态无法安全确认，暂不能准备或安装更新。"
                }
                "repair_recovery_pending" => "地区或配置修复尚未恢复完成，暂不能准备或安装更新。",
                _ => "无法确认事务状态，暂不能准备或安装更新。",
            };
            return Ok(outcome(false, message, Some(reason)));
        }
        let mut state = self.load(sample)?;
        if manual {
            state.clock.observe(sample.clone());
        }
        let now = utc(sample)?;
        if !state.clock.allows_verified_refresh()
            || (action != updates::UpdateAction::Check && !state.clock.trusted())
        {
            self.save(&state)?;
            return Ok(outcome(
                false,
                "系统时间需要恢复。修正时间后点击恢复，再完成固定源验签刷新。",
                Some("clock_recovery_required"),
            ));
        }
        match state.schedule.begin(action, manual, now) {
            Ok(true) => (),
            Ok(false) => {
                return Ok(outcome(
                    false,
                    "相同更新任务已经在执行。",
                    Some("update_in_progress"),
                ));
            }
            Err(reason) => {
                self.save(&state)?;
                let message = match reason {
                    "personal_requires_user_action" => {
                        "Personal 的每次检查、下载和安装均需本人操作。"
                    }
                    "updates_paused" => "自动更新已暂停。",
                    "installation_deferred" => "本次安装已延后。",
                    "check_not_due" => "尚未到达下次检查时间。",
                    _ => "更新任务目前不可执行。",
                };
                return Ok(outcome(false, message, Some(reason)));
            }
        }
        if let Some(pins) = &self.pins {
            state.clock.observe(sample.clone());
            self.save(&state)?; // Reserve the task before network or staging side effects.
            let result = self.execute_pinned(action, manual, sample, &mut state, pins, lock);
            match result {
                Ok(result) => {
                    self.save(&state)?;
                    return Ok(result);
                }
                Err(reason) => {
                    if matches!(reason, "update_network" | "update_http_status_or_redirect") {
                        state.schedule.transient_failure(now);
                    } else {
                        state.schedule.authorization_cancelled();
                    }
                    state.status = "update_failed".into();
                    state.message = if reason == "source_local_rebuild_required" {
                        SOURCE_LOCAL.into()
                    } else {
                        format!("更新未完成：{reason}。请按更新状态检查或恢复组件。")
                    };
                    self.save(&state)?;
                    return Ok(outcome(false, &state.message, Some(reason)));
                }
            }
        }
        // No transport is created while production materials are absent.
        state.schedule.authorization_cancelled();
        if action == updates::UpdateAction::Check {
            state.schedule.next_check_at = now.saturating_add(updates::CHECK_INTERVAL);
        }
        if !matches!(
            state.status.as_str(),
            "recovery_required"
                | "update_failed"
                | "wait_recovery_materials"
                | "failed_artifacts_suppressed"
        ) {
            state.status = "source_local_rebuild_required".into();
            state.message = SOURCE_LOCAL.into();
        }
        self.save(&state)?;
        Ok(outcome(
            false,
            &state.message,
            Some(match state.status.as_str() {
                "recovery_required" => "update_recovery_required",
                "update_failed" => "update_failed",
                "wait_recovery_materials" => "wait_update_recovery_pkg",
                "failed_artifacts_suppressed" => "failed_artifacts_suppressed",
                _ => "source_local_rebuild_required",
            }),
        ))
    }
    fn execute_pinned(
        &self,
        action: updates::UpdateAction,
        manual: bool,
        sample: &ClockSample,
        state: &mut Persistent,
        pins: &updates::ValidatedReleasePins,
        lock: &LifecycleLock,
    ) -> Result<OperationOutcome, &'static str> {
        if pins.edition() != self.edition {
            return Err("release_context");
        }
        let record = pinned_installation(
            switcher_core::install::validate_installed(self.edition, unsafe { libc::geteuid() }),
            self.edition,
        )?;
        let may_recover = match updates::ensure_installation_idle(&self.v2(), lock) {
            Ok(()) => true,
            Err("repair_recovery_pending") if action != updates::UpdateAction::Install => false,
            Err(reason) => return Err(reason),
        };
        let mut coordinator = switcher_core::install::NativeUpdateCoordinator::new(
            &self.v2(),
            lock,
            state.clock.clone(),
        )?;
        if may_recover {
            coordinator.recover(&mut state.updates)?;
        }
        self.save(state).map_err(|_| "update_state_write")?;
        let app = updates::ValidatedReleasePins::version(env!("CARGO_PKG_VERSION"))?;
        let helper = updates::ValidatedReleasePins::version(&record.helper_version)?;
        let now = utc(sample).map_err(|_| "utc_before_epoch")?;
        let transport = updates::ReleaseTransport::new(pins.clone())?;
        let candidate = match action {
            updates::UpdateAction::Check => {
                let candidate = transport.check_blocking(
                    &state.updates,
                    &app,
                    Some(&helper),
                    now,
                    state.clock.allows_verified_refresh(),
                )?;
                let refreshed = time::sample()?;
                state.clock.observe(refreshed.clone());
                if !state.clock.allows_verified_refresh() {
                    return Err("clock_recovery_required");
                }
                let fresh_now = utc(&refreshed).map_err(|_| "utc_before_epoch")?;
                if candidate.metadata().expires_at <= fresh_now
                    || candidate.metadata().issued_at > fresh_now
                {
                    return Err("metadata_expired");
                }
                let accepted = state.updates.accept(&candidate, false)?;
                state.clock.record_verified_success(refreshed)?;
                state.schedule.success(action, now)?;
                self.save(state).map_err(|_| "update_state_write")?;
                if !accepted {
                    state.status = "failed_artifacts_suppressed".into();
                    state.message =
                        "该 app/helper 工件组合此前失败。请完成恢复后再显式重试。".into();
                    return Ok(outcome(
                        false,
                        &state.message,
                        Some("failed_artifacts_suppressed"),
                    ));
                }
                if manual {
                    state.status = "authenticated".into();
                    state.message = format!(
                        "已验签更新 {}。下载需再次操作。",
                        candidate.metadata().version
                    );
                    return Ok(outcome(true, &state.message, None));
                }
                // Public may download automatically; Personal cannot enter this branch.
                state
                    .schedule
                    .begin(updates::UpdateAction::Download, false, now)?;
                self.download_candidate(&transport, &candidate, state, now)?;
                let sample = time::sample()?;
                let fresh_now = utc(&sample).map_err(|_| "utc_before_epoch")?;
                state.clock.observe(sample.clone());
                if !state.clock.trusted() {
                    return Err("clock_recovery_required");
                }
                if let Err(reason) =
                    state
                        .schedule
                        .begin(updates::UpdateAction::Install, false, fresh_now)
                {
                    return Ok(outcome(
                        true,
                        "更新已暂存。自动安装正在暂停或延后。",
                        Some(reason),
                    ));
                }
                self.save(state).map_err(|_| "update_state_write")?;
                return self.execute_pinned(
                    updates::UpdateAction::Install,
                    false,
                    &sample,
                    state,
                    pins,
                    lock,
                );
            }
            _ => pins.restore(
                &state.updates,
                &app,
                Some(&helper),
                now,
                state.clock.allows_verified_refresh(),
            )?,
        };
        if action == updates::UpdateAction::Download {
            if manual {
                state.updates.accept(&candidate, true)?;
            }
            self.download_candidate(&transport, &candidate, state, now)?;
            return Ok(outcome(true, "已验证并暂存更新。安装需再次操作。", None));
        }
        updates::ensure_installation_idle(&self.v2(), lock)?;
        let staging = self
            .directory()
            .join("update-staging")
            .join(candidate.payload_hash());
        let app_bytes = storage::read_private_bytes(
            &staging.join("app.tar.gz"),
            updates::MAX_ARTIFACT_SIZE as usize,
        )
        .map_err(|_| "cached_app_missing")?;
        let app_proof =
            transport.verify_cached(&candidate, updates::ArtifactKind::App, &app_bytes)?;
        let helper_proof = if candidate.metadata().helper.is_some() {
            let bytes = storage::read_private_bytes(
                &staging.join("helper.pkg"),
                updates::MAX_ARTIFACT_SIZE as usize,
            )
            .map_err(|_| "cached_helper_missing")?;
            Some(transport.verify_cached(&candidate, updates::ArtifactKind::Helper, &bytes)?)
        } else {
            None
        };
        let serial = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "utc_before_epoch")?
            .as_nanos();
        let expanded = staging.join(format!("app-expanded-{}-{serial}", std::process::id()));
        let app_path = updates::stage_app_archive(&candidate, &app_proof, &app_bytes, &expanded)?;
        let helper_path = staging.join("helper.pkg");
        let prepared = updates::assess_staged_installation(
            candidate,
            &app_proof,
            &app_path,
            helper_proof
                .as_ref()
                .map(|proof| (proof, helper_path.as_path())),
        );
        match prepared {
            Err(reason) => {
                let _ = std::fs::remove_dir_all(&expanded);
                // Apple failure is not successful installation. Quarantine this candidate's
                // bytes by removing the private staging tree; retain failure and sequence.
                let restored = pins.restore(&state.updates, &app, Some(&helper), now, true)?;
                state.updates.record_failure(&restored, reason, now)?;
                let _ = std::fs::remove_dir_all(&staging);
                Err(reason)
            }
            Ok(prepared) => {
                use updates::UpdateInstallCoordinator;
                let installed = coordinator.install(&prepared, &mut state.updates);
                let _ = std::fs::remove_dir_all(&expanded);
                if let Err("wait_update_recovery_pkg") = installed {
                    state.schedule.authorization_cancelled();
                    state.status = "wait_recovery_materials".into();
                    state.message = "等待已验证的上一兼容版本恢复 PKG；尚未开始安装。".into();
                    return Ok(outcome(
                        false,
                        &state.message,
                        Some("wait_update_recovery_pkg"),
                    ));
                }
                installed?;
                if !state
                    .updates
                    .candidate
                    .as_ref()
                    .is_some_and(|pending| pending.stage == updates::InstallStage::Healthy)
                {
                    return Err("update_health_unconfirmed");
                }
                state
                    .schedule
                    .success(updates::UpdateAction::Install, now)?;
                state.status = "installed".into();
                state.message =
                    "更新已安装，组件身份和后台健康检查已通过。请重新打开切换器。".into();
                Ok(outcome(true, &state.message, None))
            }
        }
    }
    fn download_candidate(
        &self,
        transport: &updates::ReleaseTransport,
        candidate: &updates::Candidate,
        state: &mut Persistent,
        now: u64,
    ) -> Result<(), &'static str> {
        let download = |kind| transport.download_blocking(candidate, kind, now);
        let record_failure = |state: &mut Persistent, reason| {
            if matches!(
                reason,
                "artifact_size"
                    | "artifact_sha256"
                    | "update_response_too_large"
                    | "minisign_shape"
                    | "minisign_signature"
                    | "minisign_signed_version"
            ) {
                state.updates.record_failure(candidate, reason, now)?;
            }
            Err::<_, &'static str>(reason)
        };
        let (app, app_proof) = match download(updates::ArtifactKind::App) {
            Ok(value) => value,
            Err(reason) => return record_failure(state, reason),
        };
        let helper = if candidate.metadata().helper.is_some() {
            Some(match download(updates::ArtifactKind::Helper) {
                Ok(value) => value,
                Err(reason) => return record_failure(state, reason),
            })
        } else {
            None
        };
        let sample = time::sample()?;
        state.clock.observe(sample.clone());
        if !state.clock.trusted() {
            return Err("clock_recovery_required");
        }
        let fresh_now = utc(&sample).map_err(|_| "utc_before_epoch")?;
        if fresh_now >= candidate.metadata().expires_at
            || fresh_now < candidate.metadata().issued_at
        {
            return Err("metadata_expired");
        }
        let parent = self.directory().join("update-staging");
        storage::create_private_directory(&parent).map_err(|_| "staging_directory")?;
        let staging = parent.join(candidate.payload_hash());
        storage::create_private_directory(&staging).map_err(|_| "staging_directory")?;
        let written = (|| {
            storage::atomic_write(&staging.join("app.tar.gz"), &app)
                .map_err(|_| "staging_write")?;
            if let Some((bytes, _)) = &helper {
                storage::atomic_write(&staging.join("helper.pkg"), bytes)
                    .map_err(|_| "staging_write")?;
            }
            state.updates.mark_downloaded(
                candidate,
                &app_proof,
                helper.as_ref().map(|(_, proof)| proof),
            )?;
            Ok::<_, &'static str>(())
        })();
        if written.is_err() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        written?;
        state
            .schedule
            .success(updates::UpdateAction::Download, now)?;
        state.status = "downloaded".into();
        state.message = "工件已通过 SHA256、Minisign 和签名版本验证。尚未安装。".into();
        Ok(())
    }
    fn change_schedule(&self, action: UpdateAction) -> OperationOutcome {
        let result = (|| -> io::Result<OperationOutcome> {
            let _lock = self.lock()?;
            let sample = (self.sample_clock)().map_err(io::Error::other)?;
            let now = utc(&sample)?;
            let mut state = self.load(&sample)?;
            state.clock.observe(sample.clone());
            let message = match action {
                UpdateAction::Pause => {
                    state.schedule.paused = true;
                    "已暂停自动检查、下载和安装。"
                }
                UpdateAction::Resume => {
                    state.schedule.paused = false;
                    if !state.clock.trusted() {
                        state.clock.establish_baseline(sample);
                        "已建立本地时间基线。等待固定更新源验签刷新。"
                    } else {
                        "已恢复更新调度。"
                    }
                }
                UpdateAction::Defer => {
                    state.schedule.deferred_until = Some(now.saturating_add(24 * 60 * 60));
                    "已将自动安装延后 24 小时。"
                }
                _ => return Err(io::Error::other("unsupported_schedule_action")),
            };
            self.save(&state)?;
            Ok(outcome(true, message, None))
        })();
        result.unwrap_or_else(outcome_error)
    }
}
// This gate consumes the existing installed-identity validator's result. It never
// substitutes a writable record or a branch flag for native identity validation.
fn pinned_installation(
    installed: Result<switcher_domain::system::InstallRecord, &'static str>,
    edition: Edition,
) -> Result<switcher_domain::system::InstallRecord, &'static str> {
    let record = installed.map_err(|_| "installed_identity_required")?;
    record
        .validate()
        .map_err(|_| "installed_identity_required")?;
    if record.edition != edition {
        return Err("installed_identity_required");
    }
    record.developer_team()?;
    Ok(record)
}
fn source_local_view(result: &mut UpdateView) {
    if matches!(
        result.state.as_str(),
        "wait_materials" | "source_local_rebuild_required" | "checked"
    ) {
        result.state = "source_local_rebuild_required".into();
        result.message = SOURCE_LOCAL.into();
    } else if !result.message.contains("源码构建版本") {
        result.message.push(' ');
        result.message.push_str(SOURCE_LOCAL);
    }
    result.next_check_at = None;
    result.candidate_version = None;
    result.can_download = false;
    result.can_install = false;
}
fn utc(sample: &ClockSample) -> io::Result<u64> {
    sample
        .utc_secs
        .try_into()
        .map_err(|_| io::Error::other("utc_before_epoch"))
}
fn write_changed(path: &Path, bytes: &[u8]) -> io::Result<()> {
    match storage::read_private_bytes(path, updates::STATE_LIMIT) {
        Ok(old) if old == bytes => return Ok(()),
        Ok(_) => (),
        Err(error) if error.kind() == io::ErrorKind::NotFound => (),
        Err(error) => return Err(error),
    }
    storage::atomic_write(path, bytes)
}
fn view(state: &Persistent) -> UpdateView {
    let (status, message) = if !state.clock.trusted() {
        (
            "clock_recovery_required",
            "系统时间需要恢复。修正时间后点击恢复，再完成固定源验签刷新。",
        )
    } else if state.schedule.paused {
        ("paused", "自动更新已暂停。")
    } else {
        (state.status.as_str(), state.message.as_str())
    };
    UpdateView {
        paused: Some(state.schedule.paused),
        state: status.into(),
        message: message.into(),
        next_check_at: if state.edition == Edition::Public {
            Some(state.schedule.next_check_at)
        } else {
            None
        },
        candidate_version: None,
        can_download: false,
        can_install: false,
    }
}
fn outcome(completed: bool, message: &str, recovery: Option<&str>) -> OperationOutcome {
    OperationOutcome {
        completed,
        message: message.into(),
        recovery: recovery.map(str::to_owned),
    }
}
fn error_code(error: &io::Error) -> &'static str {
    if error.kind() == io::ErrorKind::WouldBlock {
        "lifecycle_busy"
    } else {
        "update_storage_error"
    }
}
fn error_message(error: &io::Error) -> String {
    if error.kind() == io::ErrorKind::WouldBlock {
        "账号或更新任务持有生命周期锁。请稍后重试。".into()
    } else {
        "无法安全读取或保存更新状态。请检查 v2 更新目录的权限和恢复记录。".into()
    }
}
fn outcome_error(error: io::Error) -> OperationOutcome {
    outcome(false, &error_message(&error), Some(error_code(&error)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let parent = std::env::temp_dir().canonicalize().unwrap();
            let path = parent.join(format!(
                "switcher-update-runtime-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            Self::at(path).unwrap()
        }
        fn at(path: PathBuf) -> std::io::Result<Self> {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new().mode(0o700).create(&path)?;
            Ok(Self(path))
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn fixture_clock() -> Result<ClockSample, &'static str> {
        Ok(at(1_700_000_000))
    }
    fn fixture_runtime(root: PathBuf, edition: Edition) -> UpdateRuntime {
        UpdateRuntime::new(root, edition).with_clock(fixture_clock)
    }
    fn at(utc: u64) -> ClockSample {
        ClockSample {
            utc_secs: utc as i64,
            continuous_ms: utc * 1000,
            boot_id: "test-boot".into(),
        }
    }
    #[test]
    fn failed_clock_preserves_persisted_state_and_denies_all_entrypoints() {
        let fixture = Fixture::new();
        let runtime = fixture_runtime(fixture.0.clone(), Edition::Personal);
        assert_eq!(runtime.snapshot().paused, Some(false));
        let paths = [runtime.state_path(), runtime.directory().join("clock.json")];
        let before = paths.each_ref().map(|path| std::fs::read(path).unwrap());
        let failed = runtime.with_clock(|| Err("synthetic_clock_unavailable"));
        assert_eq!(failed.snapshot().state, "update_storage_error");
        for result in [
            failed.run_task(updates::UpdateAction::Check, false),
            failed.act(UpdateAction::Pause),
        ] {
            assert!(!result.completed);
            assert_eq!(result.recovery.as_deref(), Some("update_storage_error"));
        }
        assert_eq!(
            before,
            paths.each_ref().map(|path| std::fs::read(path).unwrap())
        );
    }

    #[test]
    fn anomalous_snapshot_persists_sticky_recovery_without_clearing_history() {
        for edition in [Edition::Personal, Edition::Public] {
            let fixture = Fixture::new();
            let runtime = fixture_runtime(fixture.0.clone(), edition);
            let anchor = {
                let _lock = runtime.lock().unwrap();
                let mut sample = fixture_clock().unwrap();
                sample.utc_secs += 600;
                let mut state = runtime.load(&sample).unwrap();
                state.clock.record_verified_success(sample.clone()).unwrap();
                state.updates.sequence.remember(7, "a".repeat(64)).unwrap();
                runtime.save(&state).unwrap();
                sample.utc_secs
            };
            assert_eq!(runtime.snapshot().state, "clock_recovery_required");
            let restarted = fixture_runtime(fixture.0.clone(), edition);
            assert_eq!(restarted.snapshot().state, "clock_recovery_required");
            assert!(restarted.act(UpdateAction::Resume).completed);
            let _lock = restarted.lock().unwrap();
            let restored = restarted.load(&fixture_clock().unwrap()).unwrap();
            assert_eq!(
                restored.clock.status,
                switcher_core::time::ClockStatus::AwaitingVerifiedRefresh
            );
            assert_eq!(restored.clock.last_success_utc, Some(anchor));
            assert_eq!(restored.updates.sequence.highest, 7);
        }
    }

    #[test]
    fn alternating_passive_runtimes_preserve_shared_files_both_editions() {
        use std::os::unix::fs::MetadataExt;
        for edition in [Edition::Personal, Edition::Public] {
            let fixture = Fixture::new();
            let updater = fixture_runtime(fixture.0.clone(), edition);
            let ads = switcher_core::ad_runtime::AdRuntime::new(fixture.0.clone(), edition)
                .with_clock(fixture_clock);
            let preferences = switcher_domain::ads::AdPreferences::default();
            assert!(updater.snapshot().paused.is_some());
            ads.snapshot(&preferences).unwrap();
            let capture = |path: &std::path::Path| {
                let metadata = std::fs::symlink_metadata(path).unwrap();
                (
                    std::fs::read(path).unwrap(),
                    metadata.ino(),
                    metadata.mtime(),
                    metadata.mtime_nsec(),
                )
            };
            let paths = [
                updater.state_path(),
                updater.directory().join("clock.json"),
                updater.directory().join("ads/state.json"),
            ];
            let before = paths.each_ref().map(|path| capture(path));
            for _ in 0..3 {
                std::thread::sleep(std::time::Duration::from_millis(3));
                assert!(updater.snapshot().paused.is_some());
                ads.snapshot(&preferences).unwrap();
                ads.image_bytes(switcher_domain::AdSlot::ALL[0], &preferences)
                    .unwrap();
            }
            assert_eq!(before, paths.each_ref().map(|path| capture(path)));
        }
    }

    #[test]
    fn background_no_work_preserves_clock_and_recovers_in_flight() {
        for edition in [Edition::Personal, Edition::Public] {
            let fixture = Fixture::new();
            let runtime = fixture_runtime(fixture.0.clone(), edition);
            let lock = runtime.lock().unwrap();
            let mut state = runtime.load(&at(100)).unwrap();
            state.updates.sequence.remember(7, "a".repeat(64)).unwrap();
            runtime.save(&state).unwrap();
            let clock_path = runtime.directory().join("clock.json");
            let before = std::fs::read(&clock_path).unwrap();
            for now in [101, 100 + updates::CHECK_INTERVAL] {
                runtime
                    .run_locked(updates::UpdateAction::Check, false, &at(now), &lock)
                    .unwrap();
                assert_eq!(std::fs::read(&clock_path).unwrap(), before);
            }
            let mut state = runtime.load(&at(100 + updates::CHECK_INTERVAL)).unwrap();
            state
                .schedule
                .begin(
                    updates::UpdateAction::Check,
                    true,
                    100 + updates::CHECK_INTERVAL,
                )
                .unwrap();
            runtime.save(&state).unwrap();
            let interrupted = std::fs::read(runtime.state_path()).unwrap();
            let restored = runtime.load(&at(101 + updates::CHECK_INTERVAL)).unwrap();
            runtime.save(&restored).unwrap();
            assert_ne!(std::fs::read(runtime.state_path()).unwrap(), interrupted);
            assert_eq!(restored.updates.sequence.highest, 7);
            assert_eq!(std::fs::read(&clock_path).unwrap(), before);
        }
    }

    #[test]
    fn passive_snapshots_preserve_initialized_files_both_editions() {
        use std::os::unix::fs::MetadataExt;
        let capture = |p: &Path| {
            let m = std::fs::symlink_metadata(p).unwrap();
            (
                std::fs::read(p).unwrap(),
                m.ino(),
                m.mtime(),
                m.mtime_nsec(),
            )
        };
        for edition in [Edition::Personal, Edition::Public] {
            let fixture = Fixture::new();
            let runtime = fixture_runtime(fixture.0.clone(), edition);
            runtime.snapshot();
            let paths = [runtime.state_path(), runtime.directory().join("clock.json")];
            let before = paths.each_ref().map(|p| capture(p));
            for _ in 0..3 {
                std::thread::sleep(std::time::Duration::from_millis(3));
                runtime.snapshot();
            }
            assert_eq!(before, paths.each_ref().map(|p| capture(p)));
        }
    }

    #[test]
    fn source_local_unconfigured_both_editions_have_rebuild_guidance_and_no_candidate() {
        for edition in [Edition::Personal, Edition::Public] {
            let fixture = Fixture::new();
            let runtime = fixture_runtime(fixture.0.clone(), edition);
            let snapshot = runtime.snapshot();
            assert_eq!(snapshot.state, "source_local_rebuild_required");
            assert!(
                snapshot
                    .message
                    .contains("源码构建版本；升级请拉取源码、重新构建并运行本地安装脚本")
            );
            assert!(!snapshot.message.contains("Apple"));
            assert_eq!(snapshot.next_check_at, None);
            assert_eq!(snapshot.candidate_version, None);
            assert!(!snapshot.can_download && !snapshot.can_install);
            assert!(runtime.pins.is_none());
            for action in [
                updates::UpdateAction::Check,
                updates::UpdateAction::Download,
                updates::UpdateAction::Install,
            ] {
                let lock = runtime.lock().unwrap();
                let result = runtime
                    .run_locked(action, true, &fixture_clock().unwrap(), &lock)
                    .unwrap();
                assert_eq!(
                    result.recovery.as_deref(),
                    Some("source_local_rebuild_required")
                );
                assert!(!runtime.directory().join("update-staging").exists());
                assert!(
                    !runtime
                        .v2()
                        .join("update-install-transaction.json")
                        .exists()
                );
            }
        }
    }
    #[test]
    fn source_notice_preserves_pause_clock_and_recovery_failures() {
        for status in [
            "paused",
            "clock_recovery_required",
            "recovery_required",
            "update_failed",
            "wait_recovery_materials",
            "failed_artifacts_suppressed",
        ] {
            let mut snapshot = view(&Persistent::new(Edition::Public, 100));
            snapshot.state = status.into();
            snapshot.message = "原有阻塞证据".into();
            source_local_view(&mut snapshot);
            assert_eq!(snapshot.state, status);
            assert!(snapshot.message.starts_with("原有阻塞证据"));
            assert!(snapshot.message.contains("源码构建版本"));
            assert!(!snapshot.can_install && !snapshot.can_download);
        }
    }
    #[test]
    fn unconfigured_actions_preserve_recovery_and_failure_reasons() {
        for (status, reason) in [
            ("recovery_required", "update_recovery_required"),
            ("update_failed", "update_failed"),
            ("wait_recovery_materials", "wait_update_recovery_pkg"),
            ("failed_artifacts_suppressed", "failed_artifacts_suppressed"),
        ] {
            let fixture = Fixture::new();
            let runtime = fixture_runtime(fixture.0.clone(), Edition::Public);
            let lock = runtime.lock().unwrap();
            let mut state = Persistent::new(Edition::Public, 100);
            state.status = status.into();
            state.message = "保留原有恢复证据".into();
            runtime.save(&state).unwrap();
            let result = runtime
                .run_locked(updates::UpdateAction::Check, true, &at(100), &lock)
                .unwrap();
            assert_eq!(result.recovery.as_deref(), Some(reason));
            assert_eq!(result.message, "保留原有恢复证据");
            assert_eq!(runtime.load(&at(100)).unwrap().status, status);
        }
    }
    fn installed_fixture(edition: Edition) -> switcher_domain::system::InstallRecord {
        use switcher_domain::system::{InstallRecord, InstallTrust};
        InstallRecord {
            schema_version: 2,
            owner_uid: 501,
            edition,
            trust: InstallTrust::SourceLocal {
                generation: "a".repeat(32),
                gui_cdhash: "1".repeat(40),
                guardian_cdhash: "2".repeat(40),
                helper_cdhash: "3".repeat(40),
            },
            helper_version: "0.2.0".into(),
            minimum_app_version: "0.2.0".into(),
        }
    }
    #[test]
    fn pinned_channel_requires_valid_developer_identity_before_recovery() {
        use switcher_domain::system::InstallTrust;
        for edition in [Edition::Personal, Edition::Public] {
            let mut record = installed_fixture(edition);
            assert_eq!(
                pinned_installation(Ok(record.clone()), edition),
                Err("source_local_rebuild_required")
            );
            record.schema_version = 1;
            assert_eq!(
                pinned_installation(Ok(record.clone()), edition),
                Err("installed_identity_required")
            );
            record.schema_version = 2;
            record.trust = InstallTrust::DeveloperId {
                team_id: "TESTTEAM01".into(),
            };
            assert_eq!(
                pinned_installation(Ok(record.clone()), edition),
                Ok(record.clone())
            );
            record.trust = InstallTrust::DeveloperId {
                team_id: "invalid".into(),
            };
            assert_eq!(
                pinned_installation(Ok(record), edition),
                Err("installed_identity_required")
            );
            assert_eq!(
                pinned_installation(Err("unknown_record"), edition),
                Err("installed_identity_required")
            );
        }
    }
    #[test]
    fn e11_consent_completed_personal_check_cannot_authorize_download_or_install_after_restart() {
        let fixture = Fixture::new();
        let runtime = fixture_runtime(fixture.0.clone(), Edition::Personal);
        {
            let _lock = runtime.lock().unwrap();
            let mut state = runtime.load(&at(100)).unwrap();
            // Precondition only: emulate a completed Check schedule. Production pins
            // are absent, so this deliberately does not claim a successful network Check.
            assert_eq!(
                state
                    .schedule
                    .begin(updates::UpdateAction::Check, true, 100),
                Ok(true)
            );
            state
                .schedule
                .success(updates::UpdateAction::Check, 101)
                .unwrap();
            state.status = "checked".into();
            runtime.save(&state).unwrap();
        }
        drop(runtime);
        for (index, action) in [
            updates::UpdateAction::Download,
            updates::UpdateAction::Install,
        ]
        .into_iter()
        .enumerate()
        {
            let sample = 102 + index as u64 * 10;
            let restarted = fixture_runtime(fixture.0.clone(), Edition::Personal);
            let lock = restarted.lock().unwrap();
            let restored = restarted.load(&at(sample)).unwrap();
            assert_eq!(restored.status, "checked");
            assert_eq!(
                restored.schedule.next_check_at,
                101 + updates::CHECK_INTERVAL
            );
            let before = std::fs::read(restarted.state_path()).unwrap();
            let result = restarted
                .run_locked(action, false, &at(sample), &lock)
                .unwrap();
            assert!(!result.completed);
            assert_eq!(
                result.recovery.as_deref(),
                Some("personal_requires_user_action")
            );
            assert_eq!(std::fs::read(restarted.state_path()).unwrap(), before);
            // Positive gate control: a fresh manual action reaches the separate
            // missing-materials guard instead of inheriting any earlier consent.
            let manual = restarted
                .run_locked(action, true, &at(sample + 1), &lock)
                .unwrap();
            assert!(!manual.completed);
            assert_eq!(
                manual.recovery.as_deref(),
                Some("source_local_rebuild_required")
            );
            // Restore only the test precondition for the independent next action.
            let mut state = restarted.load(&at(sample + 2)).unwrap();
            state.status = "checked".into();
            restarted.save(&state).unwrap();
        }
    }
    #[test]
    fn legacy_0755_root_supports_both_runtimes_without_changing_legacy_data() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        use switcher_core::ad_runtime::AdRuntime;
        use switcher_domain::ads::AdPreferences;
        let fixture = Fixture::new();
        storage::create_private_directory(&fixture.0).unwrap();
        std::fs::set_permissions(&fixture.0, std::fs::Permissions::from_mode(0o755)).unwrap();
        let accounts = fixture.0.join("accounts");
        storage::create_private_directory(&accounts).unwrap();
        std::fs::set_permissions(&accounts, std::fs::Permissions::from_mode(0o755)).unwrap();
        storage::atomic_write(&accounts.join("legacy-data"), b"opaque-legacy-fixture").unwrap();
        let preferences = AdPreferences {
            slots_enabled: [false; 3],
            request_purpose_acknowledged: true,
        };
        for edition in [Edition::Personal, Edition::Public] {
            let ads = AdRuntime::new(fixture.0.clone(), edition).with_clock(fixture_clock);
            assert_eq!(
                ads.refresh_if_due(&preferences, false).unwrap().reason,
                "ads_disabled"
            );
            assert!(ads.snapshot(&preferences).unwrap().is_none());
            let updates = fixture_runtime(fixture.0.clone(), edition);
            assert_eq!(updates.snapshot().state, "source_local_rebuild_required");
            assert_eq!(
                updates
                    .run_task(updates::UpdateAction::Check, true)
                    .recovery
                    .as_deref(),
                Some("source_local_rebuild_required")
            );
            let directory = updates.directory();
            for path in [
                fixture.0.join("v2"),
                directory.clone(),
                directory.join("ads"),
                directory.join("ads/assets"),
            ] {
                assert_eq!(std::fs::metadata(path).unwrap().mode() & 0o777, 0o700);
            }
            for path in [
                directory.join("updates.json"),
                directory.join("clock.json"),
                directory.join("ads/state.json"),
            ] {
                assert_eq!(std::fs::metadata(path).unwrap().mode() & 0o777, 0o600);
            }
        }
        assert_eq!(std::fs::metadata(&fixture.0).unwrap().mode() & 0o777, 0o755);
        assert_eq!(std::fs::metadata(&accounts).unwrap().mode() & 0o777, 0o755);
        assert_eq!(
            std::fs::read(accounts.join("legacy-data")).unwrap(),
            b"opaque-legacy-fixture"
        );
    }
    #[test]
    fn unsafe_roots_and_nonprivate_v2_fail_closed_in_both_runtimes() {
        use std::os::unix::fs::PermissionsExt;
        use switcher_core::ad_runtime::AdRuntime;
        use switcher_domain::ads::AdPreferences;
        let fixture = Fixture::new();
        storage::create_private_directory(&fixture.0).unwrap();
        let preferences = AdPreferences::default();
        for mode in [0o775, 0o757] {
            std::fs::set_permissions(&fixture.0, std::fs::Permissions::from_mode(mode)).unwrap();
            assert!(
                AdRuntime::new(fixture.0.clone(), Edition::Personal)
                    .snapshot(&preferences)
                    .is_err()
            );
            assert_eq!(
                fixture_runtime(fixture.0.clone(), Edition::Public)
                    .snapshot()
                    .state,
                "update_storage_error"
            );
            assert!(!fixture.0.join("v2").exists());
        }
        std::fs::set_permissions(&fixture.0, std::fs::Permissions::from_mode(0o755)).unwrap();
        let link = fixture.0.with_extension("link");
        std::os::unix::fs::symlink(&fixture.0, &link).unwrap();
        assert!(
            AdRuntime::new(link.clone(), Edition::Personal)
                .snapshot(&preferences)
                .is_err()
        );
        assert_eq!(
            fixture_runtime(link.clone(), Edition::Public)
                .snapshot()
                .state,
            "update_storage_error"
        );
        std::fs::remove_file(link).unwrap();
        storage::create_private_directory(&fixture.0.join("v2")).unwrap();
        std::fs::set_permissions(fixture.0.join("v2"), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        assert!(
            AdRuntime::new(fixture.0.clone(), Edition::Personal)
                .snapshot(&preferences)
                .is_err()
        );
        assert_eq!(
            fixture_runtime(fixture.0.clone(), Edition::Public)
                .snapshot()
                .state,
            "update_storage_error"
        );
    }
    #[test]
    fn pending_repairs_block_install_before_state_changes_but_allow_check() {
        let fixture = Fixture::new();
        let runtime = fixture_runtime(fixture.0.clone(), Edition::Personal);
        let lock = runtime.lock().unwrap();
        for journal in ["region-repair.json", "user-repairs/journal.json"] {
            let path = runtime.v2().join(journal);
            storage::create_private_directory(path.parent().unwrap()).unwrap();
            storage::atomic_write(&path, b"unknown-root-result").unwrap();
            let before = std::fs::read(runtime.state_path()).ok();
            let denied = runtime
                .run_locked(updates::UpdateAction::Install, true, &at(100), &lock)
                .unwrap();
            assert!(!denied.completed);
            assert_eq!(denied.recovery.as_deref(), Some("repair_recovery_pending"));
            assert_eq!(
                denied.message,
                "地区或配置修复尚未恢复完成，暂不能准备或安装更新。"
            );
            assert_eq!(std::fs::read(runtime.state_path()).ok(), before);
            assert_eq!(std::fs::read(&path).unwrap(), b"unknown-root-result");
            let check = runtime
                .run_locked(updates::UpdateAction::Check, true, &at(100), &lock)
                .unwrap();
            assert_eq!(
                check.recovery.as_deref(),
                Some("source_local_rebuild_required")
            );
            std::fs::remove_file(path).unwrap();
            let allowed = runtime
                .run_locked(updates::UpdateAction::Install, true, &at(100), &lock)
                .unwrap();
            assert_eq!(
                allowed.recovery.as_deref(),
                Some("source_local_rebuild_required")
            );
        }
    }
    #[test]
    fn update_feedback_pending_account_journals() {
        let fixture = Fixture::new();
        let runtime = fixture_runtime(fixture.0.clone(), Edition::Personal);
        let lock = runtime.lock().unwrap();
        for journal in [
            "account-journal.json",
            "legacy-account-journal.json",
            "migration/in-place-journal.json",
        ] {
            let path = runtime.v2().join(journal);
            storage::create_private_directory(path.parent().unwrap()).unwrap();
            storage::atomic_write(&path, b"synthetic-journal").unwrap();
            let before = std::fs::read(runtime.state_path()).ok();
            let denied = runtime
                .run_locked(updates::UpdateAction::Install, true, &at(100), &lock)
                .unwrap();
            assert!(!denied.completed);
            assert_eq!(denied.recovery.as_deref(), Some("account_recovery_pending"));
            assert!(denied.message.contains("账号"), "{}", denied.message);
            assert_eq!(std::fs::read(runtime.state_path()).ok(), before);
            assert_eq!(std::fs::read(&path).unwrap(), b"synthetic-journal");
            let check = runtime
                .run_locked(updates::UpdateAction::Check, true, &at(100), &lock)
                .unwrap();
            assert_eq!(
                check.recovery.as_deref(),
                Some("source_local_rebuild_required")
            );
            std::fs::remove_file(path).unwrap();
        }
    }
    #[test]
    fn update_feedback_invalid_account_state() {
        let fixture = Fixture::new();
        let runtime = fixture_runtime(fixture.0.clone(), Edition::Personal);
        let lock = runtime.lock().unwrap();
        let target = fixture.0.join("synthetic-target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("sentinel"), b"untouched").unwrap();
        let migration = runtime.v2().join("migration");
        std::os::unix::fs::symlink(&target, &migration).unwrap();
        let before = std::fs::read(runtime.state_path()).ok();
        let denied = runtime
            .run_locked(updates::UpdateAction::Install, true, &at(100), &lock)
            .unwrap();
        assert!(!denied.completed);
        assert_eq!(
            denied.recovery.as_deref(),
            Some("account_recovery_state_invalid")
        );
        assert!(denied.message.contains("账号"), "{}", denied.message);
        assert_eq!(std::fs::read(runtime.state_path()).ok(), before);
        assert_eq!(std::fs::read_link(migration).unwrap(), target);
        assert_eq!(
            std::fs::read(target.join("sentinel")).unwrap(),
            b"untouched"
        );
    }
    #[test]
    fn update_feedback_fixture_collision_does_not_claim_or_delete_directory() {
        let parent = Fixture::new();
        std::fs::create_dir_all(&parent.0).unwrap();
        let collision = parent.0.join("preexisting");
        std::fs::create_dir(&collision).unwrap();
        let sentinel = collision.join("sentinel");
        std::fs::write(&sentinel, b"untouched").unwrap();
        let claimed = Fixture::at(collision);
        let refused = claimed.is_err();
        drop(claimed);
        assert_eq!(
            std::fs::read(sentinel).ok().as_deref(),
            Some(&b"untouched"[..])
        );
        assert!(refused);
    }
    #[test]
    fn pinned_updater_policy_cannot_downgrade_or_skip_signed_version() {
        use base64::Engine;
        let config = strict_updater_config("production-key-not-provisioned".into());
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(&config.pubkey)
                .unwrap(),
            b"production-key-not-provisioned"
        );
        assert!(compiled_release_pins().is_none());
        assert!(config.require_signed_version);
        assert!(!config.allow_downgrades);
        assert!(config.endpoints.is_empty());
        assert!(!config.dangerous_accept_invalid_certs);
        assert!(!config.dangerous_accept_invalid_hostnames);
        assert!(!config.dangerous_insecure_transport_protocol);
        let settings = strict_updater_settings("production-key-not-provisioned".into());
        let configured: tauri_plugin_updater::Config = serde_json::from_value(settings).unwrap();
        assert_eq!(configured.pubkey, config.pubkey);
        assert!(configured.require_signed_version);
        assert!(!configured.allow_downgrades);
        assert!(configured.endpoints.is_empty());
        assert!(!configured.dangerous_accept_invalid_certs);
        assert!(!configured.dangerous_accept_invalid_hostnames);
        assert!(!configured.dangerous_insecure_transport_protocol);
    }
    #[test]
    fn missing_materials_and_personal_guardian_make_zero_requests_and_survive_restart() {
        let fixture = Fixture::new();
        let personal = fixture_runtime(fixture.0.clone(), Edition::Personal);
        {
            let _lock = personal.lock().unwrap();
            let denied = personal
                .run_locked(updates::UpdateAction::Check, false, &at(100), &_lock)
                .unwrap();
            assert_eq!(
                denied.recovery.as_deref(),
                Some("personal_requires_user_action")
            );
            let wait = personal
                .run_locked(updates::UpdateAction::Check, true, &at(101), &_lock)
                .unwrap();
            assert!(!wait.completed);
            assert_eq!(
                wait.recovery.as_deref(),
                Some("source_local_rebuild_required")
            );
        }
        let restarted = fixture_runtime(fixture.0.clone(), Edition::Personal);
        let _lock = restarted.lock().unwrap();
        let state = restarted.load(&at(102)).unwrap();
        assert_eq!(state.updates.sequence.highest, 0);
        assert!(state.updates.candidate.is_none());
        assert_eq!(state.status, "source_local_rebuild_required");
        assert!(view(&state).next_check_at.is_none());
        assert!(!view(&state).can_install);
    }
    #[test]
    fn public_fixed_task_persists_due_time_and_global_lock_merges_duplicate_processes() {
        let fixture = Fixture::new();
        let runtime = fixture_runtime(fixture.0.clone(), Edition::Public);
        let _lock = runtime.lock().unwrap();
        assert_eq!(
            runtime.lock().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        let early = runtime
            .run_locked(updates::UpdateAction::Check, false, &at(100), &_lock)
            .unwrap();
        assert_eq!(early.recovery.as_deref(), Some("check_not_due"));
        let due = runtime
            .run_locked(
                updates::UpdateAction::Check,
                false,
                &at(100 + updates::CHECK_INTERVAL),
                &_lock,
            )
            .unwrap();
        assert_eq!(
            due.recovery.as_deref(),
            Some("source_local_rebuild_required")
        );
        drop(_lock);
        let restored = fixture_runtime(fixture.0.clone(), Edition::Public);
        let _lock = restored.lock().unwrap();
        let state = restored.load(&at(101 + updates::CHECK_INTERVAL)).unwrap();
        assert_eq!(
            state.schedule.next_check_at,
            100 + 2 * updates::CHECK_INTERVAL
        );
        assert_eq!(
            view(&state).next_check_at,
            Some(state.schedule.next_check_at)
        );
    }
    #[test]
    fn pause_resume_snapshot_survives_restart_for_both_editions() {
        for edition in [Edition::Personal, Edition::Public] {
            let fixture = Fixture::new();
            let runtime = fixture_runtime(fixture.0.clone(), edition);
            assert_eq!(runtime.snapshot().paused, Some(false));
            assert!(runtime.act(UpdateAction::Pause).completed);
            let restarted = fixture_runtime(fixture.0.clone(), edition);
            let paused = restarted.snapshot();
            assert_eq!(paused.paused, Some(true));
            assert_eq!(paused.state, "paused");
            assert!(restarted.act(UpdateAction::Resume).completed);
            let resumed = fixture_runtime(fixture.0.clone(), edition).snapshot();
            assert_eq!(resumed.paused, Some(false));
            assert_ne!(resumed.state, "paused");
        }
    }
    #[test]
    fn clock_recovery_status_preserves_actual_pause_setting_and_resume() {
        use switcher_core::time::ClockStatus;
        for edition in [Edition::Personal, Edition::Public] {
            for clock_status in [
                ClockStatus::NeedsRecovery {
                    difference_secs: -600,
                },
                ClockStatus::AwaitingVerifiedRefresh,
            ] {
                for paused in [false, true] {
                    let fixture = Fixture::new();
                    let runtime = fixture_runtime(fixture.0.clone(), edition);
                    {
                        let _lock = runtime.lock().unwrap();
                        let mut state = Persistent::new(edition, 100);
                        state.schedule.paused = paused;
                        state.clock.status = clock_status.clone();
                        runtime.save(&state).unwrap();
                    }
                    let snapshot = runtime.snapshot();
                    assert_eq!(snapshot.state, "clock_recovery_required");
                    assert_eq!(snapshot.paused, Some(paused));
                    assert!(!snapshot.can_install);
                    assert!(runtime.act(UpdateAction::Resume).completed);
                    let restarted = fixture_runtime(fixture.0.clone(), edition);
                    let resumed = restarted.snapshot();
                    assert_eq!(resumed.paused, Some(false));
                    assert_eq!(resumed.state, "clock_recovery_required");
                    assert!(!resumed.can_install);
                    let _lock = restarted.lock().unwrap();
                    let state = restarted.load(&fixture_clock().unwrap()).unwrap();
                    assert_eq!(state.clock.status, ClockStatus::AwaitingVerifiedRefresh);
                }
            }
        }
    }
    #[test]
    fn unreadable_schedule_never_reports_a_known_pause_setting() {
        let fixture = Fixture::new();
        let runtime = fixture_runtime(fixture.0.clone(), Edition::Public);
        let _lock = runtime.lock().unwrap();
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.state, "lifecycle_busy");
        assert_eq!(snapshot.paused, None);
    }
    #[test]
    fn corrupted_state_never_resets_the_replay_watermark() {
        let fixture = Fixture::new();
        let runtime = fixture_runtime(fixture.0.clone(), Edition::Public);
        let _lock = runtime.lock().unwrap();
        let state = Persistent::new(Edition::Public, 100);
        runtime.save(&state).unwrap();
        storage::atomic_write(
            &runtime.state_path(),
            b"{\"schema_version\":1,\"schema_version\":1}",
        )
        .unwrap();
        assert!(runtime.load(&at(100)).is_err());
        assert_eq!(
            std::fs::read(runtime.state_path()).unwrap(),
            b"{\"schema_version\":1,\"schema_version\":1}"
        );
    }
    #[test]
    fn shared_clock_baseline_overrides_legacy_embedded_clock_without_resetting_sequence() {
        let fixture = Fixture::new();
        let runtime = fixture_runtime(fixture.0.clone(), Edition::Public);
        let _lock = runtime.lock().unwrap();
        let mut state = Persistent::new(Edition::Public, 100);
        state.updates.sequence.remember(7, "a".repeat(64)).unwrap();
        state.clock.record_verified_success(at(100)).unwrap();
        runtime.save(&state).unwrap();
        let wire = std::fs::read(runtime.state_path()).unwrap();
        assert!(
            serde_json::from_slice::<serde_json::Value>(&wire)
                .unwrap()
                .get("clock")
                .is_none()
        );
        let mut shared = state.clock.clone();
        shared.establish_baseline(at(101));
        storage::atomic_json(&runtime.directory().join("clock.json"), &shared).unwrap();
        let restored = runtime.load(&at(102)).unwrap();
        assert_eq!(
            restored.clock.status,
            switcher_core::time::ClockStatus::AwaitingVerifiedRefresh
        );
        assert_eq!(restored.updates.sequence.highest, 7);
        assert!(!restored.clock.trusted());
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "update_runtime/contention_tests.rs"]
mod contention_tests;
