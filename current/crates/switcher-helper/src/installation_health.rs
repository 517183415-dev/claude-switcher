//! Narrow, root-owned installation attestation. This never commits an installation.
use crate::{platform::Platform, policy::SystemActions};
use serde::de::DeserializeOwned;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use switcher_domain::system::*;

pub fn boot_session_uuid() -> Result<String, &'static str> {
    crate::root_operation::boot_id()
}

/// Fixed, public-to-owner challenge. The nonce binds a receipt; it is not a secret.
pub fn load_challenge() -> Result<InstallHealthChallenge, &'static str> {
    let path = Path::new(SOURCE_LOCAL_HEALTH_CHALLENGE);
    crate::root_operation::safe_root(path.parent().ok_or("installation_health_path")?)?;
    read_json(path, 0, 0o644)
}

fn read_json<T: DeserializeOwned>(path: &Path, owner: u32, mode: u32) -> Result<T, &'static str> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| "installation_health_file")?;
    let metadata = file.metadata().map_err(|_| "installation_health_file")?;
    if !metadata.is_file()
        || metadata.uid() != owner
        || metadata.mode() & 0o7777 != mode
        || metadata.nlink() != 1
        || metadata.len() > 16384
    {
        return Err("installation_health_permissions");
    }
    let mut bytes = Vec::new();
    file.take(16385)
        .read_to_end(&mut bytes)
        .map_err(|_| "installation_health_read")?;
    if bytes.len() > 16384 {
        return Err("installation_health_size");
    }
    serde_json::from_slice(&bytes).map_err(|_| "installation_health_schema")
}

fn require_pending() -> Result<(), &'static str> {
    let metadata = fs::symlink_metadata(SOURCE_LOCAL_TRANSACTION)
        .map_err(|_| "installation_health_pending")?;
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
    {
        return Err("installation_health_pending");
    }
    Ok(())
}

pub(crate) fn verified_challenge(
    record: &InstallRecord,
    now: u64,
) -> Result<InstallHealthChallenge, &'static str> {
    require_pending()?;
    let current = crate::platform::load_root_record(Path::new(INSTALL_RECORD))?;
    if &current != record {
        return Err("installation_health_record_changed");
    }
    let challenge = load_challenge()?;
    challenge.validate_at(record, &boot_session_uuid()?, now)?;
    Ok(challenge)
}

pub(crate) fn allows_pending_action(action: &HelperAction) -> bool {
    matches!(
        action,
        HelperAction::BeginSession | HelperAction::InstallationHealth { .. }
    )
}

pub(crate) fn publish_receipt(receipt: &InstallHealthReceipt) -> Result<(), &'static str> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("root_required");
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock_unavailable")?
        .as_secs();
    let current = verified_challenge(&receipt.challenge.record, now)?;
    if current != receipt.challenge {
        return Err("installation_health_challenge_changed");
    }
    receipt.validate_at(&current.record, &boot_session_uuid()?, now)?;
    let path = Path::new(SOURCE_LOCAL_HEALTH_RECEIPT);
    crate::root_operation::safe_root(path.parent().ok_or("installation_health_path")?)?;
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Ok(m) if m.is_file() && m.uid() == 0 && m.nlink() == 1 && m.mode() & 0o7777 == 0o600 => {}
        _ => return Err("installation_health_receipt_permissions"),
    }
    let bytes = serde_json::to_vec(receipt).map_err(|_| "installation_health_encoding")?;
    atomic_receipt(path, &bytes)
}

fn atomic_receipt(path: &Path, bytes: &[u8]) -> Result<(), &'static str> {
    // Same-directory exclusive inode, private before writing, fsync before and after rename.
    let temp = path.with_extension(format!("{}.next", Platform.random_token()?));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&temp)
            .map_err(|_| "installation_health_receipt_create")?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| "installation_health_receipt_write")?;
        fs::rename(&temp, path).map_err(|_| "installation_health_receipt_commit")?;
        File::open(path.parent().ok_or("installation_health_path")?)
            .and_then(|f| f.sync_all())
            .map_err(|_| "installation_health_receipt_sync")
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

#[derive(Default)]
pub(crate) struct HealthTracker {
    receipt: Option<InstallHealthReceipt>,
}
impl HealthTracker {
    pub(crate) fn observe(
        &mut self,
        challenge: InstallHealthChallenge,
        record: &InstallRecord,
        role: ClientRole,
        response: (&str, &str, u64),
        now: u64,
    ) -> Result<Option<InstallHealthReceipt>, &'static str> {
        let (generation, nonce, sequence) = response;
        // The production loader independently validates the live boot and current root record.
        challenge.validate_at(record, &challenge.boot_session_uuid, now)?;
        if generation != challenge.generation || nonce != challenge.nonce {
            return Err("installation_health_binding");
        }
        match role {
            ClientRole::Gui if sequence == 0 => {}
            ClientRole::Guardian if sequence > 0 => {}
            _ => return Err("installation_health_role"),
        }
        if self
            .receipt
            .as_ref()
            .is_none_or(|r| r.challenge != challenge)
        {
            self.receipt = Some(InstallHealthReceipt {
                schema_version: 1,
                challenge,
                gui_verified: false,
                guardian_verified: false,
                guardian_loop_sequence: 0,
                observed_at: now,
            });
        }
        let receipt = self.receipt.as_mut().ok_or("installation_health_state")?;
        match role {
            ClientRole::Gui => receipt.gui_verified = true,
            ClientRole::Guardian => {
                receipt.guardian_verified = true;
                receipt.guardian_loop_sequence = sequence;
            }
            ClientRole::Installer => return Err("installation_health_role"),
        }
        receipt.observed_at = now;
        Ok((receipt.gui_verified && receipt.guardian_verified).then(|| receipt.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{Service, VerifiedPeer};
    use std::{
        cell::RefCell,
        os::unix::fs::{PermissionsExt, symlink},
    };
    fn record() -> InstallRecord {
        InstallRecord {
            schema_version: 2,
            owner_uid: 501,
            edition: switcher_domain::Edition::Personal,
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
    fn challenge() -> InstallHealthChallenge {
        InstallHealthChallenge {
            schema_version: 1,
            record: record(),
            generation: "a".repeat(32),
            nonce: "b".repeat(64),
            boot_session_uuid: "12345678-1234-1234-1234-123456789abc".into(),
            expires_at: 200,
        }
    }
    #[test]
    fn tracker_binds_full_record_nonce_expiry_and_challenge_key() {
        let mut tracker = HealthTracker::default();
        let c = challenge();
        let record = record();
        assert_eq!(
            tracker.observe(
                c.clone(),
                &record,
                ClientRole::Gui,
                (&c.generation, &c.nonce, 1),
                100
            ),
            Err("installation_health_role")
        );
        assert_eq!(
            tracker.observe(
                c.clone(),
                &record,
                ClientRole::Installer,
                (&c.generation, &c.nonce, 0),
                100
            ),
            Err("installation_health_role")
        );
        assert_eq!(
            tracker.observe(
                c.clone(),
                &record,
                ClientRole::Gui,
                (&c.generation, "bad", 0),
                100
            ),
            Err("installation_health_binding")
        );
        let mut changed = c.clone();
        changed.record.minimum_app_version = "9.0.0".into();
        assert_eq!(
            tracker.observe(
                changed,
                &record,
                ClientRole::Gui,
                (&c.generation, &c.nonce, 0),
                100
            ),
            Err("installation_health_challenge")
        );
        assert_eq!(
            tracker.observe(
                c.clone(),
                &record,
                ClientRole::Gui,
                (&c.generation, &c.nonce, 0),
                200
            ),
            Err("installation_health_challenge")
        );
        assert!(
            tracker
                .observe(
                    c.clone(),
                    &record,
                    ClientRole::Gui,
                    (&c.generation, &c.nonce, 0),
                    100
                )
                .unwrap()
                .is_none()
        );
        let mut second = c.clone();
        second.nonce = "c".repeat(64);
        // A new root challenge cannot reuse the old GUI observation.
        assert!(
            tracker
                .observe(
                    second.clone(),
                    &record,
                    ClientRole::Guardian,
                    (&second.generation, &second.nonce, 1),
                    101
                )
                .unwrap()
                .is_none()
        );
        let receipt = tracker
            .observe(
                second.clone(),
                &record,
                ClientRole::Gui,
                (&second.generation, &second.nonce, 0),
                102,
            )
            .unwrap()
            .unwrap();
        assert_eq!(receipt.challenge, second);
        assert_eq!(receipt.guardian_loop_sequence, 1);
        receipt
            .validate_at(&record, &c.boot_session_uuid, 102)
            .unwrap();
    }
    struct HealthSystem {
        challenge: InstallHealthChallenge,
        receipts: RefCell<Vec<InstallHealthReceipt>>,
    }
    impl SystemActions for HealthSystem {
        fn random_token(&self) -> Result<String, &'static str> {
            Ok("session-token".into())
        }
        fn read_state(&self, _: &str) -> Result<SystemState, &'static str> {
            panic!("health must not read system settings")
        }
        fn validate_policy(&self, _: &RegionPolicy) -> Result<(), &'static str> {
            panic!("health must not repair")
        }
        fn apply_plan(&self, _: &RepairPlan) -> Vec<RepairItemResult> {
            panic!("health must not repair")
        }
        fn guardian_status(&self, _: u32) -> Result<(), &'static str> {
            panic!("health must not substitute heartbeat")
        }
        fn installation_health_challenge(
            &self,
            _: &InstallRecord,
            _: u64,
        ) -> Result<InstallHealthChallenge, &'static str> {
            Ok(self.challenge.clone())
        }
        fn installation_health_receipt(
            &self,
            receipt: &InstallHealthReceipt,
        ) -> Result<(), &'static str> {
            self.receipts.borrow_mut().push(receipt.clone());
            Ok(())
        }
    }
    fn begin(service: &mut Service, system: &HealthSystem, peer: &VerifiedPeer) {
        let result = service.handle(
            peer,
            HelperRequest {
                role: peer.role,
                session: None,
                sequence: 0,
                operation: HelperAction::BeginSession,
            },
            100,
            system,
        );
        assert!(result.ok);
        assert_eq!(result.session.as_deref(), Some("session-token"));
    }
    fn health_request(role: ClientRole, sequence: u64, loop_sequence: u64) -> HelperRequest {
        HelperRequest {
            role,
            session: Some("session-token".into()),
            sequence,
            operation: HelperAction::InstallationHealth {
                generation: "a".repeat(32),
                nonce: "b".repeat(64),
                loop_sequence,
            },
        }
    }
    #[test]
    fn session_role_proofs_require_both_real_roles_and_guardian_loop_before_receipt() {
        let mut service = Service::new(record()).unwrap();
        let system = HealthSystem {
            challenge: challenge(),
            receipts: RefCell::new(vec![]),
        };
        let gui = VerifiedPeer {
            uid: 501,
            role: ClientRole::Gui,
            connection: 1,
        };
        let guardian = VerifiedPeer {
            uid: 501,
            role: ClientRole::Guardian,
            connection: 2,
        };
        begin(&mut service, &system, &gui);
        begin(&mut service, &system, &guardian);
        let forged = service.handle(
            &gui,
            health_request(ClientRole::Guardian, 1, 1),
            100,
            &system,
        );
        assert_eq!(forged.code, "role_identity");
        assert!(
            service
                .handle(&gui, health_request(ClientRole::Gui, 1, 0), 100, &system)
                .ok
        );
        assert!(system.receipts.borrow().is_empty());
        let invalid_loop = service.handle(
            &guardian,
            health_request(ClientRole::Guardian, 1, 0),
            100,
            &system,
        );
        assert_eq!(invalid_loop.code, "installation_health_role");
        assert!(system.receipts.borrow().is_empty());
        let ready = service.handle(
            &guardian,
            health_request(ClientRole::Guardian, 2, 7),
            101,
            &system,
        );
        assert!(ready.ok);
        assert_eq!(ready.code, "installation_health_ready");
        let receipts = system.receipts.borrow();
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].gui_verified && receipts[0].guardian_verified);
        assert_eq!(receipts[0].guardian_loop_sequence, 7);
        let replay = service.handle(
            &guardian,
            health_request(ClientRole::Guardian, 2, 7),
            101,
            &system,
        );
        assert_eq!(replay.code, "replayed_message");
        assert_eq!(receipts.len(), 1);
    }
    #[test]
    fn pending_exception_excludes_every_ordinary_root_action() {
        assert!(allows_pending_action(&HelperAction::BeginSession));
        assert!(allows_pending_action(
            &health_request(ClientRole::Gui, 1, 0).operation
        ));
        for action in [
            HelperAction::ReadSystemState,
            HelperAction::GuardianStatus,
            HelperAction::CommitInstallation,
            HelperAction::RestoreManagedSystem,
            HelperAction::ApplyRepair {
                plan_id: "x".into(),
            },
            HelperAction::RecoverRepair {
                plan_id: "x".into(),
            },
            HelperAction::RepairStatus {
                plan_id: "x".into(),
            },
            HelperAction::PreviewRepair {
                target: RegionPolicy {
                    timezone: "UTC".into(),
                    network_service: "Wi-Fi".into(),
                    ipv6: Ipv6Policy::Off,
                },
            },
            HelperAction::AuthorizeRepair {
                plan_id: "x".into(),
                recovery: false,
                external_form: vec![],
            },
        ] {
            assert!(!allows_pending_action(&action));
        }
    }
    #[test]
    fn safe_challenge_io_rejects_wrong_mode_links_fifo_and_unknown_fields() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("switcher-health-io-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let path = root.join("challenge.json");
        let owner = unsafe { libc::geteuid() };
        fs::write(&path, serde_json::to_vec(&challenge()).unwrap()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            read_json::<InstallHealthChallenge>(&path, owner, 0o644).unwrap(),
            challenge()
        );
        assert_eq!(
            read_json::<InstallHealthChallenge>(&path, owner.saturating_add(1), 0o644),
            Err("installation_health_permissions")
        );
        assert!(crate::root_operation::safe_root(&root).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
        assert_eq!(
            read_json::<InstallHealthChallenge>(&path, owner, 0o644),
            Err("installation_health_permissions")
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let alias = root.join("alias");
        fs::hard_link(&path, &alias).unwrap();
        assert_eq!(
            read_json::<InstallHealthChallenge>(&path, owner, 0o644),
            Err("installation_health_permissions")
        );
        fs::remove_file(alias).unwrap();
        let mut unknown = serde_json::to_value(challenge()).unwrap();
        unknown["extra"] = true.into();
        fs::write(&path, serde_json::to_vec(&unknown).unwrap()).unwrap();
        assert_eq!(
            read_json::<InstallHealthChallenge>(&path, owner, 0o644),
            Err("installation_health_schema")
        );
        fs::remove_file(&path).unwrap();
        symlink(root.join("missing"), &path).unwrap();
        assert_eq!(
            read_json::<InstallHealthChallenge>(&path, owner, 0o644),
            Err("installation_health_file")
        );
        fs::remove_file(&path).unwrap();
        let raw = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(raw.as_ptr(), 0o644) }, 0);
        assert_eq!(
            read_json::<InstallHealthChallenge>(&path, owner, 0o644),
            Err("installation_health_permissions")
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn receipt_atomic_write_keeps_private_file_and_complete_challenge() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("switcher-health-receipt-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let path = root.join("receipt.json");
        let receipt = InstallHealthReceipt {
            schema_version: 1,
            challenge: challenge(),
            gui_verified: true,
            guardian_verified: true,
            guardian_loop_sequence: 2,
            observed_at: 100,
        };
        atomic_receipt(&path, &serde_json::to_vec(&receipt).unwrap()).unwrap();
        assert_eq!(
            read_json::<InstallHealthReceipt>(&path, unsafe { libc::geteuid() }, 0o600).unwrap(),
            receipt
        );
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
