use super::*;
use ed25519_dalek::{Signer, SigningKey};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};
use switcher_domain::ads::{Campaign, SlotContent};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-ad-runtime-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn sample(now: i64) -> ClockSample {
    ClockSample {
        utc_secs: now,
        continuous_ms: now as u64 * 1000,
        boot_id: "synthetic-boot".into(),
    }
}
fn fixture_clock() -> Result<ClockSample> {
    Ok(sample(1000))
}
fn prefs() -> AdPreferences {
    AdPreferences {
        slots_enabled: [true; 3],
        request_purpose_acknowledged: true,
    }
}
fn signer() -> SigningKey {
    SigningKey::from_bytes(&[0x55; 32])
}

#[test]
fn release_materials_require_complete_valid_fixed_configuration() {
    let public_key = signer()
        .verifying_key()
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let valid = [
        Some("https://ads.example.com/content/"),
        Some(public_key.as_str()),
        Some("synthetic-key"),
        Some("production"),
    ];
    assert!(
        ContentPins::configured_release([None; 4])
            .unwrap()
            .is_none()
    );
    let pins = ContentPins::configured_release(valid).unwrap().unwrap();
    assert!(!pins.local);
    assert_eq!(pins.key, signer().verifying_key());
    assert_eq!(pins.channel, AdChannel::Production);
    for index in 0..4 {
        let mut incomplete = valid;
        incomplete[index] = None;
        assert!(ContentPins::configured_release(incomplete).is_err());
    }
    for (index, value) in [
        (0, "http://ads.example.com/content/"),
        (0, "https://ads.example.com/content/?query=1"),
        (1, "not-a-key"),
        (
            1,
            "0000000000000000000000000000000000000000000000000000000000000000",
        ),
        (2, "key with spaces"),
        (3, "unknown"),
    ] {
        let mut invalid = valid;
        invalid[index] = Some(value);
        assert!(ContentPins::configured_release(invalid).is_err());
    }
}

#[test]
fn formal_public_build_rejects_r2_development_domains() {
    let public_key = signer()
        .verifying_key()
        .to_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    for source in [
        "https://r2.dev/content/",
        "https://bucket.r2.dev/content/",
        "https://BUCKET.R2.DEV./content/",
        "https://nested.bucket.r2.dev./content/",
    ] {
        let pins = ContentPins::configured_release([
            Some(source),
            Some(&public_key),
            Some("synthetic-key"),
            Some("production"),
        ])
        .unwrap();
        assert!(
            validate_public_release_pins(pins).is_err(),
            "accepted {source}"
        );
    }
    let pins = ContentPins::configured_release([
        Some("https://not-r2.dev/content/"),
        Some(&public_key),
        Some("synthetic-key"),
        Some("production"),
    ])
    .unwrap();
    assert!(validate_public_release_pins(pins).is_ok());
}

#[test]
fn release_and_local_trust_selection_respects_edition_and_invalid_configuration() {
    let root = Root::new();
    let directory = local::directory(&root.0);
    storage::create_private_directory(&root.0).unwrap();
    storage::create_private_directory(&root.0.join("v2")).unwrap();
    storage::create_private_directory(&directory).unwrap();
    storage::atomic_json(
        &directory.join("trust.json"),
        &local::LocalTrust {
            schema: 1,
            key_id: "ads-personal-v1".into(),
            public_key: signer().verifying_key().to_bytes(),
        },
    )
    .unwrap();
    let release = || {
        Some(ContentPins {
            source: ContentSource::new("https://ads.example.com/content/").unwrap(),
            key: signer().verifying_key(),
            key_id: "synthetic-key".into(),
            channel: AdChannel::Production,
            local: false,
        })
    };
    for edition in [Edition::Personal, Edition::Public] {
        assert!(
            AdRuntime::select_pins(&root.0, edition, Ok(None))
                .unwrap()
                .unwrap()
                .local
        );
        let result = AdRuntime::select_pins(&root.0, edition, Err("ad_release_materials"));
        if edition == Edition::Personal {
            assert!(result.unwrap().unwrap().local);
        } else {
            assert!(result.is_err());
        }
    }
    assert!(
        AdRuntime::select_pins(&root.0, Edition::Personal, Ok(release()))
            .unwrap()
            .unwrap()
            .local
    );
    assert!(
        !AdRuntime::select_pins(&root.0, Edition::Public, Ok(release()))
            .unwrap()
            .unwrap()
            .local
    );
    storage::atomic_write(&directory.join("trust.json"), b"invalid").unwrap();
    assert!(
        !AdRuntime::select_pins(&root.0, Edition::Public, Ok(release()))
            .unwrap()
            .unwrap()
            .local
    );
    assert!(AdRuntime::select_pins(&root.0, Edition::Personal, Ok(release())).is_err());
    std::fs::remove_file(directory.join("trust.json")).unwrap();
    assert!(
        AdRuntime::select_pins(&root.0, Edition::Personal, Err("ad_release_materials")).is_err()
    );
    assert!(
        !AdRuntime::select_pins(&root.0, Edition::Personal, Ok(release()))
            .unwrap()
            .unwrap()
            .local
    );
}
fn configured_runtime(root: &Root) -> AdRuntime {
    AdRuntime {
        root: root.0.clone(),
        edition: Edition::Public,
        local_error: None,
        sample_clock: fixture_clock,
        pins: Some(ContentPins {
            source: ContentSource::new("https://ads.example.com/content/").unwrap(),
            key: signer().verifying_key(),
            key_id: "synthetic-key".into(),
            channel: AdChannel::Production,
            local: false,
        }),
    }
}

#[test]
fn source_status_distinguishes_selected_missing_and_invalid_configuration() {
    let root = Root::new();
    let mut runtime = configured_runtime(&root);
    assert_eq!(runtime.content_source_configured(), Ok(true));
    runtime.pins = None;
    assert_eq!(runtime.content_source_configured(), Ok(false));
    runtime.local_error = Some("ad_release_materials");
    assert_eq!(
        runtime.content_source_configured(),
        Err("ad_release_materials")
    );
}

fn png() -> Vec<u8> {
    vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2,
        0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 96, 96, 96, 0, 0, 0,
        4, 0, 1, 246, 23, 56, 85, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ]
}
fn manifest(revision: u64) -> AdManifest {
    let bytes = png();
    let asset = ImageAsset {
        sha256: signed::digest(&bytes),
        mime: "image/png".into(),
        bytes: bytes.len() as u32,
        width: 1,
        height: 1,
    };
    AdManifest {
        schema_version: 1,
        channel: AdChannel::Production,
        content_revision: revision,
        key_id: "synthetic-key".into(),
        issued_at: 1000,
        expires_at: 200000,
        release_id: format!("release-{revision}"),
        slots: AdSlot::ALL
            .into_iter()
            .enumerate()
            .map(|(index, slot)| SlotContent {
                slot,
                enabled: true,
                campaigns: vec![Campaign {
                    campaign_id: format!("campaign-{index}"),
                    merchant_id: "merchant-one".into(),
                    merchant_name: "合成商家".into(),
                    title: "合成标题".into(),
                    description: "合成测试".into(),
                    image: asset.clone(),
                    button_text: "详情".into(),
                    target_url: "https://merchant.example.com/offer".into(),
                    starts_at: 1000,
                    ends_at: 190000,
                    enabled: true,
                }],
            })
            .collect(),
    }
}
fn wire(value: &AdManifest) -> Vec<u8> {
    wire_with(value, &signer())
}
fn wire_with(value: &AdManifest, key: &SigningKey) -> Vec<u8> {
    let payload = serde_json::to_string(value).unwrap();
    serde_json::to_vec(&signed::Envelope {
        signature: key.sign(payload.as_bytes()).to_bytes().to_vec(),
        payload,
    })
    .unwrap()
}
#[derive(Default)]
struct MemoryFetch {
    objects: HashMap<String, (Vec<u8>, String)>,
    requests: Vec<String>,
}
impl MemoryFetch {
    fn set(&mut self, runtime: &AdRuntime, value: &AdManifest) {
        self.set_signed(runtime, value, &signer());
    }
    fn set_signed(&mut self, runtime: &AdRuntime, value: &AdManifest, key: &SigningKey) {
        let pins = runtime.pins.as_ref().unwrap();
        self.objects.insert(
            pins.source
                .manifest_request(&prefs(), pins.channel)
                .unwrap()
                .to_string(),
            (wire_with(value, key), "application/json".into()),
        );
        for slot in &value.slots {
            for campaign in &slot.campaigns {
                let url = pins
                    .source
                    .asset_request(slot.slot, &campaign.image, &prefs(), pins.channel)
                    .unwrap()
                    .unwrap();
                self.objects
                    .insert(url.to_string(), (png(), "image/png".into()));
            }
        }
    }
}
impl Fetcher for MemoryFetch {
    fn get(&mut self, url: &Url, limit: usize) -> Result<Download> {
        self.requests.push(url.to_string());
        let (bytes, mime) = self.objects.get(url.as_str()).ok_or("synthetic_missing")?;
        if bytes.len() > limit {
            return Err("ad_download_size");
        }
        Ok(Download {
            bytes: bytes.clone(),
            mime: Some(mime.clone()),
            final_url: url.clone(),
        })
    }
}
fn refresh(runtime: &AdRuntime, fetcher: &mut MemoryFetch, now: i64) -> Result<RefreshResult> {
    let _lock = runtime.lock()?;
    runtime.refresh_locked(&prefs(), true, sample(now), Some(fetcher), Some(17))
}

#[test]
fn different_release_source_does_not_inherit_old_refresh_deadline() {
    let root = Root::new();
    let old = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&old, &manifest(99));
    refresh(&old, &mut fetcher, 1000).unwrap();
    let mut next = configured_runtime(&root);
    next.pins.as_mut().unwrap().source =
        ContentSource::new("https://new.example.com/content/").unwrap();
    fetcher.set(&next, &manifest(1));
    let _lock = next.lock().unwrap();
    let result = next
        .refresh_locked(&prefs(), false, sample(1001), Some(&mut fetcher), Some(0))
        .unwrap();
    assert!(result.refreshed, "{}", result.reason);
    assert_eq!(next.load(&sample(1001)).unwrap().0.sequence.highest, 1);
}

#[test]
fn different_release_source_does_not_inherit_old_revision() {
    let root = Root::new();
    let old = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&old, &manifest(99));
    refresh(&old, &mut fetcher, 1000).unwrap();
    let mut next = configured_runtime(&root);
    next.pins.as_mut().unwrap().source =
        ContentSource::new("https://new.example.com/content/").unwrap();
    fetcher.set(&next, &manifest(1));
    assert!(refresh(&next, &mut fetcher, 1040).unwrap().refreshed);
    let _lock = old.lock().unwrap();
    assert_eq!(old.load(&sample(1040)).unwrap().0.sequence.highest, 99);
}

#[test]
fn local_high_revision_does_not_delay_or_block_first_release_refresh() {
    let root = Root::new();
    let mut local_runtime = configured_runtime(&root);
    local_runtime.pins.as_mut().unwrap().local = true;
    let local_dir = local::directory(&root.0);
    storage::create_private_directory(&root.0).unwrap();
    storage::create_private_directory(&root.0.join("v2")).unwrap();
    storage::create_private_directory(&local_dir).unwrap();
    storage::create_private_directory(&local_dir.join("production")).unwrap();
    storage::atomic_write(&local_dir.join("production/current.json"), b"local-fixture").unwrap();
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&local_runtime, &manifest(99));
    refresh(&local_runtime, &mut fetcher, 1000).unwrap();
    let release = configured_runtime(&root);
    fetcher.set(&release, &manifest(1));
    let _lock = release.lock().unwrap();
    assert!(
        release
            .refresh_locked(&prefs(), false, sample(1001), Some(&mut fetcher), Some(0))
            .unwrap()
            .refreshed
    );
    assert_eq!(release.load(&sample(1001)).unwrap().0.sequence.highest, 1);
    assert_eq!(
        local_runtime
            .load(&sample(1001))
            .unwrap()
            .0
            .sequence
            .highest,
        99
    );
}

#[test]
fn every_trust_identity_component_has_a_separate_state_and_restores_on_return() {
    let root = Root::new();
    let original = configured_runtime(&root);
    let original_path = original.state_path().unwrap();
    let mut paths = Vec::new();
    for index in 0..5 {
        let mut other = configured_runtime(&root);
        let pins = other.pins.as_mut().unwrap();
        match index {
            0 => pins.source = ContentSource::new("https://other.example.com/content/").unwrap(),
            1 => pins.key = SigningKey::from_bytes(&[0x66; 32]).verifying_key(),
            2 => pins.key_id = "other-key".into(),
            3 => pins.channel = AdChannel::Preview,
            _ => pins.local = true,
        }
        let path = other.state_path().unwrap();
        assert_ne!(original_path, path);
        assert!(!paths.contains(&path));
        paths.push(path);
    }
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&original, &manifest(99));
    refresh(&original, &mut fetcher, 1000).unwrap();
    let restarted = configured_runtime(&root);
    let _lock = restarted.lock().unwrap();
    assert_eq!(
        restarted.load(&sample(1001)).unwrap().0.sequence.highest,
        99
    );
}

#[test]
fn source_key_and_channel_changes_restore_their_own_verified_watermarks() {
    for change in 0..3 {
        let root = Root::new();
        let original = configured_runtime(&root);
        let mut fetcher = MemoryFetch::default();
        fetcher.set(&original, &manifest(99));
        assert!(refresh(&original, &mut fetcher, 1000).unwrap().refreshed);
        let mut other = configured_runtime(&root);
        let other_key = SigningKey::from_bytes(&[0x66; 32]);
        let pins = other.pins.as_mut().unwrap();
        match change {
            0 => pins.source = ContentSource::new("https://other.example.com/content/").unwrap(),
            1 => pins.key = other_key.verifying_key(),
            _ => pins.channel = AdChannel::Preview,
        }
        let mut next = manifest(1);
        next.channel = pins.channel;
        let original_key = signer();
        fetcher.set_signed(
            &other,
            &next,
            if change == 1 {
                &other_key
            } else {
                &original_key
            },
        );
        assert!(refresh(&other, &mut fetcher, 1040).unwrap().refreshed);
        let _lock = original.lock().unwrap();
        let a = original.load(&sample(1041)).unwrap().0;
        let b = other.load(&sample(1041)).unwrap().0;
        assert_eq!(a.sequence.highest, 99);
        assert_eq!(a.cache.highest_revision, 99);
        assert_eq!(b.sequence.highest, 1);
        assert_eq!(b.cache.highest_revision, 1);
        assert!(a.cache.cached_wire.is_some());
        assert!(b.cache.cached_wire.is_some());
    }
}

#[test]
fn legacy_local_cache_is_verified_before_migration_and_original_is_preserved() {
    let root = Root::new();
    let mut runtime = configured_runtime(&root);
    let pins = runtime.pins.as_mut().unwrap();
    pins.local = true;
    pins.key_id = "ads-personal-v1".into();
    let _lock = runtime.lock().unwrap();
    let mut m = manifest(99);
    m.key_id = "ads-personal-v1".into();
    let bytes = wire(&m);
    let (_, hash): (AdManifest, _) =
        signed::verify(&bytes, &signer().verifying_key(), ads::MAX_MANIFEST_BYTES).unwrap();
    let mut state = Persistent::new();
    state
        .cache
        .accept(
            &bytes,
            &signer().verifying_key(),
            AdChannel::Production,
            "ads-personal-v1",
            1000,
            true,
        )
        .unwrap();
    state.sequence.remember(99, hash).unwrap();
    let legacy = runtime.ads_directory().join("state.json");
    storage::atomic_json(&legacy, &state).unwrap();
    let original = std::fs::read(&legacy).unwrap();
    let (migrated, clock) = runtime.load(&sample(1001)).unwrap();
    assert_eq!(migrated.sequence.highest, 99);
    runtime.save(&migrated, &clock).unwrap();
    assert_eq!(std::fs::read(&legacy).unwrap(), original);
    assert_ne!(runtime.state_path().unwrap(), legacy);
    assert_eq!(runtime.load(&sample(1002)).unwrap().0.sequence.highest, 99);
}

#[test]
fn unverified_legacy_local_watermark_is_preserved_and_rejected() {
    let root = Root::new();
    let mut runtime = configured_runtime(&root);
    runtime.pins.as_mut().unwrap().local = true;
    let _lock = runtime.lock().unwrap();
    let mut state = Persistent::new();
    state.sequence.highest = 99;
    state.sequence.payload_hash = Some("synthetic-hash".into());
    let legacy = runtime.ads_directory().join("state.json");
    storage::atomic_json(&legacy, &state).unwrap();
    let original = std::fs::read(&legacy).unwrap();
    assert!(matches!(
        runtime.load(&sample(1000)),
        Err("ad_legacy_state_unverified")
    ));
    assert_eq!(std::fs::read(&legacy).unwrap(), original);
    assert!(!runtime.state_path().unwrap().exists());
}

#[test]
fn legacy_local_floor_survives_key_rotation_and_failed_asset_refresh() {
    for rotated in [false, true] {
        let root = Root::new();
        let mut runtime = configured_runtime(&root);
        let pins = runtime.pins.as_mut().unwrap();
        pins.local = true;
        if rotated {
            pins.key = SigningKey::from_bytes(&[0x66; 32]).verifying_key();
        }
        let _lock = runtime.lock().unwrap();
        let mut state = Persistent::new();
        let cached = wire(&manifest(if rotated { 99 } else { 1 }));
        state
            .cache
            .accept(
                &cached,
                &signer().verifying_key(),
                AdChannel::Production,
                "synthetic-key",
                1000,
                true,
            )
            .unwrap();
        let (_, hash): (AdManifest, String) = signed::verify(
            &wire(&manifest(99)),
            &signer().verifying_key(),
            ads::MAX_MANIFEST_BYTES,
        )
        .unwrap();
        state.sequence.remember(99, hash.clone()).unwrap();
        let legacy = runtime.ads_directory().join("state.json");
        storage::atomic_json(&legacy, &state).unwrap();
        let original = std::fs::read(&legacy).unwrap();

        let (migrated, clock) = runtime.load(&sample(1001)).unwrap();
        assert_eq!(migrated.sequence.highest, 99);
        assert_eq!(
            migrated.sequence.payload_hash.as_deref(),
            Some(hash.as_str())
        );
        assert!(migrated.cache.manifest().is_none());
        assert_eq!(migrated.sequence.check(98, &hash), Err("old_sequence"));
        assert_eq!(
            migrated.sequence.check(99, "different"),
            Err("sequence_conflict")
        );
        assert!(migrated.sequence.check(100, "new-signed-payload").is_ok());
        runtime.save(&migrated, &clock).unwrap();
        assert_eq!(std::fs::read(&legacy).unwrap(), original);
        assert_eq!(runtime.load(&sample(1002)).unwrap().0.sequence.highest, 99);

        let next_key = if rotated {
            SigningKey::from_bytes(&[0x66; 32])
        } else {
            signer()
        };
        let current = wire_with(&manifest(100), &next_key);
        let local_root = local::directory(&root.0);
        storage::create_private_directory(&local_root).unwrap();
        storage::create_private_directory(&local_root.join("production")).unwrap();
        storage::atomic_write(&local_root.join("production/current.json"), &current).unwrap();
        let mut fetcher = MemoryFetch::default();
        fetcher.set_signed(&runtime, &manifest(100), &next_key);
        assert!(
            runtime
                .refresh_locked(&prefs(), true, sample(1003), Some(&mut fetcher), Some(0))
                .unwrap()
                .refreshed
        );
        let updated = runtime.load(&sample(1004)).unwrap().0;
        assert_eq!(updated.sequence.highest, 100);
        assert_eq!(updated.cache.highest_revision, 100);
        assert_eq!(std::fs::read(&legacy).unwrap(), original);
    }
}

#[test]
fn legacy_cache_cannot_authenticate_a_higher_failed_asset_watermark() {
    let root = Root::new();
    let mut runtime = configured_runtime(&root);
    runtime.pins.as_mut().unwrap().local = true;
    let _lock = runtime.lock().unwrap();
    let mut state = Persistent::new();
    state
        .cache
        .accept(
            &wire(&manifest(1)),
            &signer().verifying_key(),
            AdChannel::Production,
            "synthetic-key",
            1000,
            true,
        )
        .unwrap();
    state
        .sequence
        .remember(99, "unavailable-higher-payload".into())
        .unwrap();
    let legacy = runtime.ads_directory().join("state.json");
    storage::atomic_json(&legacy, &state).unwrap();
    let original = std::fs::read(&legacy).unwrap();
    assert!(matches!(
        runtime.load(&sample(1001)),
        Err("ad_legacy_state_unverified")
    ));
    assert_eq!(std::fs::read(&legacy).unwrap(), original);
}

#[test]
fn legacy_cache_signed_by_old_local_key_is_not_attributed_to_new_key() {
    let root = Root::new();
    let mut runtime = configured_runtime(&root);
    let pins = runtime.pins.as_mut().unwrap();
    pins.local = true;
    pins.key = SigningKey::from_bytes(&[0x66; 32]).verifying_key();
    let _lock = runtime.lock().unwrap();
    let bytes = wire(&manifest(99));
    let (_, hash): (AdManifest, _) =
        signed::verify(&bytes, &signer().verifying_key(), ads::MAX_MANIFEST_BYTES).unwrap();
    let mut state = Persistent::new();
    state
        .cache
        .accept(
            &bytes,
            &signer().verifying_key(),
            AdChannel::Production,
            "synthetic-key",
            1000,
            true,
        )
        .unwrap();
    state.sequence.remember(99, hash).unwrap();
    let legacy = runtime.ads_directory().join("state.json");
    storage::atomic_json(&legacy, &state).unwrap();
    let (migrated, _) = runtime.load(&sample(1001)).unwrap();
    assert_eq!(migrated.sequence.highest, 99);
    assert!(migrated.cache.cached_wire.is_none());
    assert!(legacy.exists());
}

#[test]
fn missing_pins_and_all_off_make_zero_requests() {
    let root = Root::new();
    let runtime = AdRuntime::new(root.0.clone(), Edition::Public).with_clock(fixture_clock);
    let mut fetcher = MemoryFetch::default();
    let _lock = runtime.lock().unwrap();
    assert_eq!(
        runtime
            .refresh_locked(&prefs(), true, sample(1000), Some(&mut fetcher), Some(0))
            .unwrap()
            .reason,
        "wait_ad_content_materials"
    );
    assert_eq!(
        runtime
            .refresh_locked(
                &AdPreferences::default(),
                true,
                sample(1000),
                Some(&mut fetcher),
                Some(0)
            )
            .unwrap()
            .reason,
        "ads_disabled"
    );
    assert!(fetcher.requests.is_empty());
}

#[test]
fn configured_pins_with_all_slots_off_make_zero_requests() {
    let root = Root::new();
    let runtime = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&runtime, &manifest(1));
    let preferences = AdPreferences {
        slots_enabled: [false; 3],
        request_purpose_acknowledged: true,
    };
    let _lock = runtime.lock().unwrap();
    let result = runtime
        .refresh_locked(
            &preferences,
            true,
            sample(1000),
            Some(&mut fetcher),
            Some(0),
        )
        .unwrap();
    assert_eq!(result.reason, "ads_disabled");
    assert!(runtime.pins.is_some());
    assert!(!fetcher.objects.is_empty());
    assert!(fetcher.requests.is_empty());
    assert_eq!(runtime.load(&sample(1000)).unwrap().0.sequence.highest, 0);
}

#[test]
fn unacknowledged_enabled_slots_never_request_content() {
    for edition in [Edition::Personal, Edition::Public] {
        for slot in 0..3 {
            for manual in [false, true] {
                let root = Root::new();
                let mut runtime = configured_runtime(&root);
                runtime.edition = edition;
                let mut fetcher = MemoryFetch::default();
                fetcher.set(&runtime, &manifest(7));
                let mut preferences = AdPreferences::default();
                preferences.slots_enabled[slot] = true;
                let _lock = runtime.lock().unwrap();
                let (state, clock) = runtime.load(&sample(1000)).unwrap();
                runtime.save(&state, &clock).unwrap();
                let state_path = runtime.state_path().unwrap();
                let clock_path = runtime.directory().join("clock.json");
                let before = (
                    passive_file_snapshot(&state_path),
                    passive_file_snapshot(&clock_path),
                );
                let result = runtime
                    .refresh_locked(
                        &preferences,
                        manual,
                        sample(1001),
                        Some(&mut fetcher),
                        Some(17),
                    )
                    .unwrap();
                assert!(!result.refreshed);
                assert_eq!(result.reason, "ads_disabled");
                assert!(fetcher.requests.is_empty());
                assert_eq!(passive_file_snapshot(&state_path), before.0);
                assert_eq!(passive_file_snapshot(&clock_path), before.1);
            }
        }
    }
}

#[test]
fn disabled_cached_ads_stay_request_free_after_due_expiry_and_restart() {
    fn expired_clock() -> Result<ClockSample> {
        Ok(sample(200001))
    }
    for edition in [Edition::Personal, Edition::Public] {
        let root = Root::new();
        let mut runtime = configured_runtime(&root);
        runtime.edition = edition;
        let mut fetcher = MemoryFetch::default();
        fetcher.set(&runtime, &manifest(7));
        assert!(refresh(&runtime, &mut fetcher, 1000).unwrap().refreshed);
        assert_eq!(
            fetcher.requests.len(),
            2,
            "manifest plus deduplicated asset"
        );
        let requests = fetcher.requests.clone();
        let (baseline, _) = runtime.load(&sample(1000)).unwrap();
        let watermark = serde_json::to_value(&baseline.sequence).unwrap();
        let preferences = AdPreferences {
            slots_enabled: [false; 3],
            request_purpose_acknowledged: true,
        };
        assert!(runtime.snapshot(&prefs()).unwrap().is_some());
        assert!(runtime.snapshot(&preferences).unwrap().is_none());
        assert!(
            runtime
                .image_bytes(AdSlot::ResidentialBroadband, &preferences)
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            runtime.click_locked(
                AdSlot::ResidentialBroadband,
                true,
                &preferences,
                &sample(1000)
            ),
            Err("ad_not_current")
        ));
        // Immediate manual refresh, scheduled refresh, 24h cache expiry and
        // signed-manifest expiry must all preserve the disabled request gate.
        for restart in [false, true] {
            if restart {
                runtime = configured_runtime(&root);
                runtime.edition = edition;
            }
            for now in [1001, 1917, 87401, 200001] {
                for manual in [false, true] {
                    let _lock = runtime.lock().unwrap();
                    let result = runtime
                        .refresh_locked(
                            &preferences,
                            manual,
                            sample(now),
                            Some(&mut fetcher),
                            Some(17),
                        )
                        .unwrap();
                    assert!(!result.refreshed);
                    assert_eq!(result.reason, "ads_disabled");
                    assert_eq!(fetcher.requests, requests);
                    let (state, clock) = runtime.load(&sample(now)).unwrap();
                    assert!(
                        runtime
                            .visible(&state, &clock, &preferences, now as u64)
                            .is_none()
                    );
                    assert_eq!(serde_json::to_value(&state.sequence).unwrap(), watermark);
                    assert_eq!(state.cache.highest_revision, 7);
                    assert_eq!(clock.last_success_utc, Some(1000));
                }
            }
        }
        runtime.sample_clock = expired_clock;
        assert!(runtime.snapshot(&preferences).unwrap().is_none());
        assert!(
            runtime
                .image_bytes(AdSlot::ResidentialBroadband, &preferences)
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            runtime.click_locked(
                AdSlot::ResidentialBroadband,
                true,
                &preferences,
                &sample(200001)
            ),
            Err("ad_not_current")
        ));
        assert_eq!(fetcher.requests, requests);
        let (state, _) = runtime.load(&sample(200001)).unwrap();
        assert_eq!(serde_json::to_value(&state.sequence).unwrap(), watermark);
        assert_eq!(state.cache.highest_revision, 7);
    }
}

#[test]
fn successful_refresh_persists_deadline_filters_slots_and_confirms_clicks() {
    let root = Root::new();
    let runtime = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    let mut value = manifest(7);
    value.slots[1].campaigns[0].image.sha256 = "c".repeat(64);
    value.slots[2].campaigns[0].image.sha256 = "d".repeat(64);
    fetcher.set(&runtime, &value);
    let mut preferences = prefs();
    preferences.slots_enabled = [true, false, false];
    {
        let _lock = runtime.lock().unwrap();
        assert_eq!(
            runtime
                .refresh_locked(
                    &preferences,
                    true,
                    sample(1000),
                    Some(&mut fetcher),
                    Some(17)
                )
                .unwrap()
                .next_check_at,
            Some(1917)
        );
    }
    assert_eq!(fetcher.requests.len(), 2);
    let (state, clock) = runtime.load(&sample(1001)).unwrap();
    let visible = runtime.visible(&state, &clock, &preferences, 1001).unwrap();
    assert_eq!(
        visible
            .slots
            .iter()
            .map(|slot| slot.campaigns.len())
            .collect::<Vec<_>>(),
        vec![1, 0, 0]
    );
    assert_eq!(clock.last_success_utc, Some(1000));
    assert!(matches!(
        runtime
            .click_locked(
                AdSlot::ResidentialBroadband,
                true,
                &preferences,
                &sample(1001)
            )
            .unwrap(),
        ClickDecision::NeedsConfirmation { .. }
    ));
    assert!(matches!(
        runtime
            .click_locked(
                AdSlot::ResidentialBroadband,
                true,
                &preferences,
                &sample(1002)
            )
            .unwrap(),
        ClickDecision::Ready { .. }
    ));
    let before = fetcher.requests.len();
    assert!(!refresh(&runtime, &mut fetcher, 1003).unwrap().refreshed);
    assert_eq!(fetcher.requests.len(), before);
    let reloaded = configured_runtime(&root);
    let (state, _) = reloaded.load(&sample(1003)).unwrap();
    assert_eq!(state.schedule.next_check_at, Some(1917));
    assert_eq!(state.sequence.highest, 7);
}

#[test]
fn scheduled_refresh_fetches_new_signed_content_and_survives_restart() {
    let root = Root::new();
    let runtime = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&runtime, &manifest(7));
    assert_eq!(
        refresh(&runtime, &mut fetcher, 1000).unwrap().next_check_at,
        Some(1917)
    );

    let mut next = manifest(8);
    next.slots[0].campaigns[0].title = "新版合成标题".into();
    fetcher.set(&runtime, &next);
    let before = fetcher.requests.len();
    {
        let _lock = runtime.lock().unwrap();
        let early = runtime
            .refresh_locked(&prefs(), false, sample(1916), Some(&mut fetcher), Some(17))
            .unwrap();
        assert!(!early.refreshed);
        assert_eq!(early.reason, "ad_refresh_not_due_or_merged");
        assert_eq!(fetcher.requests.len(), before);
        let due = runtime
            .refresh_locked(&prefs(), false, sample(1917), Some(&mut fetcher), Some(17))
            .unwrap();
        assert!(due.refreshed);
        assert_eq!(due.next_check_at, Some(2834));
    }
    // The asset is already verified and cached; only the new manifest is fetched.
    assert_eq!(fetcher.requests.len(), before + 1);
    let restarted = configured_runtime(&root);
    let (state, clock) = restarted.load(&sample(1918)).unwrap();
    assert_eq!(state.sequence.highest, 8);
    assert_eq!(state.cache.highest_revision, 8);
    assert_eq!(state.schedule.next_check_at, Some(2834));
    let visible = restarted.visible(&state, &clock, &prefs(), 1918).unwrap();
    assert_eq!(visible.slots[0].campaigns[0].title, "新版合成标题");
}

#[test]
fn background_refresh_failure_remains_visible_until_verified_recovery() {
    static NOW: AtomicU64 = AtomicU64::new(1000);
    fn current_clock() -> Result<ClockSample> {
        Ok(sample(NOW.load(Ordering::SeqCst) as i64))
    }

    for edition in [Edition::Personal, Edition::Public] {
        for bad_signature in [false, true] {
            NOW.store(1000, Ordering::SeqCst);
            let root = Root::new();
            let mut runtime = configured_runtime(&root);
            runtime.edition = edition;
            runtime.sample_clock = current_clock;
            let mut fetcher = MemoryFetch::default();
            fetcher.set(&runtime, &manifest(7));
            assert!(refresh(&runtime, &mut fetcher, 1000).unwrap().refreshed);
            let initial = runtime.snapshot_with_status(&prefs()).unwrap();
            assert!(!initial.refresh_failed);
            assert_eq!(initial.manifest.unwrap().content_revision, 7);

            if bad_signature {
                fetcher.set_signed(&runtime, &manifest(8), &SigningKey::from_bytes(&[0x66; 32]));
            } else {
                let url = runtime
                    .pins
                    .as_ref()
                    .unwrap()
                    .source
                    .manifest_request(&prefs(), AdChannel::Production)
                    .unwrap();
                fetcher.objects.remove(url.as_str());
            }
            NOW.store(1917, Ordering::SeqCst);
            {
                let _lock = runtime.lock().unwrap();
                assert!(
                    runtime
                        .refresh_locked(&prefs(), false, sample(1917), Some(&mut fetcher), Some(0))
                        .is_err()
                );
            }

            // A new runtime models an app restart and reads the persisted failure.
            let mut restarted = configured_runtime(&root);
            restarted.edition = edition;
            restarted.sample_clock = current_clock;
            let failed = restarted.snapshot_with_status(&prefs()).unwrap();
            assert!(failed.refresh_failed);
            assert_eq!(failed.manifest.unwrap().content_revision, 7);
            let disabled = restarted
                .snapshot_with_status(&AdPreferences::default())
                .unwrap();
            assert!(!disabled.refresh_failed);
            assert!(disabled.manifest.is_none());

            NOW.store(1977, Ordering::SeqCst);
            fetcher.set(&restarted, &manifest(8));
            {
                let _lock = restarted.lock().unwrap();
                assert!(
                    restarted
                        .refresh_locked(&prefs(), false, sample(1977), Some(&mut fetcher), Some(0))
                        .unwrap()
                        .refreshed
                );
            }
            let recovered = restarted.snapshot_with_status(&prefs()).unwrap();
            assert!(!recovered.refresh_failed);
            assert_eq!(recovered.manifest.unwrap().content_revision, 8);
        }
    }
}

#[test]
fn scheduled_failures_back_off_across_restarts_until_verified_recovery() {
    let root = Root::new();
    let runtime = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&runtime, &manifest(7));
    assert!(refresh(&runtime, &mut fetcher, 1000).unwrap().refreshed);
    let manifest_url = runtime
        .pins
        .as_ref()
        .unwrap()
        .source
        .manifest_request(&prefs(), AdChannel::Production)
        .unwrap();
    fetcher.objects.remove(manifest_url.as_str());

    let mut due = 1917;
    for (index, delay) in [60, 300, 900, 1800, 3600, 3600].into_iter().enumerate() {
        let restarted = configured_runtime(&root);
        let _lock = restarted.lock().unwrap();
        let before = fetcher.requests.len();
        assert_eq!(
            restarted
                .refresh_locked(&prefs(), false, sample(due), Some(&mut fetcher), Some(0))
                .unwrap_err(),
            "synthetic_missing"
        );
        assert_eq!(fetcher.requests.len(), before + 1);
        let (state, clock) = restarted.load(&sample(due)).unwrap();
        assert_eq!(state.schedule.failures, (index + 1) as u8);
        assert_eq!(state.schedule.next_check_at, Some((due + delay) as u64));
        assert_eq!(state.cache.highest_revision, 7);
        assert_eq!(state.cache.last_success, Some(1000));
        assert_eq!(clock.last_success_utc, Some(1000));
        assert!(
            restarted
                .visible(&state, &clock, &prefs(), due as u64)
                .is_some()
        );

        let early = restarted
            .refresh_locked(
                &prefs(),
                false,
                sample(due + delay - 1),
                Some(&mut fetcher),
                Some(0),
            )
            .unwrap();
        assert!(!early.refreshed);
        assert_eq!(early.reason, "ad_refresh_not_due_or_merged");
        assert_eq!(fetcher.requests.len(), before + 1);
        due += delay;
    }

    let restarted = configured_runtime(&root);
    fetcher.set(&restarted, &manifest(8));
    let _lock = restarted.lock().unwrap();
    let result = restarted
        .refresh_locked(&prefs(), false, sample(due), Some(&mut fetcher), Some(60))
        .unwrap();
    assert!(result.refreshed);
    assert_eq!(result.next_check_at, Some((due + 960) as u64));
    let (state, clock) = restarted.load(&sample(due)).unwrap();
    assert_eq!(state.schedule.failures, 0);
    assert_eq!(state.cache.highest_revision, 8);
    assert_eq!(state.cache.last_success, Some(due as u64));
    assert_eq!(clock.last_success_utc, Some(due));
}

fn rejected_tls_peer(delay: Duration) -> (Url, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (connection, _) = listener.accept().unwrap();
        std::thread::sleep(delay);
        drop(connection);
    });
    (
        Url::parse(&format!("https://{address}/content/")).unwrap(),
        server,
    )
}

#[test]
fn failed_slow_request_schedules_backoff_from_failure_time() {
    static CLOCK_READS: AtomicU64 = AtomicU64::new(0);
    fn elapsed_clock() -> Result<ClockSample> {
        let call = CLOCK_READS.fetch_add(1, Ordering::SeqCst);
        Ok(sample(if call == 0 { 1000 } else { 1120 }))
    }

    CLOCK_READS.store(0, Ordering::SeqCst);
    let (source, server) = rejected_tls_peer(Duration::ZERO);
    let root = Root::new();
    let mut runtime = configured_runtime(&root);
    runtime.sample_clock = elapsed_clock;
    runtime.pins.as_mut().unwrap().source = ContentSource::fixture(source);

    assert_eq!(
        runtime.refresh_if_due(&prefs(), false).unwrap_err(),
        "ad_network"
    );
    server.join().unwrap();
    let (state, clock) = runtime.load(&sample(1120)).unwrap();
    assert_eq!(state.schedule.failures, 1);
    assert_eq!(state.schedule.next_check_at, Some(1180));
    assert_eq!(clock.last_success_utc, None);
}

#[test]
fn failed_request_uses_elapsed_time_when_final_clock_is_unavailable() {
    static CLOCK_READS: AtomicU64 = AtomicU64::new(0);
    fn unavailable_final_clock() -> Result<ClockSample> {
        if CLOCK_READS.fetch_add(1, Ordering::SeqCst) == 0 {
            Ok(sample(1000))
        } else {
            Err("synthetic_clock_unavailable")
        }
    }

    CLOCK_READS.store(0, Ordering::SeqCst);
    let (source, server) = rejected_tls_peer(Duration::from_millis(1100));
    let root = Root::new();
    let mut runtime = configured_runtime(&root);
    runtime.sample_clock = unavailable_final_clock;
    runtime.pins.as_mut().unwrap().source = ContentSource::fixture(source);

    assert_eq!(
        runtime.refresh_if_due(&prefs(), false).unwrap_err(),
        "ad_network"
    );
    server.join().unwrap();
    let (state, clock) = runtime.load(&sample(1002)).unwrap();
    assert_eq!(state.schedule.failures, 1);
    assert!(state.schedule.next_check_at.unwrap() >= 1061);
    assert_eq!(clock.last_success_utc, None);
}

#[test]
fn restarted_runtime_rejects_different_signed_content_with_same_revision() {
    let root = Root::new();
    let runtime = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&runtime, &manifest(7));
    refresh(&runtime, &mut fetcher, 1000).unwrap();
    let restarted = configured_runtime(&root);
    let mut conflicting = manifest(7);
    conflicting.slots[0].campaigns[0].title = "冲突的合成标题".into();
    fetcher.set(&restarted, &conflicting);
    {
        let _lock = restarted.lock().unwrap();
        assert_eq!(
            restarted
                .refresh_locked(&prefs(), false, sample(1917), Some(&mut fetcher), Some(17))
                .unwrap_err(),
            "sequence_conflict"
        );
    }
    let restarted_again = configured_runtime(&root);
    let (state, clock) = restarted_again.load(&sample(1918)).unwrap();
    assert_eq!(state.sequence.highest, 7);
    assert_eq!(state.cache.highest_revision, 7);
    let visible = restarted_again
        .visible(&state, &clock, &prefs(), 1918)
        .unwrap();
    assert_eq!(visible.slots[0].campaigns[0].title, "合成标题");
}

#[test]
fn failed_asset_keeps_authenticated_watermark_and_hides_old_content() {
    let root = Root::new();
    let runtime = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&runtime, &manifest(7));
    refresh(&runtime, &mut fetcher, 1000).unwrap();
    let mut higher = manifest(8);
    higher.slots[0].campaigns[0].image.sha256 = "b".repeat(64);
    fetcher.set(&runtime, &higher);
    assert!(refresh(&runtime, &mut fetcher, 1031).is_err());
    let (state, clock) = runtime.load(&sample(1031)).unwrap();
    assert_eq!(state.sequence.highest, 8);
    assert_eq!(state.cache.highest_revision, 7);
    assert_eq!(clock.last_success_utc, Some(1000));
    assert!(runtime.visible(&state, &clock, &prefs(), 1031).is_none());
    fetcher.set(&runtime, &manifest(7));
    assert!(refresh(&runtime, &mut fetcher, 1062).is_err());
    assert_eq!(runtime.load(&sample(1062)).unwrap().0.sequence.highest, 8);
}

#[test]
fn clock_jump_hides_content_and_baseline_needs_signed_refresh() {
    let root = Root::new();
    let runtime = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&runtime, &manifest(5));
    refresh(&runtime, &mut fetcher, 1000).unwrap();
    let mut jumped = sample(2000);
    jumped.continuous_ms = 1001000;
    let (state, mut clock) = runtime.load(&jumped).unwrap();
    assert!(!clock.trusted());
    assert!(runtime.visible(&state, &clock, &prefs(), 2000).is_none());
    runtime.save(&state, &clock).unwrap();
    let count = fetcher.requests.len();
    assert_eq!(
        runtime
            .refresh_locked(&prefs(), true, jumped.clone(), Some(&mut fetcher), Some(0))
            .unwrap()
            .reason,
        "clock_recovery_required"
    );
    assert_eq!(fetcher.requests.len(), count);
    clock.establish_baseline(sample(2000));
    runtime.save(&state, &clock).unwrap();
    assert!(runtime.visible(&state, &clock, &prefs(), 2000).is_none());
    refresh(&runtime, &mut fetcher, 2000).unwrap();
    let (state, clock) = runtime.load(&sample(2001)).unwrap();
    assert!(clock.trusted());
    assert_eq!(state.sequence.highest, 5);
    assert!(runtime.visible(&state, &clock, &prefs(), 2001).is_some());
    assert!(runtime.directory().join("clock.json").exists());
}

#[test]
fn malformed_state_is_not_reset_and_offline_content_expires() {
    let root = Root::new();
    let runtime = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&runtime, &manifest(1));
    refresh(&runtime, &mut fetcher, 1000).unwrap();
    let (state, clock) = runtime.load(&sample(1000)).unwrap();
    assert!(runtime.visible(&state, &clock, &prefs(), 87400).is_none());
    storage::atomic_write(
        &runtime.state_path().unwrap(),
        b"{\"schema\":1,\"schema\":1}",
    )
    .unwrap();
    assert!(runtime.load(&sample(1001)).is_err());
}

fn response_once(response: &'static [u8]) -> (Url, std::thread::JoinHandle<String>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut buffer = [0u8; 4096];
        let count = stream.read(&mut buffer).unwrap();
        stream.write_all(response).unwrap();
        String::from_utf8_lossy(&buffer[..count]).into_owned()
    });
    (
        Url::parse(&format!("http://{address}/content/")).unwrap(),
        handle,
    )
}

#[test]
fn transport_bounds_body_rejects_redirect_and_sends_no_identity() {
    // HTTP is enabled only for this private loopback transport harness.
    // The production constructor always uses https_only(true).
    let mut fetcher = HttpsFetcher {
        client: reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
    };
    let (base, handle) = response_once(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
    let source = ContentSource::fixture(base);
    let request = source
        .manifest_request(&prefs(), AdChannel::Production)
        .unwrap();
    assert_eq!(fetcher.get(&request, 2).unwrap().bytes, b"{}");
    let received = handle.join().unwrap().to_ascii_lowercase();
    assert!(received.starts_with("get /content/production/current.json http/1.1"));
    for forbidden in [
        "authorization:",
        "cookie:",
        "referer:",
        "user-agent:",
        "?",
        "account",
        "device",
    ] {
        assert!(!received.contains(forbidden), "{forbidden}");
    }
    for response in [
        b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/never\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice(),
        b"HTTP/1.1 200 OK\r\nContent-Length: 99\r\nConnection: close\r\n\r\noversize".as_slice(),
        b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\noversize".as_slice(),
    ] {
        let (url, handle) = response_once(response);
        assert!(fetcher.get(&url, 2).is_err());
        handle.join().unwrap();
    }
    assert!(
        HttpsFetcher::new()
            .unwrap()
            .get(&Url::parse("http://127.0.0.1:1/forbidden").unwrap(), 2)
            .is_err()
    );
}

#[test]
fn real_loopback_https_rejects_non_tls_peer_without_downgrade() {
    // A real HTTPS request sends a TLS ClientHello to this controlled loopback
    // peer. Plain HTTP bytes cannot become an accepted advertisement response.
    let (mut url, handle) =
        response_once(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
    url.set_scheme("https").unwrap();
    assert_eq!(
        HttpsFetcher::new().unwrap().get(&url, 256).err(),
        Some("ad_network")
    );
    let received = handle.join().unwrap();
    assert!(!received.starts_with("GET "));
    assert_eq!(received.as_bytes().first(), Some(&0x16));
}

#[test]
fn stale_confirmation_and_slot_disabled_never_open_cached_target() {
    let root = Root::new();
    let runtime = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&runtime, &manifest(1));
    refresh(&runtime, &mut fetcher, 1000).unwrap();
    assert!(matches!(
        runtime
            .click_locked(AdSlot::ResidentialBroadband, false, &prefs(), &sample(1001))
            .unwrap(),
        ClickDecision::NeedsConfirmation { .. }
    ));
    let mut changed = manifest(2);
    changed.slots[0].campaigns[0].merchant_id = "merchant-two".into();
    fetcher.set(&runtime, &changed);
    refresh(&runtime, &mut fetcher, 1031).unwrap();
    assert!(matches!(
        runtime
            .click_locked(AdSlot::ResidentialBroadband, true, &prefs(), &sample(1032))
            .unwrap(),
        ClickDecision::NeedsConfirmation { .. }
    ));
    let mut off = prefs();
    off.slots_enabled[0] = false;
    assert_eq!(
        runtime.click_locked(AdSlot::ResidentialBroadband, true, &off, &sample(1033)),
        Err("ad_not_current")
    );
    assert!(
        runtime
            .visible(
                &runtime.load(&sample(1033)).unwrap().0,
                &runtime.load(&sample(1033)).unwrap().1,
                &AdPreferences::default(),
                1033
            )
            .is_none()
    );
}

struct TlsFixture {
    root: Root,
    child: std::process::Child,
    port: u16,
}
impl TlsFixture {
    fn new() -> Self {
        use std::io::BufRead;
        use std::process::{Command, Stdio};
        let root = Root::new();
        storage::create_private_directory(&root.0).unwrap();
        // Follow the updates fixture's local OpenSSL + Python server pattern,
        // with a separate CA certificate and a CA:FALSE server leaf.
        std::fs::write(root.0.join("ca.cnf"), "[req]\nprompt=no\ndistinguished_name=dn\nx509_extensions=ext\n[dn]\nCN=Switcher local test CA\n[ext]\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\n").unwrap();
        std::fs::write(root.0.join("leaf.cnf"), "[req]\nprompt=no\ndistinguished_name=dn\n[dn]\nCN=ads.example.com\n[ext]\nsubjectAltName=DNS:ads.example.com\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n").unwrap();
        for args in [
            vec![
                "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-sha256", "-days", "1",
                "-config", "ca.cnf", "-keyout", "ca.key", "-out", "ca.pem",
            ],
            vec![
                "req", "-new", "-newkey", "rsa:2048", "-nodes", "-sha256", "-config", "leaf.cnf",
                "-keyout", "leaf.key", "-out", "leaf.csr",
            ],
            vec![
                "x509",
                "-req",
                "-in",
                "leaf.csr",
                "-CA",
                "ca.pem",
                "-CAkey",
                "ca.key",
                "-CAcreateserial",
                "-days",
                "1",
                "-sha256",
                "-extfile",
                "leaf.cnf",
                "-extensions",
                "ext",
                "-out",
                "leaf.pem",
            ],
        ] {
            assert!(
                Command::new("/usr/bin/openssl")
                    .args(args)
                    .current_dir(&root.0)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let script = r#"import http.server,ssl,sys,pathlib,json
root=pathlib.Path(sys.argv[1])
class H(http.server.BaseHTTPRequestHandler):
 def log_message(self,*args): pass
 def do_GET(self):
  self.connection.settimeout(2)
  if self.headers.get('Transfer-Encoding') is not None:
   self.send_error(411);return
  try: length=int(self.headers.get('Content-Length','0'))
  except ValueError:
   self.send_error(400);return
  if not 0 <= length <= 4096:
   self.send_error(413);return
  body=self.rfile.read(length)
  if len(body) != length:
   self.send_error(400);return
  record={'method':self.command,'path':self.path,'headers':dict(self.headers),'header_pairs':list(self.headers.raw_items()),'body':list(body)}
  with (root/'requests.log').open('a') as f: f.write(json.dumps(record)+'\n')
  if (root/'redirect').exists():
   self.send_response(302);self.send_header('Location','https://ads.example.com/never-follow');self.end_headers();return
  name=self.path.rsplit('/',1)[-1]
  path=root/name
  if not path.is_file(): self.send_error(404);return
  data=path.read_bytes()
  self.send_response(200);self.send_header('Content-Type','application/json' if name.endswith('.json') else 'image/png');self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
server=http.server.HTTPServer(('127.0.0.1',0),H)
ctx=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);ctx.load_cert_chain(str(root/'leaf.pem'),str(root/'leaf.key'))
server.socket=ctx.wrap_socket(server.socket,server_side=True)
print(server.server_port,flush=True)
server.serve_forever()
"#;
        let mut child = Command::new("python3")
            .args(["-u", "-c", script])
            .arg(&root.0)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut port = String::new();
        std::io::BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut port)
            .unwrap();
        Self {
            root,
            child,
            port: port.trim().parse().unwrap(),
        }
    }
    fn fetcher(&self, trust: bool) -> HttpsFetcher {
        let mut builder = reqwest::blocking::Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .timeout(Duration::from_secs(5))
            .resolve(
                "ads.example.com",
                std::net::SocketAddr::from(([127, 0, 0, 1], self.port)),
            );
        if trust {
            builder = builder.add_root_certificate(
                reqwest::Certificate::from_pem(&std::fs::read(self.root.0.join("ca.pem")).unwrap())
                    .unwrap(),
            );
        }
        HttpsFetcher {
            client: builder.build().unwrap(),
        }
    }
    fn requests(&self) -> Vec<serde_json::Value> {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(self.root.0.join("requests.log"))
            .unwrap()
            .take(65_537)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(bytes.len() <= 65_536, "fixture request log exceeded bound");
        String::from_utf8(bytes)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
    fn source(&self) -> ContentSource {
        ContentSource::fixture(
            Url::parse(&format!("https://ads.example.com:{}/content/", self.port)).unwrap(),
        )
    }
}
impl Drop for TlsFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn controlled_https_manifest_and_asset_requests_exclude_synthetic_home_contents() {
    const CHILD: &str = "E11_AD_WIRE_HOME_CHILD";
    const CANARIES: [&str; 4] = [
        "E11_AD_ACCOUNT_A_SYNTHETIC_ONLY",
        "E11_AD_ACCOUNT_B_SYNTHETIC_ONLY",
        "E11_AD_ACTIVE_PROFILE_SYNTHETIC_ONLY",
        "E11_AD_PRIVATE_FILE_CONTENT_SYNTHETIC_ONLY",
    ];
    if std::env::var_os(CHILD).is_none() {
        let home = Root::new();
        storage::create_private_directory(&home.0).unwrap();
        for (relative, marker) in [
            (".claude-switcher/accounts/A/session.json", CANARIES[0]),
            (".claude-switcher/accounts/B/session.json", CANARIES[1]),
            (
                "Library/Application Support/Claude/session.json",
                CANARIES[2],
            ),
            (".claude.json", CANARIES[3]),
        ] {
            let path = home.0.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, marker).unwrap();
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "ad_runtime::tests::controlled_https_manifest_and_asset_requests_exclude_synthetic_home_contents", "--nocapture", "--test-threads=1"])
            .env("HOME", &home.0).env(CHILD, &home.0).output().unwrap();
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
    assert_eq!(home, PathBuf::from(std::env::var_os(CHILD).unwrap()));
    for edition in [Edition::Personal, Edition::Public] {
        let tls = TlsFixture::new();
        let root = Root::new();
        let mut runtime = configured_runtime(&root);
        runtime.edition = edition;
        runtime.pins.as_mut().unwrap().source = tls.source();
        let value = manifest(12);
        let image = &value.slots[0].campaigns[0].image;
        std::fs::write(tls.root.0.join("current.json"), wire(&value)).unwrap();
        std::fs::write(tls.root.0.join(format!("{}.png", image.sha256)), png()).unwrap();
        let _lock = runtime.lock().unwrap();
        let mut fetcher = tls.fetcher(true);
        assert!(
            runtime
                .refresh_locked(&prefs(), true, sample(1000), Some(&mut fetcher), Some(0))
                .unwrap()
                .refreshed
        );
        let requests = tls.requests();
        let expected = [
            "/content/production/current.json".to_owned(),
            format!("/content/production/assets/{}.png", image.sha256),
        ];
        assert_eq!(requests.len(), expected.len());
        for (request, target) in requests.iter().zip(expected) {
            assert_eq!(request["method"], "GET");
            assert_eq!(request["path"], target);
            assert_eq!(request["body"], serde_json::json!([]));
            let pairs = request["header_pairs"].as_array().unwrap();
            let mut headers = std::collections::BTreeMap::new();
            for pair in pairs {
                let name = pair[0].as_str().unwrap().to_ascii_lowercase();
                assert!(
                    matches!(name.as_str(), "host" | "accept"),
                    "unexpected outbound header: {name}"
                );
                assert!(headers.insert(name, pair[1].as_str().unwrap()).is_none());
            }
            assert_eq!(headers.len(), 2);
            assert_eq!(headers["host"], format!("ads.example.com:{}", tls.port));
            assert_eq!(headers["accept"], "application/json,image/png,image/jpeg");
            let wire = serde_json::to_string(request).unwrap();
            for forbidden in CANARIES.into_iter().chain([home.to_str().unwrap()]) {
                assert!(
                    !wire.contains(forbidden),
                    "private HOME data reached ad wire"
                );
            }
        }
        println!(
            "ad wire {edition:?}: manifest + one deduplicated asset, exact headers, empty bodies, synthetic HOME canaries absent"
        );
    }
}

#[test]
fn controlled_https_capture_detects_sensitive_query_headers_and_body_positive_control() {
    let tls = TlsFixture::new();
    let url = format!(
        "https://ads.example.com:{}/capture-control?synthetic=1",
        tls.port
    );
    let response = tls
        .fetcher(true)
        .client
        .get(url)
        .header("Cookie", "synthetic-cookie")
        .header("Authorization", "Bearer synthetic-token")
        .body("synthetic-private-body")
        .send()
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
    let requests = tls.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "GET");
    assert_eq!(requests[0]["path"], "/capture-control?synthetic=1");
    assert_eq!(
        requests[0]["body"],
        serde_json::json!(b"synthetic-private-body".to_vec())
    );
    let pairs = requests[0]["header_pairs"].as_array().unwrap();
    for (name, value) in [
        ("cookie", "synthetic-cookie"),
        ("authorization", "Bearer synthetic-token"),
    ] {
        assert!(
            pairs.iter().any(
                |pair| pair[0].as_str().unwrap().eq_ignore_ascii_case(name) && pair[1] == value
            )
        );
    }
}

#[test]
fn controlled_https_capture_rejects_request_body_over_4096_bytes() {
    let tls = TlsFixture::new();
    let url = format!("https://ads.example.com:{}/capture-control", tls.port);
    let response = tls
        .fetcher(true)
        .client
        .get(url)
        .body(vec![b'x'; 4097])
        .send()
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::PAYLOAD_TOO_LARGE);
    assert!(!tls.root.0.join("requests.log").exists());
}

#[test]
fn controlled_https_ca_signed_manifest_assets_privacy_and_negative_cases() {
    let tls = TlsFixture::new();
    let root = Root::new();
    let mut runtime = configured_runtime(&root);
    runtime.pins.as_mut().unwrap().source = tls.source();
    let value = manifest(11);
    let wire = wire(&value);
    std::fs::write(tls.root.0.join("current.json"), &wire).unwrap();
    std::fs::write(
        tls.root
            .0
            .join(format!("{}.png", value.slots[0].campaigns[0].image.sha256)),
        png(),
    )
    .unwrap();
    let request = runtime
        .pins
        .as_ref()
        .unwrap()
        .source
        .manifest_request(&prefs(), AdChannel::Production)
        .unwrap();
    assert_eq!(
        tls.fetcher(false)
            .get(&request, ads::MAX_MANIFEST_BYTES)
            .err(),
        Some("ad_network")
    );
    let _lock = runtime.lock().unwrap();
    let mut fetcher = tls.fetcher(true);
    assert!(
        runtime
            .refresh_locked(&prefs(), true, sample(1000), Some(&mut fetcher), Some(0))
            .unwrap()
            .refreshed
    );
    let (state, clock) = runtime.load(&sample(1001)).unwrap();
    assert_eq!(state.sequence.highest, 11);
    assert_eq!(clock.last_success_utc, Some(1000));
    assert_eq!(
        runtime
            .visible(&state, &clock, &prefs(), 1001)
            .unwrap()
            .slots
            .iter()
            .map(|slot| slot.campaigns.len())
            .sum::<usize>(),
        3
    );
    let log = std::fs::read_to_string(tls.root.0.join("requests.log")).unwrap();
    assert_eq!(log.lines().count(), 2);
    for line in log.lines() {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        let path = row["path"].as_str().unwrap();
        assert!(!path.contains('?'));
        assert!(path.starts_with("/content/production/"));
        let headers = row["headers"].as_object().unwrap();
        for key in headers.keys() {
            assert!(
                ![
                    "authorization",
                    "cookie",
                    "referer",
                    "user-agent",
                    "x-account-id",
                    "x-device-id"
                ]
                .contains(&key.to_ascii_lowercase().as_str())
            );
        }
    }
    std::fs::write(tls.root.0.join("redirect"), b"yes").unwrap();
    assert_eq!(
        runtime
            .refresh_locked(&prefs(), true, sample(1031), Some(&mut fetcher), Some(0))
            .err(),
        Some("ad_http_status_or_redirect")
    );
    assert!(
        !std::fs::read_to_string(tls.root.0.join("requests.log"))
            .unwrap()
            .contains("never-follow")
    );
    std::fs::remove_file(tls.root.0.join("redirect")).unwrap();
    let mut tampered: serde_json::Value = serde_json::from_slice(&wire).unwrap();
    tampered["payload"] = "{}".into();
    std::fs::write(
        tls.root.0.join("current.json"),
        serde_json::to_vec(&tampered).unwrap(),
    )
    .unwrap();
    assert_eq!(
        runtime
            .refresh_locked(&prefs(), true, sample(1062), Some(&mut fetcher), Some(0))
            .err(),
        Some("signature")
    );
    std::fs::write(
        tls.root.0.join("current.json"),
        vec![b' '; ads::MAX_MANIFEST_BYTES + 1],
    )
    .unwrap();
    assert_eq!(
        runtime
            .refresh_locked(&prefs(), true, sample(1093), Some(&mut fetcher), Some(0))
            .err(),
        Some("ad_download_size")
    );
    let (state, clock) = runtime.load(&sample(1093)).unwrap();
    assert_eq!(state.sequence.highest, 11);
    assert_eq!(clock.last_success_utc, Some(1000));
}

#[test]
fn local_trust_errors_are_explicit_and_public_open_never_creates_publication() {
    let root = Root::new();
    let runtime = AdRuntime::new(root.0.clone(), Edition::Public).with_clock(fixture_clock);
    assert!(!local::directory(&root.0).exists());
    assert!(runtime.snapshot(&prefs()).unwrap().is_none());
    assert!(!local::directory(&root.0).exists());
    storage::create_private_directory(&local::directory(&root.0)).unwrap();
    storage::atomic_write(&local::directory(&root.0).join("trust.json"), b"{}").unwrap();
    assert!(
        AdRuntime::new(root.0.clone(), Edition::Public)
            .with_clock(fixture_clock)
            .snapshot(&prefs())
            .is_err()
    );
    let trust = local::LocalTrust {
        schema: 1,
        key_id: "ads-personal-v1".into(),
        public_key: signer().verifying_key().to_bytes(),
    };
    storage::atomic_json(&local::directory(&root.0).join("trust.json"), &trust).unwrap();
    storage::create_private_directory(&local::directory(&root.0).join("production")).unwrap();
    storage::atomic_write(
        &local::directory(&root.0).join("production/current.json"),
        b"{}",
    )
    .unwrap();
    // Fully disabled advertisements do not inspect malformed manifest or assets.
    let off = AdPreferences {
        slots_enabled: [false; 3],
        request_purpose_acknowledged: true,
    };
    let runtime = AdRuntime::new(root.0.clone(), Edition::Public).with_clock(fixture_clock);
    assert!(!runtime.refresh_if_due(&off, false).unwrap().refreshed);
    assert!(runtime.snapshot(&off).unwrap().is_none());
    assert!(runtime.snapshot(&prefs()).is_err());
}

fn passive_file_snapshot(path: &std::path::Path) -> (Vec<u8>, u64, i64, i64) {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::symlink_metadata(path).unwrap();
    (
        std::fs::read(path).unwrap(),
        m.ino(),
        m.mtime(),
        m.mtime_nsec(),
    )
}

#[test]
fn passive_snapshots_and_images_preserve_initialized_files_both_editions() {
    for edition in [Edition::Personal, Edition::Public] {
        let root = Root::new();
        let runtime = AdRuntime::new(root.0.clone(), edition).with_clock(fixture_clock);
        let disabled = AdPreferences::default();
        runtime.snapshot(&disabled).unwrap();
        let paths = [
            runtime.directory().join("clock.json"),
            runtime.state_path().unwrap(),
        ];
        let before = paths.each_ref().map(|p| passive_file_snapshot(p));
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(3));
            runtime.snapshot(&disabled).unwrap();
            assert!(
                runtime
                    .image_bytes(AdSlot::ALL[0], &disabled)
                    .unwrap()
                    .is_none()
            );
        }
        assert_eq!(before, paths.each_ref().map(|p| passive_file_snapshot(p)));
    }
}

#[test]
fn passive_no_work_refresh_preserves_clock_and_authenticated_history() {
    for local in [false, true] {
        let root = Root::new();
        let mut runtime = configured_runtime(&root);
        let mut fetcher = MemoryFetch::default();
        fetcher.set(&runtime, &manifest(7));
        refresh(&runtime, &mut fetcher, 1000).unwrap();
        if local {
            let directory = local::directory(&root.0);
            storage::create_private_directory(&directory).unwrap();
            storage::create_private_directory(&directory.join("production")).unwrap();
            storage::atomic_write(
                &directory.join("production/current.json"),
                b"synthetic-current",
            )
            .unwrap();
            runtime.pins.as_mut().unwrap().local = true;
            refresh(&runtime, &mut fetcher, 1000).unwrap();
            let (mut state, clock) = runtime.load(&sample(1000)).unwrap();
            state.local_attempt_hash = local::current_digest(&root.0).unwrap();
            state.local_wire_hash = state.local_attempt_hash.clone();
            runtime.save(&state, &clock).unwrap();
        }
        let paths = [
            runtime.directory().join("clock.json"),
            runtime.state_path().unwrap(),
        ];
        let before = paths.each_ref().map(|path| passive_file_snapshot(path));
        let requests = fetcher.requests.len();
        for now in [1001, 1002, 1003] {
            let _lock = runtime.lock().unwrap();
            let result = runtime
                .refresh_locked(&prefs(), false, sample(now), Some(&mut fetcher), Some(0))
                .unwrap();
            assert_eq!(result.reason, "ad_refresh_not_due_or_merged");
        }
        assert_eq!(fetcher.requests.len(), requests);
        assert_eq!(
            before,
            paths.each_ref().map(|path| passive_file_snapshot(path))
        );
        let (state, clock) = runtime.load(&sample(1003)).unwrap();
        assert_eq!(state.sequence.highest, 7);
        assert_eq!(clock.last_success_utc, Some(1000));
    }
}

#[test]
fn passive_disabled_and_missing_materials_preserve_initialized_clock() {
    for configured in [false, true] {
        let root = Root::new();
        let runtime = if configured {
            configured_runtime(&root)
        } else {
            AdRuntime::new(root.0.clone(), Edition::Public).with_clock(fixture_clock)
        };
        let _lock = runtime.lock().unwrap();
        let (state, clock) = runtime.load(&sample(1000)).unwrap();
        runtime.save(&state, &clock).unwrap();
        let path = runtime.directory().join("clock.json");
        let before = passive_file_snapshot(&path);
        let mut fetcher = MemoryFetch::default();
        for now in [1001, 1002] {
            let preferences = if configured {
                AdPreferences::default()
            } else {
                prefs()
            };
            let result = runtime
                .refresh_locked(
                    &preferences,
                    false,
                    sample(now),
                    Some(&mut fetcher),
                    Some(0),
                )
                .unwrap();
            assert!(!result.refreshed);
        }
        assert!(fetcher.requests.is_empty());
        assert_eq!(before, passive_file_snapshot(&path));
    }
}

#[test]
fn failed_clock_preserves_persisted_state_and_denies_all_entrypoints() {
    let root = Root::new();
    let runtime = AdRuntime::new(root.0.clone(), Edition::Public).with_clock(fixture_clock);
    let disabled = AdPreferences::default();
    assert!(runtime.snapshot(&disabled).unwrap().is_none());
    let paths = [
        runtime.directory().join("clock.json"),
        runtime.state_path().unwrap(),
    ];
    let before = paths.each_ref().map(|path| passive_file_snapshot(path));
    let failed = runtime.with_clock(|| Err("synthetic_clock_unavailable"));
    assert_eq!(
        failed.snapshot(&disabled).unwrap_err(),
        "synthetic_clock_unavailable"
    );
    assert_eq!(
        failed.refresh_if_due(&disabled, false).unwrap_err(),
        "synthetic_clock_unavailable"
    );
    assert_eq!(
        failed.image_bytes(AdSlot::ALL[0], &disabled).unwrap_err(),
        "synthetic_clock_unavailable"
    );
    assert!(matches!(
        failed.click_target(AdSlot::ALL[0], false, &disabled),
        Err("synthetic_clock_unavailable")
    ));
    assert_eq!(
        before,
        paths.each_ref().map(|path| passive_file_snapshot(path))
    );
}
