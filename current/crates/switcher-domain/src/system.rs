//! Fixed messages used at the privileged boundary.
use crate::Edition;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const HELPER_LABEL: &str = "local.claudeswitcher.helper";
pub const GUARDIAN_LABEL: &str = "local.claudeswitcher.guardian";
pub const SYSTEM_ROOT: &str = "/Library/Application Support/claude-switcher";
pub const INSTALL_RECORD: &str = "/Library/Application Support/claude-switcher/install.json";
pub const SOURCE_LOCAL_TRANSACTION: &str =
    "/Library/Application Support/claude-switcher/source-local-transaction.json";
pub const SOURCE_LOCAL_HEALTH_CHALLENGE: &str =
    "/Library/Application Support/claude-switcher/source-local-health-challenge.json";
pub const SOURCE_LOCAL_HEALTH_RECEIPT: &str =
    "/Library/Application Support/claude-switcher/source-local-health-receipt.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ClientRole {
    Gui,
    Guardian,
    Installer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct InstallRecord {
    pub schema_version: u32,
    pub owner_uid: u32,
    pub edition: Edition,
    pub trust: InstallTrust,
    pub helper_version: String,
    pub minimum_app_version: String,
}

/// Source-local trust is enrolled only by the administrator's local installer.
/// It cannot be refreshed by an XPC request or a remote update manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InstallTrust {
    DeveloperId {
        team_id: String,
    },
    SourceLocal {
        generation: String,
        gui_cdhash: String,
        guardian_cdhash: String,
        helper_cdhash: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct InstallHealthChallenge {
    pub schema_version: u32,
    pub record: InstallRecord,
    pub generation: String,
    pub nonce: String,
    pub boot_session_uuid: String,
    #[ts(type = "number")]
    pub expires_at: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct InstallHealthReceipt {
    pub schema_version: u32,
    pub challenge: InstallHealthChallenge,
    pub gui_verified: bool,
    pub guardian_verified: bool,
    #[ts(type = "number")]
    pub guardian_loop_sequence: u64,
    #[ts(type = "number")]
    pub observed_at: u64,
}
impl InstallHealthChallenge {
    pub fn validate_at(
        &self,
        record: &InstallRecord,
        boot: &str,
        now: u64,
    ) -> Result<(), &'static str> {
        record.validate()?;
        let InstallTrust::SourceLocal { generation, .. } = &record.trust else {
            return Err("installation_health_source_local");
        };
        if self.schema_version != 1
            || &self.record != record
            || &self.generation != generation
            || !lower_hex(&self.generation, 32)
            || !lower_hex(&self.nonce, 64)
            || !boot_uuid(&self.boot_session_uuid)
            || self.boot_session_uuid != boot
            || now >= self.expires_at
        {
            return Err("installation_health_challenge");
        }
        Ok(())
    }
}
impl InstallHealthReceipt {
    pub fn validate_at(
        &self,
        record: &InstallRecord,
        boot: &str,
        now: u64,
    ) -> Result<(), &'static str> {
        self.challenge.validate_at(record, boot, now)?;
        if self.schema_version != 1
            || !self.gui_verified
            || !self.guardian_verified
            || self.guardian_loop_sequence == 0
            || self.observed_at > now
            || self.observed_at >= self.challenge.expires_at
        {
            return Err("installation_health_receipt");
        }
        Ok(())
    }
}
fn boot_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Ipv6Policy {
    Off,
    Automatic,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RegionPolicy {
    pub timezone: String,
    pub network_service: String,
    pub ipv6: Ipv6Policy,
}

#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, tag = "action", rename_all = "snake_case")]
pub enum HelperAction {
    BeginSession,
    InstallationHealth {
        generation: String,
        nonce: String,
        #[ts(type = "number")]
        loop_sequence: u64,
    },
    ReadSystemState,
    PreviewRepair {
        target: RegionPolicy,
    },
    /// Native credential accepted only on the authenticated connection that owns the plan.
    AuthorizeRepair {
        plan_id: String,
        recovery: bool,
        external_form: Vec<u8>,
    },
    ApplyRepair {
        plan_id: String,
    },
    RepairStatus {
        plan_id: String,
    },
    RecoverRepair {
        plan_id: String,
    },
    ReconcileManagedTimezone {
        expected_timezone: String,
    },
    GuardianStatus,
    CommitInstallation,
    RestoreManagedSystem,
}

#[derive(Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct HelperRequest {
    pub role: ClientRole,
    pub session: Option<String>,
    #[ts(type = "number")]
    pub sequence: u64,
    pub operation: HelperAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SystemState {
    pub timezone: String,
    pub network_service: String,
    pub ipv6: Ipv6Policy,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RepairPlan {
    pub id: String,
    pub original: SystemState,
    pub target: RegionPolicy,
    #[ts(type = "number")]
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct RepairItemResult {
    pub item: String,
    pub changed: bool,
    pub restored: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct HelperResponse {
    pub ok: bool,
    pub code: String,
    pub session: Option<String>,
    pub state: Option<SystemState>,
    pub plan: Option<RepairPlan>,
    pub items: Vec<RepairItemResult>,
}

impl HelperResponse {
    pub fn status(code: &str, ok: bool) -> Self {
        Self {
            ok,
            code: code.into(),
            session: None,
            state: None,
            plan: None,
            items: vec![],
        }
    }
}

impl InstallRecord {
    pub fn app_bundle_id(&self) -> &'static str {
        match self.edition {
            Edition::Personal => "local.claudeswitcher.personal",
            Edition::Public => "local.claudeswitcher.public",
        }
    }
    pub fn bundle_id(&self, role: ClientRole) -> &str {
        match role {
            ClientRole::Gui => self.app_bundle_id(),
            ClientRole::Guardian => "local.claudeswitcher.guardian",
            ClientRole::Installer => "local.claudeswitcher.installer",
        }
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != 2 || self.owner_uid == 0 {
            return Err("install_record");
        }
        match &self.trust {
            InstallTrust::DeveloperId { team_id } => {
                if team_id.len() != 10
                    || !team_id
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
                {
                    return Err("team_id");
                }
            }
            InstallTrust::SourceLocal {
                generation,
                gui_cdhash,
                guardian_cdhash,
                helper_cdhash,
            } => {
                if !lower_hex(generation, 32)
                    || [gui_cdhash, guardian_cdhash, helper_cdhash]
                        .iter()
                        .any(|v| !lower_hex(v, 40))
                {
                    return Err("local_code_identity");
                }
            }
        }
        for version in [&self.helper_version, &self.minimum_app_version] {
            if version.is_empty()
                || version.len() > 48
                || !version
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b))
            {
                return Err("component_version");
            }
        }
        Ok(())
    }

    /// Branch selection only. Call `validate` before using an installation as authority.
    pub fn is_source_local(&self) -> bool {
        matches!(self.trust, InstallTrust::SourceLocal { .. })
    }

    pub fn developer_team(&self) -> Result<&str, &'static str> {
        self.validate()?;
        match &self.trust {
            InstallTrust::DeveloperId { team_id } => Ok(team_id),
            InstallTrust::SourceLocal { .. } => Err("source_local_rebuild_required"),
        }
    }

    pub fn client_requirement(&self, role: ClientRole) -> Result<String, &'static str> {
        self.validate()?;
        let pin = match (&self.trust, role) {
            (InstallTrust::SourceLocal { gui_cdhash, .. }, ClientRole::Gui) => {
                Some(gui_cdhash.as_str())
            }
            (
                InstallTrust::SourceLocal {
                    guardian_cdhash, ..
                },
                ClientRole::Guardian,
            ) => Some(guardian_cdhash.as_str()),
            (InstallTrust::SourceLocal { .. }, ClientRole::Installer) => {
                return Err("local_installer_role_unavailable");
            }
            (InstallTrust::DeveloperId { .. }, _) => None,
        };
        self.requirement(self.bundle_id(role), pin)
    }

    pub fn helper_requirement(&self) -> Result<String, &'static str> {
        self.validate()?;
        let pin = match &self.trust {
            InstallTrust::SourceLocal { helper_cdhash, .. } => Some(helper_cdhash.as_str()),
            InstallTrust::DeveloperId { .. } => None,
        };
        self.requirement(HELPER_LABEL, pin)
    }

    fn requirement(&self, bundle: &str, pin: Option<&str>) -> Result<String, &'static str> {
        match (&self.trust, pin) {
            (InstallTrust::DeveloperId { team_id }, None) => Ok(format!(
                "anchor apple generic and certificate leaf[subject.OU] = \"{team_id}\" and identifier \"{bundle}\""
            )),
            (InstallTrust::SourceLocal { .. }, Some(pin)) => {
                Ok(format!("identifier \"{bundle}\" and cdhash H\"{pin}\""))
            }
            _ => Err("install_trust"),
        }
    }
}

fn lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && value.bytes().any(|b| b != b'0')
}

#[cfg(test)]
mod install_trust_tests {
    use super::*;

    fn local() -> InstallRecord {
        InstallRecord {
            schema_version: 2,
            owner_uid: 501,
            edition: Edition::Personal,
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
    fn local_requirements_bind_fixed_role_and_exact_code() {
        let mut record = local();
        assert_eq!(
            record.client_requirement(ClientRole::Gui).unwrap(),
            format!(
                "identifier \"local.claudeswitcher.personal\" and cdhash H\"{}\"",
                "1".repeat(40)
            )
        );
        assert!(
            record
                .client_requirement(ClientRole::Guardian)
                .unwrap()
                .contains(&"2".repeat(40))
        );
        assert!(
            record
                .helper_requirement()
                .unwrap()
                .contains(&"3".repeat(40))
        );
        assert_eq!(
            record.client_requirement(ClientRole::Installer),
            Err("local_installer_role_unavailable")
        );
        assert_eq!(
            record.developer_team(),
            Err("source_local_rebuild_required")
        );
        record.edition = Edition::Public;
        assert!(
            record
                .client_requirement(ClientRole::Gui)
                .unwrap()
                .contains("local.claudeswitcher.public")
        );
    }

    #[test]
    fn invalid_and_legacy_records_never_downgrade_to_local_trust() {
        let mut record = local();
        record.schema_version = 1;
        assert_eq!(record.helper_requirement(), Err("install_record"));
        record.schema_version = 2;
        for bad in [
            "0".repeat(40),
            "a".repeat(39),
            "A".repeat(40),
            "\"".repeat(40),
        ] {
            if let InstallTrust::SourceLocal { helper_cdhash, .. } = &mut record.trust {
                *helper_cdhash = bad;
            }
            assert_eq!(record.helper_requirement(), Err("local_code_identity"));
        }
    }

    #[test]
    fn developer_identity_keeps_apple_anchor_and_team() {
        let mut record = local();
        record.trust = InstallTrust::DeveloperId {
            team_id: "TESTTEAM01".into(),
        };
        assert_eq!(record.developer_team(), Ok("TESTTEAM01"));
        assert_eq!(
            record.client_requirement(ClientRole::Installer).unwrap(),
            "anchor apple generic and certificate leaf[subject.OU] = \"TESTTEAM01\" and identifier \"local.claudeswitcher.installer\""
        );
        assert!(!record.is_source_local());
    }
    fn health_challenge() -> InstallHealthChallenge {
        InstallHealthChallenge {
            schema_version: 1,
            record: local(),
            generation: "a".repeat(32),
            nonce: "b".repeat(64),
            boot_session_uuid: "12345678-1234-1234-1234-123456789abc".into(),
            expires_at: 200,
        }
    }
    #[test]
    fn installation_challenge_binds_boot_expiry_record_and_local_generation() {
        let c = health_challenge();
        let record = local();
        assert_eq!(c.validate_at(&record, &c.boot_session_uuid, 100), Ok(()));
        assert_eq!(
            c.validate_at(&record, "87654321-1234-1234-1234-123456789abc", 100),
            Err("installation_health_challenge")
        );
        assert_eq!(
            c.validate_at(&record, &c.boot_session_uuid, 200),
            Err("installation_health_challenge")
        );
        let mut bad = c.clone();
        bad.record.minimum_app_version = "9.0.0".into();
        assert_eq!(
            bad.validate_at(&record, &c.boot_session_uuid, 100),
            Err("installation_health_challenge")
        );
        bad = c.clone();
        bad.generation = "c".repeat(32);
        assert_eq!(
            bad.validate_at(&record, &c.boot_session_uuid, 100),
            Err("installation_health_challenge")
        );
        bad = c.clone();
        bad.nonce = "B".repeat(64);
        assert_eq!(
            bad.validate_at(&record, &c.boot_session_uuid, 100),
            Err("installation_health_challenge")
        );
        bad = c.clone();
        bad.schema_version = 2;
        assert_eq!(
            bad.validate_at(&record, &c.boot_session_uuid, 100),
            Err("installation_health_challenge")
        );
    }
    #[test]
    fn installation_receipt_requires_both_roles_loop_and_valid_observation_time() {
        let c = health_challenge();
        let record = local();
        let boot = &c.boot_session_uuid.clone();
        let receipt = InstallHealthReceipt {
            schema_version: 1,
            challenge: c,
            gui_verified: true,
            guardian_verified: true,
            guardian_loop_sequence: 1,
            observed_at: 100,
        };
        assert_eq!(receipt.validate_at(&record, boot, 101), Ok(()));
        let mut bad = receipt.clone();
        bad.gui_verified = false;
        assert_eq!(
            bad.validate_at(&record, boot, 101),
            Err("installation_health_receipt")
        );
        bad = receipt.clone();
        bad.guardian_verified = false;
        assert_eq!(
            bad.validate_at(&record, boot, 101),
            Err("installation_health_receipt")
        );
        bad = receipt.clone();
        bad.guardian_loop_sequence = 0;
        assert_eq!(
            bad.validate_at(&record, boot, 101),
            Err("installation_health_receipt")
        );
        bad = receipt.clone();
        bad.observed_at = 102;
        assert_eq!(
            bad.validate_at(&record, boot, 101),
            Err("installation_health_receipt")
        );
        assert_eq!(
            receipt.validate_at(&record, boot, 200),
            Err("installation_health_challenge")
        );
    }
}
