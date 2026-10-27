//! Authenticated update candidates, bounded pinned transport, staging and local scheduling.
//! Installation requires the separate platform coordinator and sealed Apple assessments.
use crate::signed::{self, Sequence};
use ed25519_dalek::VerifyingKey;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, path::Path};
use switcher_domain::Edition;
use url::Url;

pub const METADATA_LIMIT: usize = 256 * 1024;
pub const STATE_LIMIT: usize = 2 * 1024 * 1024;
pub const CHECK_INTERVAL: u64 = 12 * 60 * 60;
pub const MAX_ARTIFACT_SIZE: u64 = 1024 * 1024 * 1024;
const MAX_FAILURES: usize = 64;

/// Installation preparation and recovery share the region/account lifecycle lock.
/// An unknown or interrupted repair must be resolved before update mutations.
pub fn ensure_installation_idle(
    v2: &Path,
    lock: &crate::storage::LifecycleLock,
) -> Result<(), &'static str> {
    if !lock.protects(v2) {
        return Err("lifecycle_lock_required");
    }
    if crate::regions::recovery_pending_v2(v2)? {
        return Err("repair_recovery_pending");
    }
    if crate::accounts::account_recovery_pending_v2(v2)
        .map_err(|_| "account_recovery_state_invalid")?
    {
        return Err("account_recovery_pending");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    App,
    Helper,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub url: String,
    pub size: u64,
    pub sha256: String,
    pub version: Version,
    pub bundle_id: String,
    pub team_id: String,
    pub compatible_with: VersionReq,
    /// Raw Minisign text, including its authenticated version comment.
    pub minisign_signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metadata {
    pub schema_version: u32,
    pub edition: Edition,
    pub channel: String,
    pub platform: String,
    pub architecture: String,
    pub version: Version,
    pub sequence: u64,
    pub issued_at: u64,
    pub expires_at: u64,
    pub key_id: String,
    pub release_id: String,
    pub app: Artifact,
    pub helper: Option<Artifact>,
}

/// Constructed from the selected local edition, never from a server response.
pub struct PinnedContext<'a> {
    pub edition: Edition,
    pub channel: &'a str,
    pub architecture: &'a str,
    pub key_id: &'a str,
    pub app_bundle_id: &'a str,
    pub helper_bundle_id: &'a str,
    pub team_id: &'a str,
    pub artifact_origin: &'a str,
    pub artifact_prefix: &'a str,
    pub installed_app: &'a Version,
    pub installed_helper: Option<&'a Version>,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    metadata: Metadata,
    payload_hash: String,
    wire: Vec<u8>,
    install_order: InstallOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallOrder {
    AppOnly,
    HelperThenApp,
    AppThenHelper,
}

impl Candidate {
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
    pub fn payload_hash(&self) -> &str {
        &self.payload_hash
    }
    pub fn install_order(&self) -> InstallOrder {
        self.install_order
    }
    pub fn artifact(&self, kind: ArtifactKind) -> Result<&Artifact, &'static str> {
        match kind {
            ArtifactKind::App => Ok(&self.metadata.app),
            ArtifactKind::Helper => self.metadata.helper.as_ref().ok_or("no_helper_artifact"),
        }
    }
    pub fn combination(&self) -> String {
        format!(
            "{}:{}",
            self.metadata.app.sha256,
            self.metadata
                .helper
                .as_ref()
                .map(|a| a.sha256.as_str())
                .unwrap_or("none")
        )
    }
}

pub fn authenticate_candidate(
    wire: &[u8],
    key: &VerifyingKey,
    context: &PinnedContext<'_>,
    sequence: &Sequence,
    now: u64,
    time_trusted: bool,
) -> Result<Candidate, &'static str> {
    if !time_trusted {
        return Err("clock_recovery_required");
    }
    let (metadata, payload_hash): (Metadata, _) = signed::verify(wire, key, METADATA_LIMIT)?;
    if metadata.schema_version != 1 {
        return Err("schema_version");
    }
    if metadata.edition != context.edition
        || metadata.channel != context.channel
        || metadata.platform != "macos"
        || metadata.architecture != context.architecture
        || metadata.key_id != context.key_id
    {
        return Err("release_context");
    }
    if !valid_token(&metadata.key_id)
        || !valid_token(&metadata.release_id)
        || !valid_token(&metadata.channel)
        || !matches!(metadata.architecture.as_str(), "aarch64" | "x86_64")
    {
        return Err("release_identifier");
    }
    if metadata.issued_at > now
        || metadata.expires_at <= now
        || metadata.expires_at <= metadata.issued_at
    {
        return Err("metadata_expired");
    }
    if metadata.version.cmp_precedence(context.installed_app) != Ordering::Greater {
        return Err("downgrade_or_same_version");
    }
    if metadata.app.version != metadata.version {
        return Err("app_version_mismatch");
    }
    validate_artifact(&metadata.app, ArtifactKind::App, context)?;
    let final_helper = match &metadata.helper {
        Some(helper) => {
            validate_artifact(helper, ArtifactKind::Helper, context)?;
            if context
                .installed_helper
                .is_some_and(|v| helper.version.cmp_precedence(v) == Ordering::Less)
            {
                return Err("helper_downgrade");
            }
            if !helper.compatible_with.matches(&metadata.version) {
                return Err("helper_app_incompatible");
            }
            &helper.version
        }
        None => context.installed_helper.ok_or("helper_missing")?,
    };
    if !metadata.app.compatible_with.matches(final_helper) {
        return Err("app_helper_incompatible");
    }
    // Both the final pair and the intermediate pair must be compatible.
    let install_order = match &metadata.helper {
        None => InstallOrder::AppOnly,
        Some(helper) if helper.compatible_with.matches(context.installed_app) => {
            InstallOrder::HelperThenApp
        }
        Some(_)
            if context
                .installed_helper
                .is_some_and(|v| metadata.app.compatible_with.matches(v)) =>
        {
            InstallOrder::AppThenHelper
        }
        Some(_) => return Err("compatible_bridge_required"),
    };
    sequence.check(metadata.sequence, &payload_hash)?;
    Ok(Candidate {
        metadata,
        payload_hash,
        wire: wire.to_vec(),
        install_order,
    })
}

fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn validate_artifact(
    artifact: &Artifact,
    kind: ArtifactKind,
    context: &PinnedContext<'_>,
) -> Result<(), &'static str> {
    if artifact.size == 0 || artifact.size > MAX_ARTIFACT_SIZE || !is_hash(&artifact.sha256) {
        return Err("artifact_bounds");
    }
    let bundle = match kind {
        ArtifactKind::App => context.app_bundle_id,
        ArtifactKind::Helper => context.helper_bundle_id,
    };
    if artifact.bundle_id != bundle || artifact.team_id != context.team_id {
        return Err("artifact_identity");
    }
    if !valid_bundle(&artifact.bundle_id) || !valid_team(&artifact.team_id) {
        return Err("artifact_identity");
    }
    let origin = Url::parse(context.artifact_origin).map_err(|_| "pinned_origin")?;
    let url = Url::parse(&artifact.url).map_err(|_| "artifact_url")?;
    if origin.scheme() != "https"
        || origin.host_str().is_none()
        || origin.username() != ""
        || origin.password().is_some()
        || origin.query().is_some()
        || origin.fragment().is_some()
        || origin.port_or_known_default() != Some(443)
        || url.scheme() != "https"
        || url.origin() != origin.origin()
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.port_or_known_default() != Some(443)
        || !context.artifact_prefix.starts_with('/')
        || !context.artifact_prefix.ends_with('/')
        || !url.path().starts_with(context.artifact_prefix)
        || url.path().contains('%')
    {
        return Err("artifact_source");
    }
    if artifact.minisign_signature.len() > 4096
        || minisign_verify::Signature::decode(&artifact.minisign_signature).is_err()
    {
        return Err("minisign_shape");
    }
    Ok(())
}

fn valid_team(value: &str) -> bool {
    value.len() == 10
        && value
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}
fn valid_bundle(value: &str) -> bool {
    value.len() <= 255 && value.contains('.') && value.split('.').all(valid_token)
}

/// The fields are private: callers cannot turn booleans into an authenticated artifact.
#[derive(Debug)]
pub struct VerifiedDownload {
    candidate_hash: String,
    artifact_hash: String,
    kind: ArtifactKind,
}

impl VerifiedDownload {
    pub fn sha256(&self) -> &str {
        &self.artifact_hash
    }
    pub fn kind(&self) -> ArtifactKind {
        self.kind
    }
}

pub fn verify_download(
    candidate: &Candidate,
    kind: ArtifactKind,
    bytes: &[u8],
    public_key_text: &str,
) -> Result<VerifiedDownload, &'static str> {
    let artifact = candidate.artifact(kind)?;
    if bytes.len() as u64 != artifact.size {
        return Err("artifact_size");
    }
    if signed::digest(bytes) != artifact.sha256 {
        return Err("artifact_sha256");
    }
    let key = minisign_verify::PublicKey::decode(public_key_text).map_err(|_| "minisign_key")?;
    let signature = minisign_verify::Signature::decode(&artifact.minisign_signature)
        .map_err(|_| "minisign_shape")?;
    key.verify(bytes, &signature, false)
        .map_err(|_| "minisign_signature")?;
    // Only read the trusted comment after the global signature has been checked.
    let versions: Vec<_> = signature
        .trusted_comment()
        .split('\t')
        .filter_map(|field| field.strip_prefix("version:"))
        .collect();
    if versions.len() != 1
        || Version::parse(versions[0].trim_start_matches('v'))
            .ok()
            .as_ref()
            != Some(&artifact.version)
    {
        return Err("minisign_signed_version");
    }
    Ok(VerifiedDownload {
        candidate_hash: candidate.payload_hash.clone(),
        artifact_hash: artifact.sha256.clone(),
        kind,
    })
}

#[derive(Debug)]
pub struct AppleIdentity {
    candidate_hash: String,
    kind: ArtifactKind,
    artifact_hash: String,
    app_cdhash: String,
}

#[cfg(any(target_os = "macos", test))]
fn parse_app_cdhash(output: &[u8]) -> Result<String, &'static str> {
    let output = std::str::from_utf8(output).map_err(|_| "apple_app_cdhash")?;
    let mut hashes = output
        .lines()
        .filter_map(|line| line.strip_prefix("CDHash="));
    let hash = hashes.next().ok_or("apple_app_cdhash")?;
    if hashes.next().is_some() || hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("apple_app_cdhash");
    }
    Ok(hash.to_ascii_lowercase())
}

#[cfg(target_os = "macos")]
fn read_app_cdhash(path: &Path) -> Result<String, &'static str> {
    let output = std::process::Command::new("/usr/bin/codesign")
        .args(["--display", "--verbose=4"])
        .arg(path)
        .env("LC_ALL", "C")
        .output()
        .map_err(|_| "apple_app_cdhash")?;
    if !output.status.success() {
        return Err("apple_app_cdhash");
    }
    parse_app_cdhash(&output.stderr)
}

#[cfg(any(target_os = "macos", test))]
fn app_identity_command(artifact: &Artifact, path: &Path) -> std::process::Command {
    let requirement = format!(
        "=anchor apple generic and certificate leaf[subject.OU] = \"{}\" and identifier \"{}\"",
        artifact.team_id, artifact.bundle_id
    );
    let mut command = std::process::Command::new("/usr/bin/codesign");
    command
        .args(["--verify", "--deep", "--strict", "-R", &requirement])
        .arg(path)
        .env("LC_ALL", "C");
    command
}

/// Actual platform assessment. The returned proof is specific to this candidate and downloaded bytes.
/// `path` is the app extracted into a private staging directory, or the downloaded helper PKG.
/// The installation coordinator must maintain that association and recheck staged file identity.
pub fn verify_apple_identity(
    candidate: &Candidate,
    download: &VerifiedDownload,
    path: &Path,
) -> Result<AppleIdentity, &'static str> {
    if download.candidate_hash != candidate.payload_hash {
        return Err("candidate_binding");
    }
    let artifact = candidate.artifact(download.kind)?;
    if download.artifact_hash != artifact.sha256 {
        return Err("artifact_binding");
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|_| "apple_artifact_missing")?;
    if metadata.file_type().is_symlink() {
        return Err("apple_artifact_symlink");
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = metadata;
        return Err("apple_identity_unavailable");
    }
    #[cfg(target_os = "macos")]
    {
        let run = |program: &str, args: &[&str]| -> Result<std::process::Output, &'static str> {
            let mut command = std::process::Command::new(program);
            command.args(args).arg(path).env("LC_ALL", "C");
            command.output().map_err(|_| "apple_identity_unavailable")
        };
        let mut app_cdhash = String::new();
        match download.kind {
            ArtifactKind::App => {
                if !metadata.is_dir() {
                    return Err("apple_app_shape");
                }
                let before = read_app_cdhash(path)?;
                if !app_identity_command(artifact, path)
                    .output()
                    .map_err(|_| "apple_identity_unavailable")?
                    .status
                    .success()
                {
                    return Err("apple_app_identity");
                }
                if !run("/usr/sbin/spctl", &["--assess", "--type", "execute"])?
                    .status
                    .success()
                {
                    return Err("apple_app_assessment");
                }
                let version = std::process::Command::new("/usr/bin/plutil")
                    .args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-"])
                    .arg(path.join("Contents/Info.plist"))
                    .output()
                    .map_err(|_| "apple_app_version")?;
                if !version.status.success()
                    || Version::parse(String::from_utf8_lossy(&version.stdout).trim())
                        .ok()
                        .as_ref()
                        != Some(&artifact.version)
                {
                    return Err("apple_app_version");
                }
                app_cdhash = read_app_cdhash(path)?;
                if app_cdhash != before {
                    return Err("apple_app_changed_during_assessment");
                }
            }
            ArtifactKind::Helper => {
                if !metadata.is_file() || metadata.len() != artifact.size {
                    return Err("apple_pkg_shape");
                }
                // Re-read the actual PKG so a proof for different downloaded bytes cannot be reused.
                let bytes = std::fs::read(path).map_err(|_| "apple_pkg_read")?;
                if signed::digest(&bytes) != artifact.sha256 {
                    return Err("apple_pkg_binding");
                }
                let result = run("/usr/sbin/pkgutil", &["--check-signature"])?;
                let output = String::from_utf8(result.stdout).map_err(|_| "apple_pkg_identity")?;
                let suffix = format!("({})", artifact.team_id);
                let signer_matches = output.lines().any(|line| {
                    let line = line.trim();
                    line.starts_with("1. Developer ID Installer:") && line.ends_with(&suffix)
                });
                if !result.status.success() || !signer_matches {
                    return Err("apple_pkg_identity");
                }
                if !run("/usr/sbin/spctl", &["--assess", "--type", "install"])?
                    .status
                    .success()
                {
                    return Err("apple_pkg_assessment");
                }
                // Receipt identifier/version are checked by the fixed installer before committing.
            }
        }
        Ok(AppleIdentity {
            candidate_hash: candidate.payload_hash.clone(),
            kind: download.kind,
            artifact_hash: download.artifact_hash.clone(),
            app_cdhash,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallStage {
    Authenticated,
    Downloaded,
    InstallingHelper,
    InstallingApp,
    AwaitingHealth,
    Healthy,
    RecoveryRequired,
    RolledBack,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingCandidate {
    wire: Vec<u8>,
    pub payload_hash: String,
    pub stage: InstallStage,
    install_order: InstallOrder,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FailedArtifact {
    pub combination: String,
    pub reason: String,
    pub recorded_at: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateState {
    pub sequence: Sequence,
    pub candidate: Option<PendingCandidate>,
    pub failures: Vec<FailedArtifact>,
}

impl UpdateState {
    /// Caller persists the state even when this returns false: authenticated high-water marks remain.
    pub fn accept(
        &mut self,
        candidate: &Candidate,
        explicit_retry: bool,
    ) -> Result<bool, &'static str> {
        self.sequence
            .remember(candidate.metadata.sequence, candidate.payload_hash.clone())?;
        let failed = self
            .failures
            .iter()
            .any(|item| item.combination == candidate.combination());
        if let Some(pending) = &self.candidate
            && matches!(
                pending.stage,
                InstallStage::InstallingHelper
                    | InstallStage::InstallingApp
                    | InstallStage::AwaitingHealth
                    | InstallStage::RecoveryRequired
            )
        {
            return Err("install_recovery_required");
        }
        if failed && !explicit_retry {
            // Keep the newest authenticated candidate available for an explicit retry.
            // Retaining an older wire here would make resume fail its sequence check.
            self.candidate = Some(PendingCandidate {
                wire: candidate.wire.clone(),
                payload_hash: candidate.payload_hash.clone(),
                stage: InstallStage::Rejected,
                install_order: candidate.install_order,
            });
            return Ok(false);
        }
        if let Some(pending) = &self.candidate
            && pending.payload_hash == candidate.payload_hash
            && !matches!(
                pending.stage,
                InstallStage::RolledBack | InstallStage::Rejected
            )
        {
            return Ok(true);
        }
        self.candidate = Some(PendingCandidate {
            wire: candidate.wire.clone(),
            payload_hash: candidate.payload_hash.clone(),
            stage: InstallStage::Authenticated,
            install_order: candidate.install_order,
        });
        Ok(true)
    }

    /// Restored candidates are authenticated again. A JSON cache cannot mint a Candidate.
    pub fn resume(
        &self,
        key: &VerifyingKey,
        context: &PinnedContext<'_>,
        now: u64,
        time_trusted: bool,
    ) -> Result<Candidate, &'static str> {
        let pending = self.candidate.as_ref().ok_or("no_candidate")?;
        let candidate = authenticate_candidate(
            &pending.wire,
            key,
            context,
            &self.sequence,
            now,
            time_trusted,
        )?;
        if candidate.payload_hash != pending.payload_hash
            || candidate.install_order != pending.install_order
        {
            return Err("candidate_binding");
        }
        Ok(candidate)
    }

    pub fn mark_downloaded(
        &mut self,
        candidate: &Candidate,
        app: &VerifiedDownload,
        helper: Option<&VerifiedDownload>,
    ) -> Result<(), &'static str> {
        self.check_binding(candidate)?;
        check_proof(
            candidate,
            ArtifactKind::App,
            &app.candidate_hash,
            &app.artifact_hash,
            app.kind,
        )?;
        match (candidate.metadata.helper.as_ref(), helper) {
            (Some(_), Some(proof)) => check_proof(
                candidate,
                ArtifactKind::Helper,
                &proof.candidate_hash,
                &proof.artifact_hash,
                proof.kind,
            )?,
            (Some(_), None) | (None, Some(_)) => return Err("helper_proof"),
            (None, None) => (),
        }
        if self.candidate.as_ref().unwrap().stage == InstallStage::Downloaded {
            return Ok(());
        }
        if self.candidate.as_ref().unwrap().stage != InstallStage::Authenticated {
            return Err("install_stage");
        }
        self.candidate.as_mut().unwrap().stage = InstallStage::Downloaded;
        Ok(())
    }

    /// The caller holds the lifecycle lock and supplies fresh local process/compatibility observations.
    pub fn begin_install(
        &mut self,
        candidate: &Candidate,
        app: &AppleIdentity,
        helper: Option<&AppleIdentity>,
        readiness: InstallReadiness,
    ) -> Result<InstallStage, &'static str> {
        self.check_binding(candidate)?;
        if !readiness.clock_trusted {
            return Err("clock_recovery_required");
        }
        if !readiness.claude_exited || !readiness.account_idle {
            return Err("lifecycle_busy");
        }
        if !readiness.components_compatible {
            return Err("components_incompatible");
        }
        if readiness.now >= candidate.metadata.expires_at
            || readiness.now < candidate.metadata.issued_at
        {
            return Err("metadata_expired");
        }
        check_proof(
            candidate,
            ArtifactKind::App,
            &app.candidate_hash,
            &app.artifact_hash,
            app.kind,
        )?;
        match (candidate.metadata.helper.as_ref(), helper) {
            (Some(_), Some(proof)) => check_proof(
                candidate,
                ArtifactKind::Helper,
                &proof.candidate_hash,
                &proof.artifact_hash,
                proof.kind,
            )?,
            (Some(_), None) | (None, Some(_)) => return Err("helper_apple_identity"),
            (None, None) => (),
        }
        let pending = self.candidate.as_mut().unwrap();
        if pending.stage != InstallStage::Downloaded {
            return Err("install_stage");
        }
        pending.stage = if candidate.install_order == InstallOrder::HelperThenApp {
            InstallStage::InstallingHelper
        } else {
            InstallStage::InstallingApp
        };
        Ok(pending.stage)
    }

    pub fn advance_install(
        &mut self,
        expected: InstallStage,
        next: InstallStage,
    ) -> Result<(), &'static str> {
        let pending = self.candidate.as_mut().ok_or("no_candidate")?;
        let ordered_step = match pending.install_order {
            InstallOrder::AppOnly => {
                (expected, next) == (InstallStage::InstallingApp, InstallStage::AwaitingHealth)
            }
            InstallOrder::HelperThenApp => matches!(
                (expected, next),
                (InstallStage::InstallingHelper, InstallStage::InstallingApp)
                    | (InstallStage::InstallingApp, InstallStage::AwaitingHealth)
            ),
            InstallOrder::AppThenHelper => matches!(
                (expected, next),
                (InstallStage::InstallingApp, InstallStage::InstallingHelper)
                    | (InstallStage::InstallingHelper, InstallStage::AwaitingHealth)
            ),
        };
        if !ordered_step
            && !matches!(
                (expected, next),
                (InstallStage::AwaitingHealth, InstallStage::Healthy)
                    | (InstallStage::RecoveryRequired, InstallStage::RolledBack)
            )
        {
            return Err("invalid_install_transition");
        }
        if pending.stage != expected {
            return Err("install_stage");
        }
        pending.stage = next;
        Ok(())
    }

    /// No automatic assumption about which filesystem version won an interrupted install.
    pub fn recover_interrupted(&mut self) -> bool {
        if let Some(pending) = &mut self.candidate
            && matches!(
                pending.stage,
                InstallStage::InstallingHelper
                    | InstallStage::InstallingApp
                    | InstallStage::AwaitingHealth
            )
        {
            pending.stage = InstallStage::RecoveryRequired;
            return true;
        }
        false
    }

    pub fn record_failure(
        &mut self,
        candidate: &Candidate,
        reason: &str,
        now: u64,
    ) -> Result<(), &'static str> {
        self.check_binding(candidate)?;
        if reason.is_empty()
            || reason.len() > 128
            || !reason
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err("failure_reason");
        }
        let combination = candidate.combination();
        self.failures.retain(|item| item.combination != combination);
        self.failures.push(FailedArtifact {
            combination,
            reason: reason.into(),
            recorded_at: now,
        });
        if self.failures.len() > MAX_FAILURES {
            self.failures.remove(0);
        }
        let pending = self.candidate.as_mut().unwrap();
        pending.stage = if matches!(
            pending.stage,
            InstallStage::InstallingHelper
                | InstallStage::InstallingApp
                | InstallStage::AwaitingHealth
                | InstallStage::RecoveryRequired
        ) {
            InstallStage::RecoveryRequired
        } else {
            InstallStage::Rejected
        };
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, &'static str> {
        let bytes = serde_json::to_vec(self).map_err(|_| "update_state")?;
        if bytes.len() > STATE_LIMIT {
            return Err("update_state_too_large");
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        let state: Self = signed::strict_json(bytes, STATE_LIMIT)?;
        if state.failures.len() > MAX_FAILURES
            || state.failures.iter().any(|f| {
                !valid_combination(&f.combination)
                    || f.reason.is_empty()
                    || f.reason.len() > 128
                    || !f
                        .reason
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            })
            || (state.sequence.highest == 0) != state.sequence.payload_hash.is_none()
            || state
                .sequence
                .payload_hash
                .as_ref()
                .is_some_and(|hash| !is_hash(hash))
            || state
                .candidate
                .as_ref()
                .is_some_and(|c| c.wire.len() > METADATA_LIMIT || !is_hash(&c.payload_hash))
        {
            return Err("update_state_bounds");
        }
        Ok(state)
    }

    fn check_binding(&self, candidate: &Candidate) -> Result<(), &'static str> {
        if self.candidate.as_ref().map(|p| p.payload_hash.as_str())
            != Some(candidate.payload_hash.as_str())
        {
            return Err("candidate_binding");
        }
        self.sequence
            .check(candidate.metadata.sequence, &candidate.payload_hash)
    }
}

fn valid_combination(value: &str) -> bool {
    value
        .split_once(':')
        .is_some_and(|(app, helper)| is_hash(app) && (helper == "none" || is_hash(helper)))
}

fn check_proof(
    candidate: &Candidate,
    expected_kind: ArtifactKind,
    candidate_hash: &str,
    artifact_hash: &str,
    actual_kind: ArtifactKind,
) -> Result<(), &'static str> {
    if actual_kind != expected_kind
        || candidate_hash != candidate.payload_hash
        || artifact_hash != candidate.artifact(expected_kind)?.sha256
    {
        return Err("artifact_binding");
    }
    Ok(())
}

pub struct InstallReadiness {
    pub claude_exited: bool,
    pub account_idle: bool,
    pub components_compatible: bool,
    pub clock_trusted: bool,
    pub now: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateAction {
    Check,
    Download,
    Install,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateSchedule {
    pub edition: Edition,
    pub paused: bool,
    pub deferred_until: Option<u64>,
    pub next_check_at: u64,
    pub failures: u8,
    in_flight: Option<UpdateAction>,
}

impl UpdateSchedule {
    pub fn new(edition: Edition, now: u64) -> Self {
        Self {
            edition,
            paused: false,
            deferred_until: None,
            next_check_at: now.saturating_add(CHECK_INTERVAL),
            failures: 0,
            in_flight: None,
        }
    }
    pub fn automatic_check_due(&self, now: u64) -> bool {
        self.edition == Edition::Public
            && !self.paused
            && self.in_flight.is_none()
            && now >= self.next_check_at
    }
    pub fn begin(
        &mut self,
        action: UpdateAction,
        user_requested: bool,
        now: u64,
    ) -> Result<bool, &'static str> {
        if self.in_flight.is_some() {
            return Ok(false);
        }
        if !user_requested {
            if self.edition == Edition::Personal {
                return Err("personal_requires_user_action");
            }
            if self.paused {
                return Err("updates_paused");
            }
            if action == UpdateAction::Check && now < self.next_check_at {
                return Err("check_not_due");
            }
            if action == UpdateAction::Install
                && self.deferred_until.is_some_and(|until| now < until)
            {
                return Err("installation_deferred");
            }
        }
        self.in_flight = Some(action);
        Ok(true)
    }
    pub fn success(&mut self, action: UpdateAction, now: u64) -> Result<(), &'static str> {
        if self.in_flight != Some(action) {
            return Err("update_action");
        }
        self.in_flight = None;
        self.failures = 0;
        if action == UpdateAction::Check {
            self.next_check_at = now.saturating_add(CHECK_INTERVAL);
        }
        Ok(())
    }
    pub fn transient_failure(&mut self, now: u64) {
        self.in_flight = None;
        let backoff = [60, 300, 900, 1800, 3600];
        self.next_check_at =
            now.saturating_add(backoff[usize::from(self.failures).min(backoff.len() - 1)]);
        self.failures = self.failures.saturating_add(1);
    }
    pub fn authorization_cancelled(&mut self) {
        self.in_flight = None;
    }
    /// Called after acquiring the OS lifecycle lock on process restart.
    pub fn recover_process(&mut self) {
        self.in_flight = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct IdleFixture(std::path::PathBuf);
    impl IdleFixture {
        fn new() -> Self {
            use std::os::unix::fs::DirBuilderExt;
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "update-idle-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .unwrap();
            Self(path)
        }
    }
    impl Drop for IdleFixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn caller_gate_rejects_unsafe_user_repair_parent_and_pending_journal() {
        use std::os::unix::fs::symlink;
        let f = IdleFixture::new();
        let lock = crate::storage::LifecycleLock::try_acquire(&f.0).unwrap();
        assert_eq!(ensure_installation_idle(&f.0, &lock), Ok(()));
        let target = f.0.join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("sentinel"), b"unchanged").unwrap();
        let parent = f.0.join("user-repairs");
        for destination in [f.0.join("missing"), target.clone()] {
            symlink(&destination, &parent).unwrap();
            assert!(ensure_installation_idle(&f.0, &lock).is_err());
            assert_eq!(std::fs::read_link(&parent).unwrap(), destination);
            assert_eq!(
                std::fs::read(target.join("sentinel")).unwrap(),
                b"unchanged"
            );
            std::fs::remove_file(&parent).unwrap();
        }
        std::fs::create_dir(&parent).unwrap();
        std::fs::write(parent.join("journal.json"), b"pending").unwrap();
        assert_eq!(
            ensure_installation_idle(&f.0, &lock),
            Err("repair_recovery_pending")
        );
        assert_eq!(
            std::fs::read(parent.join("journal.json")).unwrap(),
            b"pending"
        );
    }
    #[test]
    fn caller_gate_blocks_account_journals_and_links() {
        use std::os::unix::fs::symlink;
        let f = IdleFixture::new();
        let lock = crate::storage::LifecycleLock::try_acquire(&f.0).unwrap();
        std::fs::create_dir(f.0.join("migration")).unwrap();
        assert_eq!(ensure_installation_idle(&f.0, &lock), Ok(()));
        for name in [
            "account-journal.json",
            "legacy-account-journal.json",
            "migration/in-place-journal.json",
        ] {
            let journal = f.0.join(name);
            std::fs::write(&journal, b"pending").unwrap();
            assert_eq!(
                ensure_installation_idle(&f.0, &lock),
                Err("account_recovery_pending")
            );
            assert_eq!(std::fs::read(&journal).unwrap(), b"pending");
            std::fs::remove_file(&journal).unwrap();
            let missing = f.0.join("missing");
            symlink(&missing, &journal).unwrap();
            assert_eq!(
                ensure_installation_idle(&f.0, &lock),
                Err("account_recovery_pending")
            );
            assert_eq!(std::fs::read_link(&journal).unwrap(), missing);
            std::fs::remove_file(&journal).unwrap();
            assert_eq!(ensure_installation_idle(&f.0, &lock), Ok(()));
        }
    }
    #[test]
    fn caller_gate_rejects_unsafe_migration_parent() {
        use std::os::unix::fs::symlink;
        let f = IdleFixture::new();
        let lock = crate::storage::LifecycleLock::try_acquire(&f.0).unwrap();
        let target = f.0.join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("sentinel"), b"unchanged").unwrap();
        let parent = f.0.join("migration");
        for destination in [f.0.join("missing"), target.clone()] {
            symlink(&destination, &parent).unwrap();
            assert_eq!(
                ensure_installation_idle(&f.0, &lock),
                Err("account_recovery_state_invalid")
            );
            assert_eq!(std::fs::read_link(&parent).unwrap(), destination);
            assert_eq!(
                std::fs::read(target.join("sentinel")).unwrap(),
                b"unchanged"
            );
            std::fs::remove_file(&parent).unwrap();
        }
    }
    #[test]
    fn e11_consent_personal_schedule_never_reuses_completed_action_after_roundtrip() {
        for completed in [
            UpdateAction::Check,
            UpdateAction::Download,
            UpdateAction::Install,
        ] {
            let mut schedule = UpdateSchedule::new(Edition::Personal, 100);
            assert_eq!(schedule.begin(completed, true, 101), Ok(true));
            schedule.success(completed, 102).unwrap();
            let bytes = serde_json::to_vec(&schedule).unwrap();
            let mut restarted: UpdateSchedule = serde_json::from_slice(&bytes).unwrap();
            restarted.recover_process();
            for next in [
                UpdateAction::Check,
                UpdateAction::Download,
                UpdateAction::Install,
            ] {
                assert_eq!(
                    restarted.begin(next, false, 103),
                    Err("personal_requires_user_action")
                );
                assert_eq!(restarted.begin(next, true, 103), Ok(true));
                restarted.success(next, 104).unwrap();
                assert_eq!(
                    restarted.begin(next, false, 105),
                    Err("personal_requires_user_action")
                );
            }
        }
    }

    use ed25519_dalek::{Signer, SigningKey};
    #[test]
    fn codesign_app_identity_passes_inline_requirement_in_production_argv() {
        let metadata = metadata();
        let path = Path::new("/private/tmp/staging with spaces/更新.app");
        let command = app_identity_command(&metadata.app, path);
        assert_eq!(command.get_program(), "/usr/bin/codesign");
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(args.len(), 6);
        assert_eq!(&args[..4], ["--verify", "--deep", "--strict", "-R"]);
        assert_eq!(
            args[4],
            "=anchor apple generic and certificate leaf[subject.OU] = \"TESTTEAM01\" and identifier \"com.example.switcher\""
        );
        assert_eq!(args[5], path);
        assert!(
            command
                .get_envs()
                .any(|(key, value)| key == "LC_ALL" && value == Some(std::ffi::OsStr::new("C")))
        );
    }
    #[test]
    fn app_cdhash_requires_one_exact_fingerprint() {
        let hash = "ABCDEF0123456789ABCDEF0123456789ABCDEF01";
        assert_eq!(
            parse_app_cdhash(
                format!("Executable=/fixture\nCDHash={hash}\nSignature=adhoc\n").as_bytes()
            ),
            Ok(hash.to_ascii_lowercase())
        );
        for invalid in [
            String::new(),
            "CDHash=abc".into(),
            format!("CDHash={}g", &hash[..39]),
            format!("CDHash={hash}\nCDHash={hash}\n"),
            format!("CDHash={hash} "),
        ] {
            assert_eq!(
                parse_app_cdhash(invalid.as_bytes()),
                Err("apple_app_cdhash")
            );
        }
        assert!(parse_app_cdhash(&[0xff]).is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn app_cdhash_binds_resigned_bundle_resources() {
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "switcher-cdhash-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        )));
        let app = fixture.0.join("Fixture.app");
        std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        std::fs::create_dir_all(app.join("Contents/Resources")).unwrap();
        std::fs::copy("/usr/bin/true", app.join("Contents/MacOS/Fixture")).unwrap();
        std::fs::write(app.join("Contents/Info.plist"), br#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleExecutable</key><string>Fixture</string><key>CFBundleIdentifier</key><string>com.example.cdhash-fixture</string><key>CFBundleShortVersionString</key><string>1.0.0</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>"#).unwrap();
        let sign = || {
            let output = std::process::Command::new("/usr/bin/codesign")
                .args(["--force", "--sign", "-"])
                .arg(&app)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        std::fs::write(app.join("Contents/Resources/release"), "first").unwrap();
        sign();
        let sealed = read_app_cdhash(&app).unwrap();
        std::fs::write(app.join("Contents/Resources/release"), "replacement").unwrap();
        let changed = std::process::Command::new("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(&app)
            .output()
            .unwrap();
        assert!(
            !changed.status.success(),
            "modified resources must fail the strict seal"
        );
        sign();
        assert_ne!(
            read_app_cdhash(&app).unwrap(),
            sealed,
            "resigning the same bundle/version with different resources must change CDHash"
        );
    }
    // Upstream Tauri updater offline fixture, synthetic test key and bytes only.
    pub(super) const MINI_KEY: &str = "untrusted comment: minisign public key: A49EF4746FA6E6D7\nRWTX5qZvdPSepDGWg2bz8n6ArQnBMwgQK1geVosorcbLzcunD+KJ+cYe";
    const MINI_SIG: &str = "untrusted comment: signature from tauri secret key\nRUTX5qZvdPSepPF2zsjvP/L/TQtBLRnWRvaq+4omK1UEugtQp/USrGiI1v762sDy3id3aor7bRC9ETyeot6lMHHrWoZ5hjUM1Qc=\ntrusted comment: timestamp:1791649051\tfile:payload\tversion:2.0.0\nDDq2CLOEl5cQmC2eJRHlnZPVViOZsu1CjpPKQoei4A/XBfkDqGO0B+QMrcrHgEQ5qV3mne75tdFWhJpqUXh9AA==";
    fn context<'a>(app: &'a Version, helper: &'a Version) -> PinnedContext<'a> {
        PinnedContext {
            edition: Edition::Public,
            channel: "stable",
            architecture: "aarch64",
            key_id: "software-a",
            app_bundle_id: "com.example.switcher",
            helper_bundle_id: "com.example.switcher.helper",
            team_id: "TESTTEAM01",
            artifact_origin: "https://updates.example.com",
            artifact_prefix: "/public/stable/aarch64/",
            installed_app: app,
            installed_helper: Some(helper),
        }
    }
    pub(super) fn metadata() -> Metadata {
        Metadata {
            schema_version: 1,
            edition: Edition::Public,
            channel: "stable".into(),
            platform: "macos".into(),
            architecture: "aarch64".into(),
            version: Version::parse("2.0.0").unwrap(),
            sequence: 7,
            issued_at: 100,
            expires_at: 1000,
            key_id: "software-a".into(),
            release_id: "release-2".into(),
            app: Artifact {
                url: "https://updates.example.com/public/stable/aarch64/app.tar.gz".into(),
                size: 4,
                sha256: signed::digest(b"test"),
                version: Version::parse("2.0.0").unwrap(),
                bundle_id: "com.example.switcher".into(),
                team_id: "TESTTEAM01".into(),
                compatible_with: VersionReq::parse(">=1, <3").unwrap(),
                minisign_signature: MINI_SIG.into(),
            },
            helper: None,
        }
    }
    pub(super) fn sign(metadata: &Metadata, key: &SigningKey) -> Vec<u8> {
        let payload = serde_json::to_string(metadata).unwrap();
        let signature = key.sign(payload.as_bytes()).to_bytes().to_vec();
        serde_json::to_vec(&serde_json::json!({"payload":payload,"signature":signature})).unwrap()
    }
    fn candidate(metadata: &Metadata) -> Candidate {
        let key = SigningKey::from_bytes(&[17; 32]);
        let installed = Version::parse("1.0.0").unwrap();
        authenticate_candidate(
            &sign(metadata, &key),
            &key.verifying_key(),
            &context(&installed, &installed),
            &Sequence::default(),
            200,
            true,
        )
        .unwrap()
    }
    #[test]
    fn metadata_is_bound_to_release_identity_time_version_and_source() {
        let key = SigningKey::from_bytes(&[17; 32]);
        let installed = Version::parse("1.0.0").unwrap();
        let check = |m: &Metadata| {
            authenticate_candidate(
                &sign(m, &key),
                &key.verifying_key(),
                &context(&installed, &installed),
                &Sequence::default(),
                200,
                true,
            )
        };
        assert!(check(&metadata()).is_ok());
        for mutate in [
            |m: &mut Metadata| m.edition = Edition::Personal,
            |m: &mut Metadata| m.architecture = "x86_64".into(),
            |m: &mut Metadata| m.key_id = "ads-a".into(),
            |m: &mut Metadata| m.expires_at = 200,
            |m: &mut Metadata| {
                m.app.url = "https://evil.example/public/stable/aarch64/app.tar.gz".into()
            },
            |m: &mut Metadata| {
                m.app.url = "https://updates.example.com/public/stable/aarch64/%2e%2e/app".into()
            },
            |m: &mut Metadata| m.app.team_id = "OTHERTEAM1".into(),
            |m: &mut Metadata| m.app.compatible_with = VersionReq::parse(">=9").unwrap(),
            |m: &mut Metadata| m.version = Version::parse("1.0.0+repack").unwrap(),
        ] {
            let mut m = metadata();
            mutate(&mut m);
            assert!(check(&m).is_err());
        }
        assert!(
            authenticate_candidate(
                &sign(&metadata(), &key),
                &SigningKey::from_bytes(&[18; 32]).verifying_key(),
                &context(&installed, &installed),
                &Sequence::default(),
                200,
                true
            )
            .is_err()
        );
        assert!(
            authenticate_candidate(
                &sign(&metadata(), &key),
                &key.verifying_key(),
                &context(&installed, &installed),
                &Sequence::default(),
                200,
                false
            )
            .is_err()
        );
    }
    #[test]
    fn minisign_proof_requires_exact_bytes_and_signed_version() {
        let c = candidate(&metadata());
        let proof = verify_download(&c, ArtifactKind::App, b"test", MINI_KEY).unwrap();
        assert_eq!(proof.sha256(), signed::digest(b"test"));
        assert_eq!(
            verify_download(&c, ArtifactKind::App, b"bad!", MINI_KEY).unwrap_err(),
            "artifact_sha256"
        );
        assert_eq!(
            verify_download(&c, ArtifactKind::App, b"tes", MINI_KEY).unwrap_err(),
            "artifact_size"
        );
        let mut m = metadata();
        m.version = Version::parse("2.1.0").unwrap();
        m.app.version = m.version.clone();
        assert_eq!(
            verify_download(&candidate(&m), ArtifactKind::App, b"test", MINI_KEY).unwrap_err(),
            "minisign_signed_version"
        );
        let mut m = metadata();
        m.app.minisign_signature = MINI_SIG.replace("version:2.0.0", "version:2.1.0");
        assert_eq!(
            verify_download(&candidate(&m), ArtifactKind::App, b"test", MINI_KEY).unwrap_err(),
            "minisign_signature"
        );
        assert_eq!(
            verify_apple_identity(&c, &proof, Path::new("/nonexistent/switcher-test.app"))
                .unwrap_err(),
            "apple_artifact_missing"
        );
    }
    #[test]
    fn persistent_failure_suppression_preserves_replay_watermark() {
        let c = candidate(&metadata());
        let mut state = UpdateState::default();
        assert!(state.accept(&c, false).unwrap());
        state.record_failure(&c, "health_failed", 210).unwrap();
        let restored = UpdateState::decode(&state.encode().unwrap()).unwrap();
        let installed = Version::parse("1.0.0").unwrap();
        let key = SigningKey::from_bytes(&[17; 32]);
        assert_eq!(
            restored
                .resume(
                    &key.verifying_key(),
                    &context(&installed, &installed),
                    220,
                    true
                )
                .unwrap()
                .payload_hash(),
            c.payload_hash()
        );
        let mut state = restored;
        let mut newer = metadata();
        newer.sequence += 1;
        let newer = candidate(&newer);
        assert!(!state.accept(&newer, false).unwrap());
        assert_eq!(state.sequence.highest, 8);
        let replayed = state
            .resume(
                &key.verifying_key(),
                &context(&installed, &installed),
                220,
                true,
            )
            .unwrap();
        assert_eq!(replayed.payload_hash(), newer.payload_hash());
        assert_eq!(
            state.candidate.as_ref().unwrap().stage,
            InstallStage::Rejected
        );
        assert!(state.accept(&newer, true).unwrap());
        assert!(state.accept(&c, true).is_err());
        state.candidate.as_mut().unwrap().stage = InstallStage::InstallingApp;
        assert!(state.recover_interrupted());
        assert_eq!(state.accept(&newer, true), Err("install_recovery_required"));
        assert_eq!(
            state.candidate.as_ref().unwrap().stage,
            InstallStage::RecoveryRequired
        );
        state
            .advance_install(InstallStage::RecoveryRequired, InstallStage::RolledBack)
            .unwrap();
        assert_eq!(state.sequence.highest, 8);
        let bytes = state.encode().unwrap();
        let corrupt = String::from_utf8(bytes)
            .unwrap()
            .replace("\"sequence\":", "\"sequence\":{},\"sequence\":");
        assert!(UpdateState::decode(corrupt.as_bytes()).is_err());
    }
    #[test]
    fn download_proofs_cannot_cross_candidate_releases() {
        let c = candidate(&metadata());
        let mut state = UpdateState::default();
        state.accept(&c, false).unwrap();
        let proof = verify_download(&c, ArtifactKind::App, b"test", MINI_KEY).unwrap();
        let mut newer = metadata();
        newer.sequence += 1;
        let newer = candidate(&newer);
        state.accept(&newer, false).unwrap();
        assert_eq!(
            state.mark_downloaded(&newer, &proof, None),
            Err("artifact_binding")
        );
        let proof = verify_download(&newer, ArtifactKind::App, b"test", MINI_KEY).unwrap();
        state.mark_downloaded(&newer, &proof, None).unwrap();
        assert_eq!(
            state.candidate.as_ref().unwrap().stage,
            InstallStage::Downloaded
        );
    }
    #[test]
    fn component_order_requires_a_compatible_intermediate_pair() {
        let mut m = metadata();
        let mut helper = m.app.clone();
        helper.url = "https://updates.example.com/public/stable/aarch64/helper.pkg".into();
        helper.bundle_id = "com.example.switcher.helper".into();
        helper.compatible_with = VersionReq::parse(">=1, <3").unwrap();
        m.helper = Some(helper);
        assert_eq!(candidate(&m).install_order(), InstallOrder::HelperThenApp);
        m.helper.as_mut().unwrap().compatible_with = VersionReq::parse(">=2, <3").unwrap();
        assert_eq!(candidate(&m).install_order(), InstallOrder::AppThenHelper);
        m.app.compatible_with = VersionReq::parse(">=2, <3").unwrap();
        let key = SigningKey::from_bytes(&[17; 32]);
        let installed = Version::parse("1.0.0").unwrap();
        assert_eq!(
            authenticate_candidate(
                &sign(&m, &key),
                &key.verifying_key(),
                &context(&installed, &installed),
                &Sequence::default(),
                200,
                true
            )
            .unwrap_err(),
            "compatible_bridge_required"
        );
    }
    #[test]
    fn personal_requires_each_action_public_schedule_pause_defer_backoff_survive_restart() {
        let mut personal = UpdateSchedule::new(Edition::Personal, 0);
        for action in [
            UpdateAction::Check,
            UpdateAction::Download,
            UpdateAction::Install,
        ] {
            assert!(!personal.automatic_check_due(u64::MAX));
            assert!(personal.begin(action, false, CHECK_INTERVAL).is_err());
            assert!(personal.begin(action, true, 1).unwrap());
            assert!(!personal.begin(action, true, 1).unwrap());
            personal.success(action, 1).unwrap();
        }
        let mut public = UpdateSchedule::new(Edition::Public, 0);
        assert!(!public.automatic_check_due(CHECK_INTERVAL - 1));
        assert!(public.automatic_check_due(CHECK_INTERVAL));
        public.paused = true;
        assert!(
            public
                .begin(UpdateAction::Download, false, CHECK_INTERVAL)
                .is_err()
        );
        public.paused = false;
        public.deferred_until = Some(99999);
        assert_eq!(
            public.begin(UpdateAction::Install, false, CHECK_INTERVAL),
            Err("installation_deferred")
        );
        public
            .begin(UpdateAction::Check, false, CHECK_INTERVAL)
            .unwrap();
        public.transient_failure(CHECK_INTERVAL);
        assert_eq!(public.next_check_at, CHECK_INTERVAL + 60);
        public
            .begin(UpdateAction::Check, true, CHECK_INTERVAL)
            .unwrap();
        public.transient_failure(CHECK_INTERVAL);
        assert_eq!(public.next_check_at, CHECK_INTERVAL + 300);
        public
            .begin(UpdateAction::Install, true, CHECK_INTERVAL)
            .unwrap();
        public.authorization_cancelled();
        assert_eq!(public.failures, 2);
        let mut restored: UpdateSchedule =
            signed::strict_json(&serde_json::to_vec(&public).unwrap(), STATE_LIMIT).unwrap();
        restored.recover_process();
        assert_eq!(restored.next_check_at, public.next_check_at);
        assert_eq!(restored.deferred_until, Some(99999));
    }
    #[test]
    fn bridge_release_is_required_before_new_signing_key() {
        let old = SigningKey::from_bytes(&[17; 32]);
        let new = SigningKey::from_bytes(&[18; 32]);
        let v1 = Version::parse("1.0.0").unwrap();
        let v2 = Version::parse("2.0.0").unwrap();
        let bridge = metadata();
        let first = authenticate_candidate(
            &sign(&bridge, &old),
            &old.verifying_key(),
            &context(&v1, &v1),
            &Sequence::default(),
            200,
            true,
        )
        .unwrap();
        let mut seq = Sequence::default();
        seq.remember(first.metadata.sequence, first.payload_hash.clone())
            .unwrap();
        let mut next = metadata();
        next.sequence = 8;
        next.version = Version::parse("3.0.0").unwrap();
        next.app.version = next.version.clone();
        next.key_id = "software-b".into();
        let wire = sign(&next, &new);
        assert!(
            authenticate_candidate(
                &wire,
                &old.verifying_key(),
                &context(&v1, &v1),
                &seq,
                200,
                true
            )
            .is_err()
        );
        // The installed bridge supplies the next pinned key/context. No key is learned from wire data.
        let mut pinned = context(&v2, &v1);
        pinned.key_id = "software-b";
        assert!(
            authenticate_candidate(&wire, &new.verifying_key(), &pinned, &seq, 200, true).is_ok()
        );
    }
}

/// Build-time release configuration. It has no Deserialize implementation and is
/// never accepted by IPC, CLI arguments, preferences, or unsigned server metadata.
pub struct ReleasePinSpec {
    pub edition: Edition,
    pub metadata_url: &'static str,
    pub metadata_key: [u8; 32],
    pub key_id: &'static str,
    pub minisign_key: &'static str,
    pub channel: &'static str,
    pub architecture: &'static str,
    pub app_bundle_id: &'static str,
    pub helper_bundle_id: &'static str,
    pub team_id: &'static str,
    pub artifact_origin: &'static str,
    pub artifact_prefix: &'static str,
}
#[derive(Clone)]
pub struct ValidatedReleasePins {
    metadata_url: Url,
    metadata_key: VerifyingKey,
    minisign_key: &'static str,
    edition: Edition,
    key_id: &'static str,
    channel: &'static str,
    architecture: &'static str,
    app_bundle_id: &'static str,
    helper_bundle_id: &'static str,
    team_id: &'static str,
    artifact_origin: &'static str,
    artifact_prefix: &'static str,
}
impl ValidatedReleasePins {
    pub fn validate(spec: ReleasePinSpec) -> Result<Self, &'static str> {
        let url = Url::parse(spec.metadata_url).map_err(|_| "metadata_pin_url")?;
        let origin = Url::parse(spec.artifact_origin).map_err(|_| "artifact_pin_origin")?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port_or_known_default() != Some(443)
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path().contains('%')
            || url.origin() != origin.origin()
            || !url.path().starts_with(spec.artifact_prefix)
            || !spec.artifact_prefix.starts_with('/')
            || !spec.artifact_prefix.ends_with('/')
            || !valid_token(spec.key_id)
            || !valid_token(spec.channel)
            || !matches!(spec.architecture, "aarch64" | "x86_64")
            || !valid_bundle(spec.app_bundle_id)
            || !valid_bundle(spec.helper_bundle_id)
            || !valid_team(spec.team_id)
        {
            return Err("release_pins");
        }
        let metadata_key =
            VerifyingKey::from_bytes(&spec.metadata_key).map_err(|_| "metadata_pin_key")?;
        minisign_verify::PublicKey::decode(spec.minisign_key).map_err(|_| "minisign_pin_key")?;
        Ok(Self {
            metadata_url: url,
            metadata_key,
            minisign_key: spec.minisign_key,
            edition: spec.edition,
            key_id: spec.key_id,
            channel: spec.channel,
            architecture: spec.architecture,
            app_bundle_id: spec.app_bundle_id,
            helper_bundle_id: spec.helper_bundle_id,
            team_id: spec.team_id,
            artifact_origin: spec.artifact_origin,
            artifact_prefix: spec.artifact_prefix,
        })
    }
    pub fn edition(&self) -> Edition {
        self.edition
    }
    pub fn minisign_public_key(&self) -> &str {
        self.minisign_key
    }
    pub fn context<'a>(
        &'a self,
        app: &'a Version,
        helper: Option<&'a Version>,
    ) -> PinnedContext<'a> {
        PinnedContext {
            edition: self.edition,
            channel: self.channel,
            architecture: self.architecture,
            key_id: self.key_id,
            app_bundle_id: self.app_bundle_id,
            helper_bundle_id: self.helper_bundle_id,
            team_id: self.team_id,
            artifact_origin: self.artifact_origin,
            artifact_prefix: self.artifact_prefix,
            installed_app: app,
            installed_helper: helper,
        }
    }
    pub fn restore(
        &self,
        state: &UpdateState,
        app: &Version,
        helper: Option<&Version>,
        now: u64,
        clock_allows_refresh: bool,
    ) -> Result<Candidate, &'static str> {
        state.resume(
            &self.metadata_key,
            &self.context(app, helper),
            now,
            clock_allows_refresh,
        )
    }
    pub fn version(value: &str) -> Result<Version, &'static str> {
        Version::parse(value).map_err(|_| "installed_component_version")
    }
}

// Keep production and isolated TLS tests on the same transport policy.
fn release_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(120))
        .user_agent("ClaudeSwitcher/0.2 update")
}

pub struct ReleaseTransport {
    pins: ValidatedReleasePins,
    client: reqwest::Client,
}
impl ReleaseTransport {
    /// Dedicated worker thread avoids nesting a Tokio runtime in an IPC executor.
    pub fn check_blocking(
        &self,
        state: &UpdateState,
        app: &Version,
        helper: Option<&Version>,
        now: u64,
        clock_allows_refresh: bool,
    ) -> Result<Candidate, &'static str> {
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|_| "update_runtime")?;
                    runtime.block_on(self.check(state, app, helper, now, clock_allows_refresh))
                })
                .join()
                .map_err(|_| "update_worker")?
        })
    }
    pub fn download_blocking(
        &self,
        candidate: &Candidate,
        kind: ArtifactKind,
        now: u64,
    ) -> Result<(Vec<u8>, VerifiedDownload), &'static str> {
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|_| "update_runtime")?;
                    runtime.block_on(self.download(candidate, kind, now))
                })
                .join()
                .map_err(|_| "update_worker")?
        })
    }
    pub fn new(pins: ValidatedReleasePins) -> Result<Self, &'static str> {
        let client = release_client_builder()
            .build()
            .map_err(|_| "update_transport")?;
        Ok(Self { pins, client })
    }
    pub async fn check(
        &self,
        state: &UpdateState,
        app: &Version,
        helper: Option<&Version>,
        now: u64,
        clock_allows_refresh: bool,
    ) -> Result<Candidate, &'static str> {
        if !clock_allows_refresh {
            return Err("clock_recovery_required");
        }
        let bytes = self
            .fetch(self.pins.metadata_url.as_str(), METADATA_LIMIT)
            .await?;
        authenticate_candidate(
            &bytes,
            &self.pins.metadata_key,
            &self.pins.context(app, helper),
            &state.sequence,
            now,
            clock_allows_refresh,
        )
    }
    pub async fn download(
        &self,
        candidate: &Candidate,
        kind: ArtifactKind,
        now: u64,
    ) -> Result<(Vec<u8>, VerifiedDownload), &'static str> {
        self.validate_candidate(candidate)?;
        if candidate.metadata.issued_at > now || candidate.metadata.expires_at <= now {
            return Err("metadata_expired");
        }
        let artifact = candidate.artifact(kind)?;
        // Repeat source restrictions against these build's pins before issuing any request.
        let unused_version = Version::new(0, 0, 0);
        validate_artifact(artifact, kind, &self.pins.context(&unused_version, None))?;
        if candidate.metadata.edition != self.pins.edition
            || candidate.metadata.key_id != self.pins.key_id
        {
            return Err("release_context");
        }
        let bytes = self
            .fetch(
                &artifact.url,
                artifact.size.try_into().map_err(|_| "artifact_bounds")?,
            )
            .await?;
        let proof = verify_download(candidate, kind, &bytes, self.pins.minisign_key)?;
        Ok((bytes, proof))
    }
    async fn fetch(&self, url: &str, limit: usize) -> Result<Vec<u8>, &'static str> {
        let mut response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| "update_network")?;
        if response.url().as_str() != url || !response.status().is_success() {
            return Err("update_http_status_or_redirect");
        }
        if response
            .content_length()
            .is_some_and(|size| size > limit as u64)
        {
            return Err("update_response_too_large");
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| "update_network")? {
            if chunk.len() > limit.saturating_sub(bytes.len()) {
                return Err("update_response_too_large");
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
    pub fn verify_cached(
        &self,
        candidate: &Candidate,
        kind: ArtifactKind,
        bytes: &[u8],
    ) -> Result<VerifiedDownload, &'static str> {
        self.validate_candidate(candidate)?;
        verify_download(candidate, kind, bytes, self.pins.minisign_key)
    }
    fn validate_candidate(&self, candidate: &Candidate) -> Result<(), &'static str> {
        let (metadata, hash): (Metadata, _) =
            signed::verify(&candidate.wire, &self.pins.metadata_key, METADATA_LIMIT)?;
        if hash != candidate.payload_hash
            || metadata.edition != self.pins.edition
            || metadata.key_id != self.pins.key_id
            || metadata.channel != self.pins.channel
            || metadata.architecture != self.pins.architecture
            || metadata.platform != "macos"
        {
            return Err("release_context");
        }
        Ok(())
    }
}

/// Sealed Apple assessments for the same authenticated candidate are required
/// before a coordinator receives staged paths. This method performs no install.
pub struct PreparedInstallation {
    candidate: Candidate,
    app_path: std::path::PathBuf,
    helper_path: Option<std::path::PathBuf>,
    app_identity: AppleIdentity,
    helper_identity: Option<AppleIdentity>,
}
impl PreparedInstallation {
    pub fn candidate(&self) -> &Candidate {
        &self.candidate
    }
    pub fn app_path(&self) -> &Path {
        &self.app_path
    }
    /// The assessed CodeDirectory hash. The coordinator must compare this after
    /// its privileged copy and also verify the copied bundle's strict resource seal.
    pub fn app_cdhash(&self) -> &str {
        &self.app_identity.app_cdhash
    }
    pub fn helper_path(&self) -> Option<&Path> {
        self.helper_path.as_deref()
    }
    pub fn begin(
        &self,
        state: &mut UpdateState,
        readiness: InstallReadiness,
    ) -> Result<InstallStage, &'static str> {
        state.begin_install(
            &self.candidate,
            &self.app_identity,
            self.helper_identity.as_ref(),
            readiness,
        )
    }
}
pub fn assess_staged_installation(
    candidate: Candidate,
    app: &VerifiedDownload,
    app_path: &Path,
    helper: Option<(&VerifiedDownload, &Path)>,
) -> Result<PreparedInstallation, &'static str> {
    if app.kind != ArtifactKind::App
        || helper.is_some_and(|(proof, _)| proof.kind != ArtifactKind::Helper)
    {
        return Err("artifact_binding");
    }
    let app_identity = verify_apple_identity(&candidate, app, app_path)?;
    let helper_identity = match (candidate.metadata.helper.as_ref(), helper) {
        (Some(_), Some((proof, path))) => Some(verify_apple_identity(&candidate, proof, path)?),
        (None, None) => None,
        _ => return Err("helper_apple_identity"),
    };
    Ok(PreparedInstallation {
        candidate,
        app_path: app_path.to_owned(),
        helper_path: helper.map(|(_, path)| path.to_owned()),
        app_identity,
        helper_identity,
    })
}
/// A platform coordinator must retain verified compatible backups, persist each
/// step, use Installer authorization for helper changes, and verify final health.
/// Production without that coordinator must report WAIT and never begin_install.
pub trait UpdateInstallCoordinator {
    fn install(
        &mut self,
        prepared: &PreparedInstallation,
        state: &mut UpdateState,
    ) -> Result<(), &'static str>;
}

/// Unpack only authenticated archive bytes into a new private directory. The tar
/// library's unpack_in path checks remain enabled in addition to these bounds.
pub fn stage_app_archive(
    candidate: &Candidate,
    proof: &VerifiedDownload,
    bytes: &[u8],
    staging: &Path,
) -> Result<std::path::PathBuf, &'static str> {
    use std::path::Component;
    check_proof(
        candidate,
        ArtifactKind::App,
        &proof.candidate_hash,
        &proof.artifact_hash,
        proof.kind,
    )?;
    if bytes.len() as u64 != candidate.metadata.app.size
        || signed::digest(bytes) != proof.artifact_hash
    {
        return Err("staging_artifact_binding");
    }
    crate::storage::create_private_directory(staging).map_err(|_| "staging_directory")?;
    if std::fs::read_dir(staging)
        .map_err(|_| "staging_directory")?
        .next()
        .is_some()
    {
        return Err("staging_not_empty");
    }
    let result = (|| {
        let decoder = flate2::read::GzDecoder::new(bytes);
        let mut archive = tar::Archive::new(decoder);
        let mut total = 0u64;
        let mut count = 0u32;
        let mut app_root = None;
        for entry in archive.entries().map_err(|_| "app_archive")? {
            let mut entry = entry.map_err(|_| "app_archive")?;
            count = count.checked_add(1).ok_or("app_archive_bounds")?;
            total = total
                .checked_add(entry.size())
                .ok_or("app_archive_bounds")?;
            if count > 50_000 || total > 2 * MAX_ARTIFACT_SIZE {
                return Err("app_archive_bounds");
            }
            let path = entry.path().map_err(|_| "app_archive_path")?.into_owned();
            let parts: Vec<_> = path.components().collect();
            if parts.is_empty()
                || parts
                    .iter()
                    .any(|part| !matches!(part, Component::Normal(_)))
            {
                return Err("app_archive_path");
            }
            let Component::Normal(root) = parts[0] else {
                return Err("app_archive_path");
            };
            let root = root.to_str().ok_or("app_archive_path")?;
            if !root.ends_with(".app") || root.len() > 255 {
                return Err("app_archive_root");
            }
            if app_root.as_ref().is_some_and(|old: &String| old != root) {
                return Err("app_archive_root");
            }
            app_root = Some(root.to_owned());
            let kind = entry.header().entry_type();
            if !(kind.is_file() || kind.is_dir() || kind.is_symlink() || kind.is_hard_link()) {
                return Err("app_archive_entry");
            }
            if kind.is_symlink() || kind.is_hard_link() {
                let target = entry
                    .link_name()
                    .map_err(|_| "app_archive_link")?
                    .ok_or("app_archive_link")?;
                let mut depth = if kind.is_hard_link() {
                    0
                } else {
                    parts.len() - 1
                };
                for part in target.components() {
                    match part {
                        Component::Normal(_) => depth += 1,
                        Component::CurDir => (),
                        Component::ParentDir if depth > 1 => depth -= 1,
                        _ => return Err("app_archive_link"),
                    }
                }
                if depth == 0 {
                    return Err("app_archive_link");
                }
            }
            if !entry.unpack_in(staging).map_err(|_| "app_archive_unpack")? {
                return Err("app_archive_path");
            }
        }
        let app = staging.join(app_root.ok_or("app_archive_empty")?);
        let metadata = std::fs::symlink_metadata(&app).map_err(|_| "app_archive_root")?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("app_archive_root");
        }
        Ok(app)
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(staging);
    }
    result
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Read, Write},
        process::{Child, Command, Stdio},
        sync::atomic::{AtomicU64, Ordering},
    };
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    struct TlsFixture {
        root: std::path::PathBuf,
        child: Child,
        port: u16,
    }
    fn isolated_tls_tool(root: &Path, name: &str) -> Command {
        use std::os::unix::fs::PermissionsExt;
        let executable = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .filter(|directory| directory.is_absolute())
            .find_map(|directory| {
                let path = directory.join(name).canonicalize().ok()?;
                let metadata = path.metadata().ok()?;
                let mode = metadata.permissions().mode();
                (metadata.is_file() && mode & 0o111 != 0 && mode & 0o022 == 0).then_some(path)
            })
            .expect("a trusted absolute TLS fixture executable is required");
        let mut command = Command::new(executable);
        command
            .env_clear()
            .env("HOME", root)
            .env("TMPDIR", root)
            .env("LC_ALL", "C")
            .current_dir(root);
        command
    }
    impl TlsFixture {
        fn new() -> Self {
            let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "switcher-update-tls-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            crate::storage::create_private_directory(&root).unwrap();
            std::fs::write(root.join("openssl.cnf"),"[req]\nprompt=no\ndistinguished_name=dn\nx509_extensions=ext\n[dn]\nCN=updates.example.com\n[ext]\nsubjectAltName=DNS:updates.example.com,IP:127.0.0.1\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n").unwrap();
            let result = isolated_tls_tool(&root, "openssl")
                .args([
                    "req",
                    "-x509",
                    "-newkey",
                    "rsa:2048",
                    "-nodes",
                    "-sha256",
                    "-days",
                    "1",
                    "-config",
                    "openssl.cnf",
                    "-keyout",
                    "key.pem",
                    "-out",
                    "cert.pem",
                ])
                .current_dir(&root)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap();
            assert!(result.success());
            let script = r#"import http.server,ssl,sys,pathlib,json,os
root=pathlib.Path(sys.argv[1])
assert sys.flags.isolated == 1
assert set(os.environ) <= {'HOME','TMPDIR','LC_ALL','LC_CTYPE','__CF_USER_TEXT_ENCODING'}
assert os.environ['HOME'] == str(root) == os.environ['TMPDIR']
class H(http.server.BaseHTTPRequestHandler):
 def log_message(self,*args): pass
 def do_GET(self):
  self.connection.settimeout(2)
  if self.headers.get('Transfer-Encoding') is not None:
   self.send_error(411); return
  try: length=int(self.headers.get('Content-Length','0'))
  except ValueError:
   self.send_error(400); return
  if not 0 <= length <= 4096:
   self.send_error(413); return
  body=self.rfile.read(length)
  if len(body) != length:
   self.send_error(400); return
  record={'method':self.command,'target':self.path,'headers':list(self.headers.raw_items()),'body':list(body)}
  with (root/'wire.jsonl').open('a') as f: f.write(json.dumps(record)+'\n')
  with (root/'requests.log').open('a') as f: f.write(self.path+'\n')
  with (root/'user-agents.log').open('a') as f: f.write(self.headers.get('User-Agent','')+'\n')
  if (root/'redirect').exists():
   self.send_response(302); self.send_header('Location','https://updates.example.com/never-follow'); self.end_headers(); return
  name=self.path.rsplit('/',1)[-1]
  if name not in ('metadata.json','app.tar.gz','helper.pkg'): self.send_error(404); return
  if name == 'app.tar.gz' and (root/'response-mode').exists():
   mode=(root/'response-mode').read_text()
   self.close_connection=True
   with (root/'responses.log').open('a') as f: f.write(mode+'\n')
   if mode in ('declared-short','close-short'):
    self.send_response(200)
    if mode == 'declared-short': self.send_header('Content-Length','4')
    self.send_header('Connection','close'); self.end_headers()
    self.wfile.write(b'tes'); self.wfile.flush()
    # Close TLS cleanly so declared-short isolates HTTP length truncation.
    try: self.connection.unwrap().close()
    except (ssl.SSLError,OSError): pass
    return
   if mode in ('chunk-exact','chunk-overflow'):
    self.protocol_version='HTTP/1.1'
    self.send_response(200); self.send_header('Transfer-Encoding','chunked')
    self.send_header('Connection','close'); self.end_headers()
    for chunk in ([b'te',b'st',b'!'] if mode == 'chunk-overflow' else [b'te',b'st']):
     self.wfile.write(('%x\r\n'%len(chunk)).encode()+chunk+b'\r\n'); self.wfile.flush()
    self.wfile.write(b'0\r\n\r\n'); self.wfile.flush(); return
   assert mode == 'full'
  data=(root/name).read_bytes()
  self.send_response(200); self.send_header('Content-Length',str(len(data))); self.end_headers(); self.wfile.write(data)
 do_POST=do_GET
server=http.server.HTTPServer(('127.0.0.1',0),H)
ctx=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);ctx.load_cert_chain(str(root/'cert.pem'),str(root/'key.pem'))
server.socket=ctx.wrap_socket(server.socket,server_side=True)
print(server.server_port,flush=True)
server.serve_forever()
"#;
            let mut child = isolated_tls_tool(&root, "python3")
                .arg("-I")
                .arg("-u")
                .arg("-c")
                .arg(script)
                .arg(&root)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let mut port = String::new();
            BufReader::new(child.stdout.take().unwrap())
                .read_line(&mut port)
                .unwrap();
            Self {
                root,
                child,
                port: port.trim().parse().unwrap(),
            }
        }
        fn client(&self, trust: bool) -> reqwest::Client {
            let mut builder = release_client_builder()
                .connect_timeout(std::time::Duration::from_secs(2))
                .timeout(std::time::Duration::from_secs(5))
                .resolve(
                    "updates.example.com",
                    std::net::SocketAddr::from(([127, 0, 0, 1], self.port)),
                );
            if trust {
                builder = builder.tls_certs_only([reqwest::Certificate::from_pem(
                    &std::fs::read(self.root.join("cert.pem")).unwrap(),
                )
                .unwrap()]);
            }
            builder.build().unwrap()
        }
        fn wire(&self) -> Vec<serde_json::Value> {
            let mut bytes = Vec::new();
            std::fs::File::open(self.root.join("wire.jsonl"))
                .unwrap()
                .take(65_537)
                .read_to_end(&mut bytes)
                .unwrap();
            assert!(bytes.len() <= 65_536, "fixture wire log exceeded bound");
            String::from_utf8(bytes)
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect()
        }
        fn pins() -> ValidatedReleasePins {
            ValidatedReleasePins::validate(ReleasePinSpec {
                edition: Edition::Public,
                metadata_url: "https://updates.example.com/public/stable/aarch64/metadata.json",
                metadata_key: ed25519_dalek::SigningKey::from_bytes(&[17; 32])
                    .verifying_key()
                    .to_bytes(),
                key_id: "software-a",
                minisign_key: super::tests::MINI_KEY,
                channel: "stable",
                architecture: "aarch64",
                app_bundle_id: "com.example.switcher",
                helper_bundle_id: "com.example.switcher.helper",
                team_id: "TESTTEAM01",
                artifact_origin: "https://updates.example.com",
                artifact_prefix: "/public/stable/aarch64/",
            })
            .unwrap()
        }
    }
    impl Drop for TlsFixture {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn actual_new_client_rejects_http_before_contacting_live_listener() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        // Positive TCP control proves this exact listener is reachable.
        let control =
            std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_secs(2))
                .unwrap();
        let (accepted, _) = listener.accept().unwrap();
        drop((control, accepted));
        listener.set_nonblocking(true).unwrap();
        let transport = ReleaseTransport::new(TlsFixture::pins()).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        // Probe the private client without relaxing production pin validation.
        let error = runtime.block_on(async {
            tokio::time::timeout(
                std::time::Duration::from_secs(3),
                transport
                    .client
                    .get(format!("http://{address}/metadata.json"))
                    .send(),
            )
            .await
            .expect("HTTP must be rejected before the production timeout")
            .unwrap_err()
        });
        assert!(error.is_builder(), "expected scheme rejection: {error:?}");
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
    #[test]
    fn actual_new_client_rejects_untrusted_tls_on_live_matching_ip_endpoint() {
        let mut fixture = TlsFixture::new();
        std::fs::write(fixture.root.join("metadata.json"), b"test").unwrap();
        // The fixture certificate includes this IP SAN. No DNS override is needed.
        let url = format!("https://127.0.0.1:{}/metadata.json", fixture.port);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let trusted = fixture.client(true);
        runtime.block_on(async {
            let response =
                tokio::time::timeout(std::time::Duration::from_secs(5), trusted.get(&url).send())
                    .await
                    .unwrap()
                    .unwrap();
            assert!(response.status().is_success());
            assert_eq!(response.bytes().await.unwrap().as_ref(), b"test");
        });
        let before = std::fs::read_to_string(fixture.root.join("requests.log")).unwrap();
        assert_eq!(before, "/metadata.json\n");
        assert!(fixture.child.try_wait().unwrap().is_none());
        let transport = ReleaseTransport::new(TlsFixture::pins()).unwrap();
        let error = runtime.block_on(async {
            tokio::time::timeout(
                std::time::Duration::from_secs(5),
                transport.client.get(&url).send(),
            )
            .await
            .expect("TLS rejection must finish before the production timeout")
            .unwrap_err()
        });
        assert!(
            error.is_connect(),
            "expected TLS connect rejection: {error:?}"
        );
        let reason = format!("{error:?}");
        // Platform certificate verifiers report either a rustls issuer error or
        // the macOS trust verdict. Both must identify certificate validation.
        assert!(
            reason.contains("InvalidCertificate")
                && (reason.contains("UnknownIssuer")
                    || reason.contains("certificate is not trusted")),
            "expected certificate trust rejection, not a dead endpoint: {error:?}"
        );
        assert!(fixture.child.try_wait().unwrap().is_none());
        assert_eq!(
            std::fs::read_to_string(fixture.root.join("requests.log")).unwrap(),
            before,
            "untrusted TLS must never reach the HTTP handler"
        );
    }
    #[test]
    fn controlled_tls_request_uses_production_user_agent() {
        let fixture = TlsFixture::new();
        std::fs::write(fixture.root.join("metadata.json"), b"test").unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let transport = ReleaseTransport {
            pins: TlsFixture::pins(),
            client: fixture.client(true),
        };
        assert_eq!(
            runtime.block_on(transport.fetch(transport.pins.metadata_url.as_str(), METADATA_LIMIT)),
            Ok(b"test".to_vec())
        );
        assert_eq!(
            std::fs::read_to_string(fixture.root.join("user-agents.log")).unwrap(),
            "ClaudeSwitcher/0.2 update\n"
        );
    }
    #[test]
    fn e11_consent_tls_check_and_download_have_distinct_exact_request_targets() {
        const CHILD: &str = "E11_UPDATE_WIRE_HOME_CHILD";
        const CANARIES: [&str; 4] = [
            "E11_ACCOUNT_A_MARKER_SYNTHETIC_ONLY",
            "E11_ACCOUNT_B_MARKER_SYNTHETIC_ONLY",
            "E11_ACTIVE_CLAUDE_MARKER_SYNTHETIC_ONLY",
            "E11_PRIVATE_FILE_CONTENT_SYNTHETIC_ONLY",
        ];
        if std::env::var_os(CHILD).is_none() {
            let home = TlsFixture::new();
            for (relative, marker) in [
                (".claude-switcher/accounts/A/session.json", CANARIES[0]),
                (".claude-switcher/accounts/B/session.json", CANARIES[1]),
                (
                    "Library/Application Support/Claude/session.json",
                    CANARIES[2],
                ),
                (".claude.json", CANARIES[3]),
            ] {
                let path = home.root.join(relative);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, marker).unwrap();
            }
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "updates::transport_tests::e11_consent_tls_check_and_download_have_distinct_exact_request_targets",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env("HOME", &home.root)
                .env(CHILD, &home.root)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "HOME child failed: stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            print!("{}", String::from_utf8_lossy(&output.stdout));
            return;
        }
        let home = crate::config::home_dir().unwrap();
        assert_eq!(
            home,
            std::path::PathBuf::from(std::env::var_os(CHILD).unwrap())
        );
        for include_helper in [false, true] {
            let fixture = TlsFixture::new();
            let key = ed25519_dalek::SigningKey::from_bytes(&[17; 32]);
            let mut metadata = super::tests::metadata();
            if include_helper {
                // Reuse signed synthetic bytes. Identity and URL remain bound to helper pins.
                let mut helper = metadata.app.clone();
                helper.url = "https://updates.example.com/public/stable/aarch64/helper.pkg".into();
                helper.bundle_id = "com.example.switcher.helper".into();
                metadata.helper = Some(helper);
            }
            std::fs::write(
                fixture.root.join("metadata.json"),
                super::tests::sign(&metadata, &key),
            )
            .unwrap();
            std::fs::write(fixture.root.join("app.tar.gz"), b"test").unwrap();
            std::fs::write(fixture.root.join("helper.pkg"), b"test").unwrap();
            let transport = ReleaseTransport {
                pins: TlsFixture::pins(),
                client: fixture.client(true),
            };
            let installed = Version::new(1, 0, 0);
            let mut state = UpdateState::default();
            let candidate = transport
                .check_blocking(&state, &installed, Some(&installed), 200, true)
                .unwrap();
            let log = || std::fs::read_to_string(fixture.root.join("requests.log")).unwrap();
            let prefix = "/public/stable/aarch64/";
            assert_eq!(log(), format!("{prefix}metadata.json\n"));
            assert!(state.accept(&candidate, false).unwrap());
            let (_, app) = transport
                .download_blocking(&candidate, ArtifactKind::App, 201)
                .unwrap();
            let helper = if include_helper {
                Some(
                    transport
                        .download_blocking(&candidate, ArtifactKind::Helper, 201)
                        .unwrap()
                        .1,
                )
            } else {
                assert_eq!(
                    transport
                        .download_blocking(&candidate, ArtifactKind::Helper, 201)
                        .unwrap_err(),
                    "no_helper_artifact"
                );
                None
            };
            state
                .mark_downloaded(&candidate, &app, helper.as_ref())
                .unwrap();
            let expected = format!(
                "{prefix}metadata.json\n{prefix}app.tar.gz\n{}",
                if include_helper {
                    format!("{prefix}helper.pkg\n")
                } else {
                    String::new()
                }
            );
            assert_eq!(log(), expected);
            let requests = fixture.wire();
            let targets: Vec<_> = expected.lines().collect();
            assert_eq!(requests.len(), targets.len());
            for (request, target) in requests.iter().zip(targets) {
                assert_eq!(request["method"], "GET");
                assert_eq!(request["target"], target);
                assert_eq!(request["body"], serde_json::json!([]));
                let headers = request["headers"].as_array().unwrap();
                let mut observed = std::collections::BTreeMap::new();
                for pair in headers {
                    let name = pair[0].as_str().unwrap().to_ascii_lowercase();
                    assert!(
                        matches!(name.as_str(), "host" | "user-agent" | "accept"),
                        "unexpected outbound header: {name}"
                    );
                    assert!(observed.insert(name, pair[1].as_str().unwrap()).is_none());
                }
                assert_eq!(observed.len(), 3);
                assert_eq!(observed["host"], "updates.example.com");
                assert_eq!(observed["user-agent"], "ClaudeSwitcher/0.2 update");
                assert_eq!(observed["accept"], "*/*");
                let serialized = serde_json::to_string(request).unwrap();
                for forbidden in CANARIES.into_iter().chain([home.to_str().unwrap()]) {
                    assert!(
                        !serialized.contains(forbidden),
                        "private data reached update wire"
                    );
                }
            }
            println!(
                "update wire helper_present={include_helper}: {} exact GET requests, empty bodies, fixed headers, synthetic HOME canaries absent",
                requests.len()
            );
            assert_eq!(
                state.candidate.as_ref().unwrap().stage,
                InstallStage::Downloaded
            );
        }
    }
    #[test]
    fn e11_update_wire_capture_observes_sensitive_headers_and_body_positive_control() {
        let fixture = TlsFixture::new();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let response = runtime
            .block_on(async {
                fixture
                    .client(true)
                    .post("https://updates.example.com/capture-control?synthetic=1")
                    .header("Cookie", "synthetic-cookie")
                    .header("Authorization", "Bearer synthetic-token")
                    .body("synthetic-private-body")
                    .send()
                    .await
            })
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
        let requests = fixture.wire();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["method"], "POST");
        assert_eq!(requests[0]["target"], "/capture-control?synthetic=1");
        assert_eq!(
            requests[0]["body"],
            serde_json::json!(b"synthetic-private-body".to_vec())
        );
        let headers = requests[0]["headers"].as_array().unwrap();
        for (name, value) in [
            ("cookie", "synthetic-cookie"),
            ("authorization", "Bearer synthetic-token"),
        ] {
            assert!(headers.iter().any(
                |pair| pair[0].as_str().unwrap().eq_ignore_ascii_case(name) && pair[1] == value
            ));
        }
    }
    #[test]
    fn e11_update_wire_capture_rejects_body_above_4096_byte_limit() {
        let fixture = TlsFixture::new();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let response = runtime
            .block_on(async {
                fixture
                    .client(true)
                    .post("https://updates.example.com/capture-control")
                    .body(vec![b'x'; 4097])
                    .send()
                    .await
            })
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::PAYLOAD_TOO_LARGE);
        assert!(!fixture.root.join("wire.jsonl").exists());
    }
    #[test]
    fn controlled_tls_checks_exact_candidate_bytes_rejects_redirect_bad_signature_and_oversize() {
        let fixture = TlsFixture::new();
        let key = ed25519_dalek::SigningKey::from_bytes(&[17; 32]);
        let wire = super::tests::sign(&super::tests::metadata(), &key);
        std::fs::write(fixture.root.join("metadata.json"), &wire).unwrap();
        std::fs::write(fixture.root.join("app.tar.gz"), b"test").unwrap();
        let transport = ReleaseTransport {
            pins: TlsFixture::pins(),
            client: fixture.client(true),
        };
        let installed = Version::new(1, 0, 0);
        let mut state = UpdateState::default();
        let candidate = transport
            .check_blocking(&state, &installed, Some(&installed), 200, true)
            .unwrap();
        state.accept(&candidate, false).unwrap();
        let (bytes, proof) = transport
            .download_blocking(&candidate, ArtifactKind::App, 200)
            .unwrap();
        assert_eq!(bytes, b"test");
        assert_eq!(proof.sha256(), signed::digest(b"test"));
        let untrusted = ReleaseTransport {
            pins: TlsFixture::pins(),
            client: fixture.client(false),
        };
        assert_eq!(
            untrusted
                .check_blocking(&state, &installed, Some(&installed), 200, true)
                .unwrap_err(),
            "update_network"
        );
        std::fs::write(fixture.root.join("app.tar.gz"), b"evil").unwrap();
        assert_eq!(
            transport
                .download_blocking(&candidate, ArtifactKind::App, 200)
                .unwrap_err(),
            "artifact_sha256"
        );
        std::fs::write(fixture.root.join("redirect"), b"yes").unwrap();
        assert_eq!(
            transport
                .check_blocking(&state, &installed, Some(&installed), 200, true)
                .unwrap_err(),
            "update_http_status_or_redirect"
        );
        assert!(
            !std::fs::read_to_string(fixture.root.join("requests.log"))
                .unwrap()
                .contains("never-follow")
        );
        std::fs::remove_file(fixture.root.join("redirect")).unwrap();
        std::fs::write(
            fixture.root.join("metadata.json"),
            vec![b' '; METADATA_LIMIT + 1],
        )
        .unwrap();
        assert_eq!(
            transport
                .check_blocking(&state, &installed, Some(&installed), 200, true)
                .unwrap_err(),
            "update_response_too_large"
        );
        let mut value: serde_json::Value = serde_json::from_slice(&wire).unwrap();
        value["payload"] = serde_json::Value::String("{}".into());
        std::fs::write(
            fixture.root.join("metadata.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
        assert!(
            transport
                .check_blocking(&state, &installed, Some(&installed), 200, true)
                .is_err()
        );
        assert_eq!(state.sequence.highest, 7);
    }
    fn download_boundary_fixture() -> (TlsFixture, ReleaseTransport, Candidate) {
        let fixture = TlsFixture::new();
        let key = ed25519_dalek::SigningKey::from_bytes(&[17; 32]);
        std::fs::write(
            fixture.root.join("metadata.json"),
            super::tests::sign(&super::tests::metadata(), &key),
        )
        .unwrap();
        std::fs::write(fixture.root.join("app.tar.gz"), b"test").unwrap();
        let transport = ReleaseTransport {
            pins: TlsFixture::pins(),
            client: fixture.client(true),
        };
        let installed = Version::new(1, 0, 0);
        let candidate = transport
            .check_blocking(
                &UpdateState::default(),
                &installed,
                Some(&installed),
                200,
                true,
            )
            .unwrap();
        (fixture, transport, candidate)
    }
    #[test]
    fn controlled_tls_download_rejects_truncated_responses() {
        let (fixture, transport, candidate) = download_boundary_fixture();
        for (mode, expected) in [
            ("full", None),
            ("declared-short", Some("update_network")),
            ("close-short", Some("artifact_size")),
        ] {
            std::fs::write(fixture.root.join("response-mode"), mode).unwrap();
            let result = transport.download_blocking(&candidate, ArtifactKind::App, 200);
            if let Some(error) = expected {
                assert_eq!(result.unwrap_err(), error, "{mode}");
            } else {
                let (bytes, proof) = result.unwrap();
                assert_eq!(bytes, b"test");
                assert_eq!(proof.sha256(), signed::digest(b"test"));
            }
        }
        assert_eq!(
            std::fs::read_to_string(fixture.root.join("responses.log")).unwrap(),
            "full\ndeclared-short\nclose-short\n"
        );
        assert_eq!(fixture.wire().len(), 4);
    }
    #[test]
    fn controlled_tls_download_enforces_unknown_length_stream_limit() {
        let (fixture, transport, candidate) = download_boundary_fixture();
        for (mode, expected) in [
            ("chunk-exact", None),
            ("chunk-overflow", Some("update_response_too_large")),
        ] {
            std::fs::write(fixture.root.join("response-mode"), mode).unwrap();
            let result = transport.download_blocking(&candidate, ArtifactKind::App, 200);
            if let Some(error) = expected {
                assert_eq!(result.unwrap_err(), error, "{mode}");
            } else {
                let (bytes, proof) = result.unwrap();
                assert_eq!(bytes, b"test");
                assert_eq!(proof.sha256(), signed::digest(b"test"));
            }
        }
        assert_eq!(
            std::fs::read_to_string(fixture.root.join("responses.log")).unwrap(),
            "chunk-exact\nchunk-overflow\n"
        );
        assert_eq!(fixture.wire().len(), 3);
    }
    #[test]
    fn archive_paths_links_and_expansion_are_bounded_before_apple_assessment() {
        fn archive(link: bool) -> Vec<u8> {
            let mut output =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            {
                let mut tar = tar::Builder::new(&mut output);
                let mut header = tar::Header::new_gnu();
                header.set_mode(0o755);
                if link {
                    header.set_entry_type(tar::EntryType::Symlink);
                    header.set_size(0);
                    header.set_link_name("/tmp/outside-switcher-test").unwrap();
                    header.set_cksum();
                    tar.append_data(&mut header, "Test.app/Contents/link", std::io::empty())
                        .unwrap();
                } else {
                    let data = b"test";
                    header.set_entry_type(tar::EntryType::Regular);
                    header.set_size(data.len() as u64);
                    header.set_cksum();
                    tar.append_data(&mut header, "Test.app/Contents/Info.plist", &data[..])
                        .unwrap();
                }
                tar.finish().unwrap();
            }
            output.flush().unwrap();
            output.finish().unwrap()
        }
        for link in [false, true] {
            let bytes = archive(link);
            let mut metadata = super::tests::metadata();
            metadata.app.size = bytes.len() as u64;
            metadata.app.sha256 = signed::digest(&bytes);
            let key = ed25519_dalek::SigningKey::from_bytes(&[17; 32]);
            let installed = Version::new(1, 0, 0);
            let candidate = authenticate_candidate(
                &super::tests::sign(&metadata, &key),
                &key.verifying_key(),
                &TlsFixture::pins().context(&installed, Some(&installed)),
                &Sequence::default(),
                200,
                true,
            )
            .unwrap();
            // This unit test isolates tar validation. It does not claim this synthetic
            // archive has a Minisign or Apple signature. Real callers cannot mint proof.
            let proof = VerifiedDownload {
                candidate_hash: candidate.payload_hash.clone(),
                artifact_hash: metadata.app.sha256,
                kind: ArtifactKind::App,
            };
            let staging = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "switcher-archive-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            let result = stage_app_archive(&candidate, &proof, &bytes, &staging);
            if link {
                assert_eq!(result.unwrap_err(), "app_archive_link");
                assert!(!staging.exists());
            } else {
                assert!(result.unwrap().join("Contents/Info.plist").exists());
                std::fs::remove_dir_all(staging).unwrap();
            }
        }
    }
}
