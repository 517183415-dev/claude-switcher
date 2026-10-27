use super::super::super::{
    TargetLedger,
    cleanup::{CleanupOptions, FixedCurrents},
    protocol::Phase,
};
use super::*;

fn publish(p: &mut Publisher, root: &Root, v: &MemoryVault, channel: AdChannel, now: u64) {
    p.publish_configured_with(&root.0, channel, now, v, |c, _, _| {
        remote(&root.0, c, false)
    })
    .unwrap();
}
fn with_sources<T>(
    p: &mut Publisher,
    root: &Root,
    v: &MemoryVault,
    run: impl FnOnce(&mut Publisher, &mut dyn FixedCurrents) -> Result<T>,
) -> Result<T> {
    p.with_configured_currents_using(&root.0, v, |c, _, _| remote(&root.0, c, false), run)
}
fn select_local(p: &mut Publisher, v: &mut MemoryVault) {
    p.configure_r2(
        AdChannel::Preview,
        None,
        Zeroizing::new(String::new()),
        Zeroizing::new(String::new()),
        v,
    )
    .unwrap();
}

#[test]
fn configured_sources_cover_local_both_remote_channels_and_complete_cleanup_gc() {
    let (root, mut p, mut v) = fixture();
    configure(&mut p, &mut v, AdChannel::Preview, "preview-key").unwrap();
    configure(&mut p, &mut v, AdChannel::Production, "production-key").unwrap();
    publish(&mut p, &root, &v, AdChannel::Preview, 1001);
    publish(&mut p, &root, &v, AdChannel::Production, 1002);
    select_local(&mut p, &mut v);
    publish(&mut p, &root, &v, AdChannel::Preview, 1003);
    // A registered but unused file must pass both confirmations before deletion.
    let bytes = b"synthetic-unused-asset";
    let unused = switcher_domain::ads::ImageAsset {
        sha256: switcher_core::signed::digest(bytes),
        mime: "image/png".into(),
        bytes: bytes.len() as u32,
        width: 1,
        height: 1,
    };
    let unused_path = p.directory().join("assets").join(&unused.sha256);
    storage::atomic_write(&unused_path, bytes).unwrap();
    let mut state = p.state.clone();
    state.assets.push(unused.clone());
    p.persist(state).unwrap();
    let ids: Vec<_> = p.state.other_targets.keys().cloned().collect();
    let preview = with_sources(&mut p, &root, &v, |p, source| {
        let others = source.read_other_currents(&ids)?;
        assert_eq!(others.len(), 2);
        assert!(others[&config(AdChannel::Preview).identity()?][0].is_some());
        assert!(others[&config(AdChannel::Production).identity()?][1].is_some());
        p.cleanup_preview(source, CleanupOptions::default(), 1004)
    })
    .unwrap();
    assert!(preview.candidates.contains(&unused));
    assert!(unused_path.exists());
    p.require_no_remote_intents().unwrap();
    let history = p.state.other_targets.clone();
    with_sources(&mut p, &root, &v, |p, source| {
        p.cleanup_commit(source, CleanupOptions::default(), &preview.digest, 1004)
    })
    .unwrap();
    let gc = with_sources(&mut p, &root, &v, |p, source| p.gc_preview(source, 1004)).unwrap();
    with_sources(&mut p, &root, &v, |p, source| {
        p.gc_commit(source, &gc.digest, 1004)
    })
    .unwrap();
    assert!(!unused_path.exists());
    assert_eq!(
        serde_json::to_vec(&p.state.other_targets).unwrap(),
        serde_json::to_vec(&history).unwrap()
    );
    p.require_no_remote_intents().unwrap();
}

#[test]
fn missing_configuration_is_rejected_before_any_provider_is_opened() {
    let (root, mut p, v) = fixture();
    p.state.schema = 3;
    p.state.target = Some(format!("r2:{}", "a".repeat(64)));
    p.state
        .other_targets
        .insert(format!("r2:{}", "b".repeat(64)), TargetLedger::default());
    assert!(matches!(
        p.with_configured_currents_using(
            &root.0,
            &v,
            |_, _, _| panic!("missing target must be detected first"),
            |_, _| Ok(())
        ),
        Err(Error::Invalid("cleanup_target_configuration_missing"))
    ));
    p.require_no_remote_intents().unwrap();
}

#[test]
fn domain_aliases_share_one_physical_lock_and_remote_reader() {
    let (root, mut p, mut v) = fixture();
    configure(&mut p, &mut v, AdChannel::Preview, "preview-key").unwrap();
    publish(&mut p, &root, &v, AdChannel::Preview, 1001);
    let old = p.active_target().unwrap().to_owned();
    let mut alias = config(AdChannel::Preview);
    alias.public_origin = "https://alias.example.com".into();
    p.configure_r2(
        AdChannel::Preview,
        Some(alias),
        Zeroizing::new("preview-key".into()),
        Zeroizing::new("secret-preview-key".into()),
        &mut v,
    )
    .unwrap();
    publish(&mut p, &root, &v, AdChannel::Preview, 1002);
    let mut opened = 0;
    p.with_configured_currents_using(
        &root.0,
        &v,
        |c, _, _| {
            opened += 1;
            remote(&root.0, c, false)
        },
        |p, s| {
            assert!(s.read_other_currents(&[old])?.values().next().unwrap()[0].is_some());
            p.cleanup_preview(s, CleanupOptions::default(), 1003)
        },
    )
    .unwrap();
    assert_eq!(opened, 1);
    p.require_no_remote_intents().unwrap();
}

#[test]
fn pending_publication_retains_lock_across_preview_restart_and_backup_restore_attempt() {
    let (root, mut p, mut v) = fixture();
    configure(&mut p, &mut v, AdChannel::Preview, "preview-key").unwrap();
    assert!(
        p.publish_configured_with(&root.0, AdChannel::Preview, 1001, &v, |c, _, _| remote(
            &root.0, c, true
        ))
        .is_err()
    );
    assert_eq!(
        p.state.pending.as_ref().unwrap().phase,
        Phase::CommitUnknown
    );
    assert!(p.require_no_remote_intents().is_err());
    let preview = with_sources(&mut p, &root, &v, |p, s| {
        p.cleanup_preview(s, CleanupOptions::default(), 1002)
    })
    .unwrap();
    assert!(preview.pending_blocks_commit);
    assert!(p.require_no_remote_intents().is_err());
    let generation = p.generation.clone();
    assert!(matches!(
        p.restore_backup(b"not-an-archive", b"unused", &mut v),
        Err(Error::Invalid("remote_lock_recovery_required"))
    ));
    assert_eq!(p.generation, generation);
    drop(p);
    let mut p = Publisher::open(&root.0.join("publisher"), "ads-personal-v1", None, 1003).unwrap();
    publish(&mut p, &root, &v, AdChannel::Preview, 1003);
    assert!(p.state.pending.is_none());
    p.require_no_remote_intents().unwrap();
}

#[test]
fn closure_errors_release_locks_and_foreign_locks_prevent_maintenance() {
    let (root, mut p, mut v) = fixture();
    configure(&mut p, &mut v, AdChannel::Preview, "preview-key").unwrap();
    publish(&mut p, &root, &v, AdChannel::Preview, 1001);
    assert!(
        with_sources(&mut p, &root, &v, |_, _| Err::<(), _>(Error::Invalid(
            "injected"
        )))
        .is_err()
    );
    p.require_no_remote_intents().unwrap();
    let (_other_root, other, _v2) = fixture();
    let mut store = remote(&root.0, config(AdChannel::Preview), false).unwrap();
    let lease = super::super::super::remote_lock::Lease::acquire(&other, store.as_mut()).unwrap();
    drop(store);
    assert!(matches!(
        with_sources::<()>(&mut p, &root, &v, |_, _| panic!(
            "busy lock must block business logic"
        )),
        Err(Error::Store(StoreError::Conflict))
    ));
    let mut store = remote(&root.0, config(AdChannel::Preview), false).unwrap();
    lease.release(&other, store.as_mut()).unwrap();
    p.require_no_remote_intents().unwrap();
}

#[test]
fn partial_acquisition_releases_prior_locks_without_running_maintenance() {
    let (root, mut p, mut v) = fixture();
    for (channel, key) in [
        (AdChannel::Preview, "preview-key"),
        (AdChannel::Production, "production-key"),
    ] {
        configure(&mut p, &mut v, channel, key).unwrap();
        publish(&mut p, &root, &v, channel, 1001);
    }
    // Lock the last physical namespace in the production acquisition order.
    let mut configs = [config(AdChannel::Preview), config(AdChannel::Production)];
    configs.sort_by_key(|c| c.lock_identity().unwrap());
    let (_other_root, other, _v2) = fixture();
    let mut blocked = remote(&root.0, configs[1].clone(), false).unwrap();
    let lease = super::super::super::remote_lock::Lease::acquire(&other, blocked.as_mut()).unwrap();
    drop(blocked);
    assert!(
        with_sources::<()>(&mut p, &root, &v, |_, _| panic!(
            "partial acquisition must stop"
        ))
        .is_err()
    );
    p.require_no_remote_intents().unwrap();
    let mut first = remote(&root.0, configs[0].clone(), false).unwrap();
    let free = first.read_lock().unwrap().unwrap();
    assert!(serde_json::from_slice::<serde_json::Value>(&free.bytes).unwrap()["owner"].is_null());
    let mut blocked = remote(&root.0, configs[1].clone(), false).unwrap();
    lease.release(&other, blocked.as_mut()).unwrap();
}

#[test]
fn changed_local_identity_fails_before_remote_acquisition() {
    let (root, mut p, mut v) = fixture();
    publish(&mut p, &root, &v, AdChannel::Preview, 1001);
    configure(&mut p, &mut v, AdChannel::Preview, "preview-key").unwrap();
    publish(&mut p, &root, &v, AdChannel::Preview, 1002);
    let publication_root = switcher_core::ad_runtime::local::directory(&root.0);
    fs::rename(
        &publication_root,
        publication_root.with_extension("retained"),
    )
    .unwrap();
    assert!(matches!(
        p.with_configured_currents_using(
            &root.0,
            &v,
            |_, _, _| panic!("changed local root must be detected first"),
            |_, _| Ok(())
        ),
        Err(Error::Invalid("cleanup_local_target_changed"))
    ));
    p.require_no_remote_intents().unwrap();
}

#[test]
fn authenticated_inactive_current_pins_local_asset_through_real_cleanup_and_gc() {
    use ed25519_dalek::Signer;
    use switcher_core::signed;
    let (root, mut p, mut v) = fixture();
    configure(&mut p, &mut v, AdChannel::Preview, "preview-key").unwrap();
    publish(&mut p, &root, &v, AdChannel::Preview, 1001);
    select_local(&mut p, &mut v);
    publish(&mut p, &root, &v, AdChannel::Preview, 1002);
    // Protocol fixtures use synthetic bytes; this test exercises reference/GC
    // validation, not the separate image decoder or uploader.
    let mut assets = Vec::new();
    for bytes in [
        b"pinned-synthetic".as_slice(),
        b"unused-synthetic".as_slice(),
    ] {
        let asset = switcher_domain::ads::ImageAsset {
            sha256: signed::digest(bytes),
            mime: "image/png".into(),
            bytes: bytes.len() as u32,
            width: 1,
            height: 1,
        };
        storage::atomic_write(&p.directory().join("assets").join(&asset.sha256), bytes).unwrap();
        assets.push(asset);
    }
    let mut state = p.state.clone();
    state.assets = assets.clone();
    p.persist(state).unwrap();
    let mut manifest = p.draft_read();
    manifest.channel = AdChannel::Preview;
    manifest.content_revision = 10;
    manifest.release_id = super::super::super::random_id().unwrap();
    manifest.slots[0].campaigns = vec![switcher_domain::ads::Campaign {
        campaign_id: "synthetic".into(),
        merchant_id: "synthetic".into(),
        merchant_name: "Synthetic".into(),
        title: "Synthetic".into(),
        description: "Synthetic".into(),
        image: assets[0].clone(),
        button_text: "Open".into(),
        target_url: "https://example.com/".into(),
        starts_at: 1000,
        ends_at: 1100,
        enabled: true,
    }];
    let payload = serde_json::to_string(&manifest).unwrap();
    let envelope = signed::Envelope {
        signature: ed25519_dalek::SigningKey::from_bytes(&[42; 32])
            .sign(payload.as_bytes())
            .to_bytes()
            .to_vec(),
        payload,
    };
    let wire = serde_json::to_vec(&envelope).unwrap();
    let mut remote = remote(&root.0, config(AdChannel::Preview), false).unwrap();
    let old = remote.current().unwrap().unwrap();
    remote.commit_current(Some(&old.etag), &wire).unwrap();
    drop(remote);
    let preview = with_sources(&mut p, &root, &v, |p, s| {
        p.cleanup_preview(s, CleanupOptions::default(), 1003)
    })
    .unwrap();
    assert_eq!(preview.candidates, vec![assets[1].clone()]);
    assert!(
        preview
            .protected_assets
            .iter()
            .any(|a| a.asset == assets[0])
    );
    with_sources(&mut p, &root, &v, |p, s| {
        p.cleanup_commit(s, CleanupOptions::default(), &preview.digest, 1003)
    })
    .unwrap();
    let gc = with_sources(&mut p, &root, &v, |p, s| p.gc_preview(s, 1003)).unwrap();
    with_sources(&mut p, &root, &v, |p, s| p.gc_commit(s, &gc.digest, 1003)).unwrap();
    assert!(
        p.directory()
            .join("assets")
            .join(&assets[0].sha256)
            .exists()
    );
    assert!(
        !p.directory()
            .join("assets")
            .join(&assets[1].sha256)
            .exists()
    );
    p.require_no_remote_intents().unwrap();
}
