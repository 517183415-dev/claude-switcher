//! Explicit native acceptance only. Default unit tests never access Keychain.
use super::{KEYCHAIN_SERVICE, KeychainVault, Secrets, Vault};
use crate::publisher::{Error, Publisher, Result, random_id};
use core_foundation::{
    array::{CFArray, CFArrayRef},
    base::{CFTypeRef, TCFType},
};
use security_framework::os::macos::keychain::{CreateOptions, SecKeychain};
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier, Mutex};

unsafe extern "C" {
    fn SecKeychainCopySearchList(list: *mut CFArrayRef) -> i32;
    fn SecKeychainDelete(keychain: CFTypeRef) -> i32;
}

fn preferences() -> (
    std::result::Result<SecKeychain, i32>,
    std::result::Result<CFArray<SecKeychain>, i32>,
) {
    let default = SecKeychain::default().map_err(|error| error.code());
    let mut array = std::ptr::null();
    // The API returns a retained array; the wrapper owns exactly that retain.
    let code = unsafe { SecKeychainCopySearchList(&mut array) };
    let list = if code == 0 {
        assert!(!array.is_null());
        Ok(unsafe { CFArray::wrap_under_create_rule(array) })
    } else {
        Err(code)
    };
    (default, list)
}

struct PrivateKeychain(Option<SecKeychain>);

impl PrivateKeychain {
    fn create(root: &Path, name: &str) -> Self {
        // These fixed private names cannot match login.keychain or System.keychain.
        assert!(matches!(
            name,
            "isolated-a.keychain" | "isolated-b.keychain"
        ));
        let keychain = CreateOptions::new()
            .password("synthetic-private-keychain-password")
            .create(root.join(name))
            .unwrap();
        Self(Some(keychain))
    }

    fn vault(&self) -> KeychainVault {
        KeychainVault::isolated(self.0.as_ref().unwrap().clone())
    }

    fn remove(&mut self) -> bool {
        let Some(keychain) = self.0.as_ref() else {
            return true;
        };
        // Only the explicit private handle created above reaches this API.
        if unsafe { SecKeychainDelete(keychain.as_CFTypeRef()) } != 0 {
            return false;
        }
        self.0 = None;
        true
    }
}

impl Drop for PrivateKeychain {
    fn drop(&mut self) {
        let _ = self.remove();
    }
}

#[derive(Clone)]
struct OwnedVault {
    items: Arc<Mutex<BTreeSet<String>>>,
    journal: Arc<PathBuf>,
    inner: KeychainVault,
}

impl OwnedVault {
    fn new(root: &Path, inner: KeychainVault) -> Self {
        Self {
            items: Default::default(),
            journal: Arc::new(root.join("owned-keychain-items.json")),
            inner,
        }
    }

    fn owns(&self, generation: &str) -> bool {
        self.items.lock().unwrap().contains(generation)
    }

    fn cleanup(&self) -> bool {
        let mut items = self.items.lock().unwrap();
        let mut remaining = BTreeSet::new();
        for generation in items.iter() {
            let found = security_framework::os::macos::passwords::find_generic_password(
                Some(std::slice::from_ref(self.inner.keychain.as_ref().unwrap())),
                KEYCHAIN_SERVICE,
                generation,
            );
            let removed = found.map(|(_, item)| item.delete());
            if removed.is_err() || !matches!(self.inner.load_optional(generation), Ok(None)) {
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
        self.inner.load(generation)
    }

    fn load_optional(&self, generation: &str) -> Result<Option<Secrets>> {
        self.load(generation).map(Some)
    }

    fn create(&mut self, generation: &str, secrets: &Secrets) -> Result<()> {
        self.inner.create(generation, secrets)?;
        let mut items = self.items.lock().unwrap();
        items.insert(generation.into());
        switcher_core::storage::atomic_write(&self.journal, &serde_json::to_vec(&*items).unwrap())?;
        Ok(())
    }

    fn store(&mut self, generation: &str, secrets: &Secrets) -> Result<()> {
        if !self.owns(generation) {
            return Err(Error::SecretUnavailable);
        }
        self.inner.store(generation, secrets)
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

fn exercise(root: &Path, vault: &mut OwnedVault, other: &mut KeychainVault) {
    let publisher_root = root.join("publisher");
    let mut publisher =
        Publisher::open(&publisher_root, "native-synthetic-key", None, 1000).unwrap();
    let original_generation = publisher.generation.clone();
    assert!(matches!(
        vault.inner.load_optional(&original_generation),
        Ok(None)
    ));
    other.create(&original_generation, &secrets(0x77)).unwrap();
    let original = secrets(0x41);
    vault.create(&original_generation, &original).unwrap();
    publisher.credentials_save(vault, &original).unwrap();
    assert!(vault.create(&original_generation, &secrets(0x42)).is_err());
    same_secrets(&vault.load(&original_generation).unwrap(), &original);
    same_secrets(&other.load(&original_generation).unwrap(), &secrets(0x77));

    // Both operations must reject invalid secret shapes before changing the item.
    let mut short_seed = secrets(0x41);
    short_seed.signing_seed.pop();
    let mut oversized_credential = secrets(0x41);
    oversized_credential.preview_credential = vec![0; 16 * 1024 + 1];
    for invalid in [short_seed, oversized_credential] {
        assert!(vault.store(&original_generation, &invalid).is_err());
        assert!(vault.create(&random_id().unwrap(), &invalid).is_err());
        same_secrets(&vault.load(&original_generation).unwrap(), &original);
    }

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
    assert!(
        publisher
            .restore_backup(&backup, b"incorrect-synthetic-password", vault)
            .is_err()
    );
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
    same_secrets(&other.load(&original_generation).unwrap(), &secrets(0x77));
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
    assert!(
        root.parent()
            .unwrap()
            .canonicalize()
            .unwrap()
            .starts_with(output)
    );
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let _interaction = SecKeychain::disable_user_interaction().unwrap();
    let before = preferences();
    let mut first = PrivateKeychain::create(&root, "isolated-a.keychain");
    let mut second = PrivateKeychain::create(&root, "isolated-b.keychain");
    let mut vault = OwnedVault::new(&root, first.vault());
    let mut other = second.vault();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        exercise(&root, &mut vault, &mut other)
    }));
    let cleaned = vault.cleanup();
    let private_removed = first.remove() & second.remove();
    let unchanged = before == preferences();
    let receipt = serde_json::json!({"exercise_passed": result.is_ok(), "owned_items_removed": cleaned,
        "private_keychains_removed": private_removed, "preferences_unchanged": unchanged,
        "scope": "native KeychainVault with explicit private keychains plus direct Publisher; same-process reopen only"});
    switcher_core::storage::atomic_write(
        &root.join("result.json"),
        &serde_json::to_vec(&receipt).unwrap(),
    )
    .unwrap();
    assert!(
        cleaned,
        "native cleanup incomplete; retain exact owned-item journal"
    );
    assert!(
        private_removed && unchanged,
        "private cleanup or preference invariance failed"
    );
    if let Err(error) = result {
        std::panic::resume_unwind(error);
    }
}

#[path = "native_keychain_process_tests.rs"]
mod process_tests;
