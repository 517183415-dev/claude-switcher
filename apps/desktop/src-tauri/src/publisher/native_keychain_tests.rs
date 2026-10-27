//! Explicit native acceptance only. Default unit tests never access Keychain.
use super::{KeychainVault, Secrets, Vault};
use crate::publisher::{random_id, Error, Publisher, Result};
use security_framework::os::macos::keychain::SecKeychain;
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier, Mutex};

const SERVICE: &str = "com.claude-switcher.personal.publisher";

#[derive(Clone)]
struct OwnedVault {
    items: Arc<Mutex<BTreeSet<String>>>,
    journal: Arc<PathBuf>,
}

impl OwnedVault {
    fn new(root: &Path) -> Self {
        Self {
            items: Default::default(),
            journal: Arc::new(root.join("owned-keychain-items.json")),
        }
    }

    fn owns(&self, generation: &str) -> bool {
        self.items.lock().unwrap().contains(generation)
    }

    fn cleanup(&self) -> bool {
        let mut items = self.items.lock().unwrap();
        let mut remaining = BTreeSet::new();
        for generation in items.iter() {
            let removed =
                security_framework::passwords::delete_generic_password(SERVICE, generation);
            if removed.is_err() || !matches!(KeychainVault.load_optional(generation), Ok(None)) {
                remaining.insert(generation.clone());
            }
        }
        *items = remaining;
        let recorded = switcher_core::storage::atomic_write(
            &self.journal,
            &serde_json::to_vec(&*items).unwrap(),
        );
        items.is_empty() && recorded.is_ok()
    }
}

impl Vault for OwnedVault {
    fn load(&self, generation: &str) -> Result<Secrets> {
        if !self.owns(generation) {
            return Err(Error::SecretUnavailable);
        }
        KeychainVault.load(generation)
    }

    fn load_optional(&self, generation: &str) -> Result<Option<Secrets>> {
        self.load(generation).map(Some)
    }

    fn create(&mut self, generation: &str, secrets: &Secrets) -> Result<()> {
        KeychainVault.create(generation, secrets)?;
        let mut items = self.items.lock().unwrap();
        items.insert(generation.into());
        switcher_core::storage::atomic_write(&self.journal, &serde_json::to_vec(&*items).unwrap())?;
        Ok(())
    }

    fn store(&mut self, generation: &str, secrets: &Secrets) -> Result<()> {
        if !self.owns(generation) {
            return Err(Error::SecretUnavailable);
        }
        KeychainVault.store(generation, secrets)
    }
}

fn secrets(value: u8) -> Secrets {
    Secrets {
        signing_seed: vec![value; 32],
        preview_credential: b"native-synthetic-preview-only".to_vec(),
        production_credential: b"native-synthetic-production-only".to_vec(),
    }
}

fn same_secrets(actual: &Secrets, expected: &Secrets) {
    // Never format secret bytes, including in a failed assertion.
    assert!(actual.signing_seed == expected.signing_seed);
    assert!(actual.preview_credential == expected.preview_credential);
    assert!(actual.production_credential == expected.production_credential);
}

fn exercise(root: &Path, vault: &mut OwnedVault) {
    let publisher_root = root.join("publisher");
    let mut publisher =
        Publisher::open(&publisher_root, "native-synthetic-key", None, 1000).unwrap();
    let original_generation = publisher.generation.clone();
    assert!(matches!(
        KeychainVault.load_optional(&original_generation),
        Ok(None)
    ));
    let original = secrets(0x41);
    vault.create(&original_generation, &original).unwrap();
    publisher.credentials_save(vault, &original).unwrap();
    assert!(vault.create(&original_generation, &secrets(0x42)).is_err());
    same_secrets(&vault.load(&original_generation).unwrap(), &original);

    let contested = random_id().unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let writers: Vec<_> = (0x51..=0x52)
        .map(|value| {
            let (mut writer, generation, barrier) =
                (vault.clone(), contested.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                (value, writer.create(&generation, &secrets(value)).is_ok())
            })
        })
        .collect();
    let winners: Vec<_> = writers
        .into_iter()
        .map(|t| t.join().unwrap())
        .filter(|(_, created)| *created)
        .collect();
    assert!(winners.len() == 1);
    same_secrets(&vault.load(&contested).unwrap(), &secrets(winners[0].0));

    let password = b"native-synthetic-backup-password";
    let backup = zeroize::Zeroizing::new(publisher.export_backup(password, vault).unwrap());
    publisher.pause(None, 1001).unwrap();
    let old_state_path = publisher_root.join(&original_generation).join("state.json");
    let old_state = fs::read(&old_state_path).unwrap();
    let active_before = fs::read(publisher_root.join("active.json")).unwrap();
    assert!(publisher
        .restore_backup(&backup, b"incorrect-synthetic-password", vault)
        .is_err());
    assert!(fs::read(publisher_root.join("active.json")).unwrap() == active_before);
    assert!(fs::read(&old_state_path).unwrap() == old_state);
    assert!(vault.items.lock().unwrap().len() == 2);

    publisher.restore_backup(&backup, password, vault).unwrap();
    let restored_generation = publisher.generation.clone();
    assert!(restored_generation != original_generation);
    assert!(vault.items.lock().unwrap().len() == 3);
    assert!(fs::read(&old_state_path).unwrap() == old_state);
    same_secrets(&vault.load(&original_generation).unwrap(), &original);
    same_secrets(&vault.load(&restored_generation).unwrap(), &original);
    assert!(publisher.state.recovery_required);
    assert!(!publisher_root.join("public").exists());
    drop(publisher);
    let reopened = Publisher::open(&publisher_root, "native-synthetic-key", None, 1002).unwrap();
    assert!(reopened.generation == restored_generation);
    assert!(reopened.state.recovery_required);
    same_secrets(&vault.load(&reopened.generation).unwrap(), &original);
}

#[test]
#[ignore = "explicit native Keychain acceptance; creates and removes only synthetic owned items"]
fn native_keychain_create_race_and_backup_restore_preserve_owned_items() {
    assert!(
        std::env::var("SWITCHER_NATIVE_KEYCHAIN_OPT_IN").as_deref()
            == Ok("synthetic-owned-items-only")
    );
    let root = PathBuf::from(std::env::var_os("SWITCHER_NATIVE_KEYCHAIN_ROOT").unwrap());
    let output = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .join("output")
        .canonicalize()
        .unwrap();
    assert!(root
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap()
        .starts_with(output));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let _interaction = SecKeychain::disable_user_interaction().unwrap();
    let mut vault = OwnedVault::new(&root);
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| exercise(&root, &mut vault)));
    let cleaned = vault.cleanup();
    let receipt = serde_json::json!({"exercise_passed": result.is_ok(), "owned_items_removed": cleaned,
        "scope": "native KeychainVault plus direct Publisher; same-process reopen only"});
    switcher_core::storage::atomic_write(
        &root.join("result.json"),
        &serde_json::to_vec(&receipt).unwrap(),
    )
    .unwrap();
    assert!(
        cleaned,
        "native cleanup incomplete; retain exact owned-item journal"
    );
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}
