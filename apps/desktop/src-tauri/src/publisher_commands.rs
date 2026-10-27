//! Compiled into Personal only. Remote settings require an explicit operator action.
use crate::{
    bridge::{self, DesktopState},
    publisher::{
        self, Publisher,
        cleanup::CleanupOptions,
        security::{ManagedVault, Vault},
    },
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::sync::Arc;
use switcher_domain::{
    AdSlot,
    ads::{AdChannel, AdManifest},
    desktop::*,
};
use tauri::State;

type Desktop<'a> = State<'a, Arc<DesktopState>>;
pub(crate) type PublisherVault<'a> = State<'a, Arc<ManagedVault>>;

/// Legacy web clients send integer arrays; the native client sends base64 to
/// avoid allocating one Swift object per byte for a large backup.
#[derive(serde::Deserialize)]
#[serde(untagged)]
pub enum PublisherInputBytes {
    Array(Vec<u8>),
    Base64(String),
}

impl PublisherInputBytes {
    pub(crate) fn decode(self, limit: usize, too_large: &str) -> Result<Vec<u8>, String> {
        let bytes = match self {
            Self::Array(bytes) => bytes,
            Self::Base64(encoded) => {
                let max_encoded = (limit.saturating_add(2) / 3).saturating_mul(4);
                if encoded.len() > max_encoded {
                    return Err(too_large.into());
                }
                STANDARD
                    .decode(encoded)
                    .map_err(|_| "文件数据格式无效".to_owned())?
            }
        };
        if bytes.len() > limit {
            return Err(too_large.into());
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod publisher_input_tests {
    use super::PublisherInputBytes;
    use serde_json::json;

    #[test]
    fn native_base64_and_legacy_arrays_preserve_exact_bytes() {
        for value in [json!([0, 1, 127, 255]), json!("AAF//w==")] {
            let input: PublisherInputBytes = serde_json::from_value(value).unwrap();
            assert_eq!(input.decode(4, "too large").unwrap(), [0, 1, 127, 255]);
        }
    }

    #[test]
    fn invalid_and_oversized_binary_inputs_are_rejected() {
        for value in [json!([0, 1]), json!("AAE="), json!("AAECAw==")] {
            let input: PublisherInputBytes = serde_json::from_value(value).unwrap();
            assert_eq!(input.decode(1, "too large").unwrap_err(), "too large");
        }
        let input: PublisherInputBytes = serde_json::from_value(json!("!!!")).unwrap();
        assert_eq!(
            input.decode(8, "too large").unwrap_err(),
            "文件数据格式无效"
        );
        assert!(serde_json::from_value::<PublisherInputBytes>(json!([256])).is_err());
    }
}
pub(crate) fn with_publisher<T>(
    state: &DesktopState,
    run: impl FnOnce(&mut Publisher) -> publisher::Result<T>,
) -> Result<T, String> {
    let mut publisher = open_publisher(state)?;
    run(&mut publisher).map_err(|e| e.to_string())
}

pub(crate) fn with_publisher_vault<T>(
    state: &DesktopState,
    vault: &ManagedVault,
    run: impl FnOnce(&mut Publisher, &mut dyn Vault) -> publisher::Result<T>,
) -> Result<T, String> {
    with_publisher(state, |publisher| vault.with(|vault| run(publisher, vault)))
}
pub(crate) fn open_publisher(state: &DesktopState) -> Result<Publisher, String> {
    state.prepare_directory()?;
    let directory = state.directory().join("publisher");
    Publisher::open(
        &directory,
        "ads-personal-v1",
        None,
        switcher_core::checks::unix_now(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn ad_draft_read(state: Desktop<'_>) -> Result<AdManifest, String> {
    with_publisher(&state, |p| Ok(p.draft_read()))
}
#[tauri::command]
pub fn ad_draft_save(state: Desktop<'_>, manifest: AdManifest) -> Result<AdManifest, String> {
    with_publisher(&state, |p| {
        p.draft_save(manifest, switcher_core::checks::unix_now())
    })
}
#[tauri::command]
pub fn import_ad_image(
    state: Desktop<'_>,
    bytes: PublisherInputBytes,
) -> Result<ImportedImage, String> {
    let bytes = bytes.decode(switcher_core::ads::MAX_IMAGE_BYTES, "图片超过 1 MiB 上限")?;
    import_ad_image_body(bytes, |bytes, mime| {
        with_publisher(&state, |p| {
            let asset = p.import_image(bytes, mime)?;
            let preview_data_url = p.image_preview(&asset.sha256)?;
            Ok(ImportedImage {
                asset,
                preview_data_url,
            })
        })
    })
}

pub(crate) fn import_ad_image_body(
    bytes: Vec<u8>,
    import: impl FnOnce(&[u8], &str) -> Result<ImportedImage, String>,
) -> Result<ImportedImage, String> {
    if bytes.len() > switcher_core::ads::MAX_IMAGE_BYTES {
        return Err("图片超过 1 MiB 上限".into());
    }
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else {
        return Err("请选择静态 PNG 或 JPEG 图片".into());
    };
    import(&bytes, mime)
}

#[cfg(test)]
mod image_command_body_tests {
    use super::*;
    use std::cell::Cell;
    use switcher_domain::ads::ImageAsset;

    fn imported(mime: &str, bytes: usize) -> ImportedImage {
        ImportedImage {
            asset: ImageAsset {
                sha256: "fixture".into(),
                mime: mime.into(),
                bytes: bytes as u32,
                width: 1,
                height: 1,
            },
            preview_data_url: "fixture-preview".into(),
        }
    }

    #[test]
    fn inclusive_image_limit_forwards_original_bytes_and_detected_mime_once() {
        let limit = switcher_core::ads::MAX_IMAGE_BYTES;
        for (magic, mime) in [
            (b"\x89PNG\r\n\x1a\n".as_slice(), "image/png"),
            (b"\xff\xd8\xff".as_slice(), "image/jpeg"),
        ] {
            for len in [limit - 1, limit] {
                let mut bytes = vec![0x42; len];
                bytes[..magic.len()].copy_from_slice(magic);
                let expected = bytes.clone();
                let calls = Cell::new(0);
                let result = import_ad_image_body(bytes, |actual, actual_mime| {
                    calls.set(calls.get() + 1);
                    assert_eq!(actual, expected);
                    assert_eq!(actual_mime, mime);
                    Ok(imported(actual_mime, actual.len()))
                })
                .unwrap();
                assert_eq!(calls.get(), 1);
                assert_eq!(result.asset.mime, mime);
                assert_eq!(result.asset.bytes as usize, len);
                assert_eq!(result.preview_data_url, "fixture-preview");
            }
        }
    }

    #[test]
    fn oversized_images_are_rejected_before_mime_or_import() {
        for magic in [b"\x89PNG\r\n\x1a\n".as_slice(), b"\xff\xd8\xff", b"invalid"] {
            let mut bytes = vec![0; switcher_core::ads::MAX_IMAGE_BYTES + 1];
            bytes[..magic.len()].copy_from_slice(magic);
            let result = import_ad_image_body(bytes, |_, _| panic!("import must not run"));
            assert_eq!(result.unwrap_err(), "图片超过 1 MiB 上限");
        }
    }

    #[test]
    fn empty_and_unsupported_images_are_rejected_before_import() {
        for bytes in [vec![], b"GIF89a".to_vec(), b"\x89PNG".to_vec()] {
            let result = import_ad_image_body(bytes, |_, _| panic!("import must not run"));
            assert_eq!(result.unwrap_err(), "请选择静态 PNG 或 JPEG 图片");
        }
    }

    #[test]
    fn import_error_is_returned_without_replacement() {
        let calls = Cell::new(0);
        let result = import_ad_image_body(b"\xff\xd8\xff".to_vec(), |bytes, mime| {
            calls.set(calls.get() + 1);
            assert_eq!(bytes, b"\xff\xd8\xff");
            assert_eq!(mime, "image/jpeg");
            Err("fixture import error".into())
        });
        assert_eq!(calls.get(), 1);
        assert_eq!(result.unwrap_err(), "fixture import error");
    }
}
#[tauri::command]
pub fn ad_image_preview(state: Desktop<'_>, sha256: String) -> Result<String, String> {
    with_publisher(&state, |p| p.image_preview(&sha256))
}
#[tauri::command]
pub fn publisher_status(state: Desktop<'_>) -> Result<PublisherView, String> {
    with_publisher(&state, |p| {
        let mut view = publisher_view(
            p.status(),
            p.configured_public_key().is_some(),
            p.cleanup_pending(),
        );
        if p.active_target()
            .is_some_and(|target| target.starts_with("r2:"))
        {
            view.source = Some("Cloudflare R2（上次选择的发布目标）".into());
        }
        Ok(view)
    })
}
fn publisher_view(
    status: publisher::Status,
    has_signing_key: bool,
    cleanup_pending: bool,
) -> PublisherView {
    let key_status = if has_signing_key {
        "签名公钥已配置。"
    } else {
        "尚未生成广告签名密钥。"
    };
    let pending_status = status
        .pending_release_id
        .as_ref()
        .map_or_else(String::new, |id| {
            format!(" 未完成发布：{id}；重新发布时先核对所选发布目标的结果。")
        });
    let resolution_status = status
        .last_resolution
        .as_ref()
        .map_or_else(String::new, |summary| {
            format!(" 保留的发布处理记录：{}", resolution_message(summary))
        });
    let cleanup_status = if cleanup_pending {
        " 存在待结束的文件回收批次，请重新预览后确认。"
    } else {
        ""
    };
    PublisherView {
        source: Some("本机私有目录".into()),
        status: if status.recovery_required {
            "recovery_required"
        } else if status.phase.is_some() {
            "pending"
        } else if !has_signing_key {
            "wait_signing_key"
        } else if cleanup_pending {
            "cleanup_pending"
        } else if status.last_resolution.is_some() {
            "resolution_recorded"
        } else {
            "ready_local"
        }
        .into(),
        message: format!(
            "本地草稿可用。{key_status}{pending_status}{resolution_status}{cleanup_status} 发布来源按渠道配置选择；只有签名内容与素材公开回读通过，才视为发布完成。"
        ),
        history: status
            .history
            .into_iter()
            .map(|record| PublishRecord {
                release_id: record.manifest.release_id,
                revision: record.manifest.content_revision,
                channel: match record.manifest.channel {
                    AdChannel::Preview => "preview",
                    AdChannel::Production => "production",
                }
                .into(),
                published_at: record.published_at,
                payload_hash: record.payload_hash,
                verified: true,
            })
            .collect(),
    }
}
fn publication_outcome(
    result: publisher::Result<publisher::Publication>,
) -> publisher::Result<OperationOutcome> {
    match result {
        Ok(_) => Ok(bridge::completed("签名内容及素材已通过公开回读验证")),
        Err(publisher::Error::Resolved(summary)) => Ok(OperationOutcome {
            completed: false,
            message: resolution_message(&summary),
            recovery: Some("处理证据已保留。请编辑新草稿或建立全部暂停草稿，再预览并发布；整理时可明确确认释放此记录槽。".into()),
        }),
        Err(error) => Err(error),
    }
}
fn resolution_message(summary: &publisher::ResolutionSummary) -> String {
    let reason = match summary.kind {
        publisher::ResolutionKind::Superseded => {
            format!("已由更高序号 {} 接替", summary.current_revision)
        }
        publisher::ResolutionKind::ExpiredCommitted => "已提交，但内容已过期".into(),
        publisher::ResolutionKind::CommittedAssetsUnavailable => {
            "已提交，但所需素材无法补齐".into()
        }
    };
    format!(
        "发布 {}（序号 {}）{reason}；未记入已验证历史。",
        summary.release_id, summary.revision
    )
}
#[cfg(test)]
mod f16_feedback_tests {
    use super::*;
    use publisher::{ResolutionKind, ResolutionSummary};

    #[test]
    fn restored_backup_feedback_requires_explicit_publication_to_selected_target() {
        let calls = std::cell::Cell::new(0);
        let result = restore_publisher_backup_body(
            b"synthetic-backup",
            "synthetic-passphrase".into(),
            |bytes, passphrase| {
                calls.set(calls.get() + 1);
                assert_eq!(bytes, b"synthetic-backup");
                assert_eq!(passphrase, b"synthetic-passphrase");
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(calls.get(), 1);
        assert!(!result.completed);
        assert!(result.message.contains("尚未发布"));
        let recovery = result.recovery.unwrap();
        assert!(recovery.contains("所选发布目标"));
        assert!(!recovery.contains("本机正式目录"));
        assert_eq!(
            restore_publisher_backup_body(
                b"synthetic-backup",
                "synthetic-passphrase".into(),
                |_, _| Err("synthetic restore rejected".into())
            )
            .unwrap_err(),
            "synthetic restore rejected"
        );
    }

    #[test]
    fn pending_publication_feedback_rechecks_selected_target() {
        let mut pending = status();
        pending.phase = Some(publisher::protocol::Phase::CommitUnknown);
        pending.pending_release_id = Some("synthetic-pending".into());
        let view = publisher_view(pending, true, false);
        assert_eq!(view.status, "pending");
        assert!(view.message.contains("先核对所选发布目标的结果"));
        assert!(!view.message.contains("先核对本机结果"));
    }

    fn resolution(kind: ResolutionKind) -> ResolutionSummary {
        ResolutionSummary {
            kind,
            release_id: "synthetic-pending".into(),
            channel: AdChannel::Production,
            revision: 3,
            current_release_id: "synthetic-current".into(),
            current_revision: 4,
            resolved_at: 100,
        }
    }
    fn status() -> publisher::Status {
        publisher::Status {
            phase: None,
            pending_release_id: None,
            recovery_required: false,
            history: vec![],
            last_resolution: Some(resolution(ResolutionKind::Superseded)),
        }
    }
    #[test]
    fn resolved_outcomes_are_delivered_without_claiming_verified_publication() {
        for kind in [
            ResolutionKind::Superseded,
            ResolutionKind::ExpiredCommitted,
            ResolutionKind::CommittedAssetsUnavailable,
        ] {
            let result =
                publication_outcome(Err(publisher::Error::Resolved(resolution(kind)))).unwrap();
            assert!(!result.completed);
            assert!(result.message.contains("synthetic-pending"));
            assert!(result.message.contains("未记入已验证历史"));
            assert!(result.recovery.is_some());
        }
    }
    #[test]
    fn only_verified_publication_is_completed() {
        let result = publication_outcome(Ok(publisher::Publication {
            release_id: "synthetic-verified".into(),
            revision: 3,
            channel: AdChannel::Preview,
        }))
        .unwrap();
        assert!(result.completed);
        assert!(result.message.contains("回读验证"));
    }
    #[test]
    fn ordinary_publish_errors_remain_errors() {
        for error in [
            publisher::Error::PendingOperation,
            publisher::Error::ResolutionOccupied,
            publisher::Error::Store(publisher::protocol::StoreError::Unavailable),
        ] {
            assert!(publication_outcome(Err(error)).is_err());
        }
    }
    #[test]
    fn status_keeps_recovery_and_pending_priority_without_inventing_history() {
        let view = publisher_view(status(), true, false);
        assert_eq!(view.status, "resolution_recorded");
        assert!(view.message.contains("synthetic-pending"));
        assert!(view.history.is_empty());
        let mut pending = status();
        pending.phase = Some(publisher::protocol::Phase::CommitUnknown);
        pending.pending_release_id = Some("synthetic-second".into());
        assert_eq!(
            publisher_view(pending.clone(), true, true).status,
            "pending"
        );
        pending.recovery_required = true;
        assert_eq!(
            publisher_view(pending, true, true).status,
            "recovery_required"
        );
        assert_eq!(
            publisher_view(status(), false, true).status,
            "wait_signing_key"
        );
        assert_eq!(
            publisher_view(status(), true, true).status,
            "cleanup_pending"
        );
    }
}
#[cfg(feature = "ipc-business")]
pub(crate) fn publish_local(
    publisher: &mut Publisher,
    root: &std::path::Path,
    channel: AdChannel,
    now: u64,
    vault: &dyn Vault,
) -> publisher::Result<OperationOutcome> {
    let public_key = publisher
        .configured_public_key()
        .ok_or(publisher::Error::MissingSigningKey)?;
    let trust = switcher_core::ad_runtime::local::LocalTrust {
        schema: 1,
        key_id: "ads-personal-v1".into(),
        public_key,
    };
    let mut source = publisher::local_store::LocalObjectStore::open(root, channel, &trust)?;
    publication_outcome(publisher.publish(channel, now, Some(&mut source), vault))
}
#[tauri::command]
pub async fn publish_ad(
    state: Desktop<'_>,
    vault: PublisherVault<'_>,
    channel: AdChannel,
) -> Result<OperationOutcome, String> {
    let state = state.inner().clone();
    let vault = vault.inner().clone();
    tokio::task::spawn_blocking(move || {
        with_publisher_vault(&state, &vault, |p, vault| {
            publication_outcome(p.publish_configured(
                &state.root,
                channel,
                switcher_core::checks::unix_now(),
                vault,
            ))
        })
    })
    .await
    .map_err(|_| "发布操作被中断".to_owned())?
}
#[tauri::command]
pub async fn publisher_remote_read(
    state: Desktop<'_>,
    vault: PublisherVault<'_>,
) -> Result<publisher::connection::RemoteSettings, String> {
    let state = state.inner().clone();
    let vault = vault.inner().clone();
    tokio::task::spawn_blocking(move || {
        with_publisher_vault(&state, &vault, |p, vault| p.remote_settings(vault))
    })
    .await
    .map_err(|_| "远程发布配置读取被中断".to_owned())?
}

#[tauri::command]
pub async fn publisher_remote_save(
    state: Desktop<'_>,
    vault: PublisherVault<'_>,
    channel: AdChannel,
    config: Option<publisher::r2_store::R2Config>,
    access_key: String,
    secret_key: String,
) -> Result<publisher::connection::RemoteSettings, String> {
    let access_key = zeroize::Zeroizing::new(access_key);
    let secret_key = zeroize::Zeroizing::new(secret_key);
    let state = state.inner().clone();
    let vault = vault.inner().clone();
    tokio::task::spawn_blocking(move || {
        with_publisher_vault(&state, &vault, |p, vault| {
            p.configure_r2(channel, config, access_key, secret_key, vault)
        })
    })
    .await
    .map_err(|_| "远程发布配置保存被中断，请重新读取配置".to_owned())?
}
#[tauri::command]
pub async fn publisher_remote_select(
    state: Desktop<'_>,
    vault: PublisherVault<'_>,
    channel: AdChannel,
    target: String,
) -> Result<publisher::connection::RemoteSettings, String> {
    let state = state.inner().clone();
    let vault = vault.inner().clone();
    tokio::task::spawn_blocking(move || {
        with_publisher_vault(&state, &vault, |p, vault| {
            p.select_retained_r2(channel, &target, vault)
        })
    })
    .await
    .map_err(|_| "发布目标切换被中断，请重新读取配置".to_owned())?
}

async fn with_cleanup_source<T: Send + 'static>(
    state: Arc<DesktopState>,
    vault: Arc<ManagedVault>,
    run: impl FnOnce(
        &mut Publisher,
        &mut dyn publisher::cleanup::FixedCurrents,
        u64,
    ) -> publisher::Result<T>
    + Send
    + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(move || {
        with_publisher_vault(&state, &vault, |publisher, vault| {
            publisher.with_configured_currents(&state.root, vault, |publisher, source| {
                run(publisher, source, switcher_core::checks::unix_now())
            })
        })
    })
    .await
    .map_err(|_| "本机整理操作被中断，请重新预览".to_owned())?
}
fn history_label(record: &publisher::HistoryRecord) -> String {
    format!(
        "{} · 序号 {} · {}",
        if record.manifest.channel == AdChannel::Preview {
            "本机预览"
        } else {
            "本机正式"
        },
        record.manifest.content_revision,
        record.manifest.release_id
    )
}
fn protection_label(reason: &str) -> &str {
    match reason {
        "current_preview" => "当前预览内容",
        "current_production" => "当前正式内容",
        "draft" => "已保存草稿",
        "pending" => "未完成发布",
        "retained_history" => "保留历史",
        "resolution_pending" => "上次处理记录中的原发布",
        "resolution_current" => "上次处理记录中的已认证内容",
        _ => "其他保留引用",
    }
}
#[tauri::command]
pub async fn preview_publisher_cleanup(
    state: Desktop<'_>,
    vault: PublisherVault<'_>,
    retain: u32,
    release_resolution: bool,
) -> Result<PublisherCleanupPreview, String> {
    with_cleanup_source(state.inner().clone(), vault.inner().clone(), move |publisher, source, now| {
        let preview = publisher.cleanup_preview(source, CleanupOptions { retain: retain as usize, release_resolution }, now)?;
        let history = publisher.status().history;
        let retained: std::collections::HashSet<_> = preview.keep_history.iter().copied().collect();
        Ok(PublisherCleanupPreview {
            digest: preview.digest,
            retain,
            history_before: history.len() as u32,
            history_after: retained.len() as u32,
            assets_before: (preview.protected_assets.len() + preview.candidates.len()) as u32,
            assets_after: preview.protected_assets.len() as u32,
            candidate_bytes: preview.candidate_bytes,
            pending_blocks_commit: preview.pending_blocks_commit,
            release_resolution,
            replaces_checkpoint: preview.previous_checkpoint.is_some(),
            retained_history: history.iter().enumerate().filter(|(index, _)| retained.contains(index)).map(|(_, record)| history_label(record)).collect(),
            removed_history_records: history.iter().enumerate().filter(|(index, _)| !retained.contains(index)).map(|(_, record)| history_label(record)).collect(),
            protected_assets: preview.protected_assets.iter().map(|item| format!("{}：{}", item.asset.sha256, item.reasons.iter().map(|reason| protection_label(reason)).collect::<Vec<_>>().join("、"))).collect(),
            candidate_assets: preview.candidates.iter().map(|asset| format!("{} · {} KiB", asset.sha256, asset.bytes.div_ceil(1024))).collect(),
            message: "请核对下方具体条目。确认后只整理索引，并保存本批撤销记录；文件回收需要下一次独立预览和确认。".into(),
        })
    }).await
}
#[tauri::command]
pub async fn commit_publisher_cleanup(
    state: Desktop<'_>,
    vault: PublisherVault<'_>,
    retain: u32,
    release_resolution: bool,
    digest: String,
) -> Result<OperationOutcome, String> {
    with_cleanup_source(
        state.inner().clone(),
        vault.inner().clone(),
        move |publisher, source, now| {
            let result = publisher.cleanup_commit(
                source,
                CleanupOptions {
                    retain: retain as usize,
                    release_resolution,
                },
                &digest,
                now,
            )?;
            Ok(OperationOutcome {
                completed: true,
                message: format!(
                    "索引整理已完成：移除 {} 条历史和 {} 条素材索引。文件回收仍待单独确认。",
                    result.removed_history, result.removed_assets
                ),
                recovery: Some("请点击“预览本批文件回收”，核对文件后再次确认。".into()),
            })
        },
    )
    .await
}
#[tauri::command]
pub async fn preview_publisher_gc(
    state: Desktop<'_>,
    vault: PublisherVault<'_>,
) -> Result<PublisherGcPreview, String> {
    with_cleanup_source(state.inner().clone(), vault.inner().clone(), |publisher, source, now| {
        let preview = publisher.gc_preview(source, now)?;
        Ok(PublisherGcPreview {
            batch_id: preview.batch_id, digest: preview.digest,
            eligible: preview.eligible, already_missing: preview.already_missing,
            blocked: preview.blocked, referenced: preview.referenced,
            eligible_bytes: preview.eligible_bytes,
            pending_blocks_commit: preview.pending_blocks_commit,
            blocked_details: preview.blocked_reasons.iter().map(|(hash, _)| format!("{hash}：未通过本批回收校验，保留文件。")).collect(),
            message: "只回收本批核实通过且没有保留引用的文件。校验未通过和新增引用的文件保留。确认后结束本批完整撤销能力。".into(),
        })
    }).await
}
#[tauri::command]
pub async fn commit_publisher_gc(
    state: Desktop<'_>,
    vault: PublisherVault<'_>,
    digest: String,
) -> Result<OperationOutcome, String> {
    with_cleanup_source(state.inner().clone(), vault.inner().clone(), move |publisher, source, now| {
        let result = publisher.gc_commit(source, &digest, now)?;
        Ok(OperationOutcome {
            completed: result.batch_finished,
            message: format!(
                "{}：删除 {} 个文件（{} KiB），已不存在 {} 个；校验保留 {} 个，引用保留 {} 个。",
                if result.batch_finished {
                    "本批回收已结束"
                } else {
                    "本批回收尚未结束"
                },
                result.deleted.len(),
                result.deleted_bytes.div_ceil(1024),
                result.already_missing.len(),
                result.blocked.len(),
                result.referenced.len()
            ),
            recovery: if result.batch_finished {
                None
            } else {
                Some("请重新预览本批回收，再核对并确认。".into())
            },
        })
    })
    .await
}
#[tauri::command]
pub fn pause_ad(state: Desktop<'_>, slot: Option<AdSlot>) -> Result<AdManifest, String> {
    with_publisher(&state, |p| p.pause(slot, switcher_core::checks::unix_now()))
}
#[tauri::command]
pub fn restore_ad_history(state: Desktop<'_>, release_id: String) -> Result<AdManifest, String> {
    with_publisher(&state, |p| {
        p.restore_history(&release_id, switcher_core::checks::unix_now())
    })
}
#[tauri::command]
pub fn create_ad_signing_key(
    state: Desktop<'_>,
    vault: PublisherVault<'_>,
) -> Result<OperationOutcome, String> {
    with_publisher_vault(&state, &vault, |p, vault| {
        let key = p.generate_signing_key(vault, Vec::new(), Vec::new())?;
        let hex: String = key.iter().map(|byte| format!("{byte:02x}")).collect();
        Ok(bridge::completed(format!(
            "广告签名私钥已保存到本机钥匙串。公开验证公钥：{hex}"
        )))
    })
}
#[tauri::command]
pub async fn export_publisher_backup(
    state: Desktop<'_>,
    vault: PublisherVault<'_>,
    passphrase: String,
) -> Result<OperationOutcome, String> {
    let state = state.inner().clone();
    let vault = vault.inner().clone();
    tokio::task::spawn_blocking(move || {
        let passphrase = zeroize::Zeroizing::new(passphrase);
        with_publisher_vault(&state, &vault, |p, vault| {
            let bytes = p.export_backup(passphrase.as_bytes(), vault)?;
            let directory = state.directory().join("publisher/backups");
            switcher_core::storage::create_private_directory(&directory)?;
            let digest = switcher_core::signed::digest(&bytes);
            let filename = format!(
                "backup-{}-{}.csbackup",
                switcher_core::checks::unix_now(),
                &digest[..12]
            );
            switcher_core::storage::atomic_write(&directory.join(&filename), &bytes)?;
            Ok(bridge::completed(format!(
                "已保存加密备份：{filename}。请打开备份目录，将备份复制到安全位置。"
            )))
        })
    })
    .await
    .map_err(|_| "备份操作被中断".to_owned())?
}
#[tauri::command]
pub async fn restore_publisher_backup(
    state: Desktop<'_>,
    vault: PublisherVault<'_>,
    bytes: PublisherInputBytes,
    passphrase: String,
) -> Result<OperationOutcome, String> {
    let state = state.inner().clone();
    let vault = vault.inner().clone();
    tokio::task::spawn_blocking(move || {
        let mut passphrase = zeroize::Zeroizing::new(passphrase);
        let bytes = bytes.decode(publisher::backup::LIMIT, "备份文件超过 64 MiB 上限")?;
        restore_publisher_backup_body(
            &bytes,
            std::mem::take(&mut *passphrase),
            |bytes, passphrase| {
                with_publisher_vault(&state, &vault, |p, vault| {
                    p.restore_backup(bytes, passphrase, vault)
                })
            },
        )
    })
    .await
    .map_err(|_| "恢复操作被中断".to_owned())?
}

pub(crate) fn restore_publisher_backup_body(
    bytes: &[u8],
    passphrase: String,
    restore: impl FnOnce(&[u8], &[u8]) -> Result<(), String>,
) -> Result<OperationOutcome, String> {
    let passphrase = zeroize::Zeroizing::new(passphrase);
    if bytes.len() > publisher::backup::LIMIT {
        return Err("备份文件超过 64 MiB 上限".into());
    }
    restore(bytes, passphrase.as_bytes())?;
    Ok(bridge::waiting(
        "已恢复加密备份中的本地资料，尚未发布",
        "请核对草稿后明确发布；软件将先回读所选发布目标的最高序号，并补齐缺失素材",
    ))
}
#[tauri::command]
pub fn open_publisher_backup_folder(state: Desktop<'_>) -> Result<OperationOutcome, String> {
    let directory = state.directory().join("publisher/backups");
    if !directory.is_dir() {
        return Err("尚无已保存的加密备份".into());
    }
    let status = std::process::Command::new("/usr/bin/open")
        .arg(directory)
        .status()
        .map_err(|_| "无法打开备份目录")?;
    if status.success() {
        Ok(bridge::completed("已打开本机加密备份目录"))
    } else {
        Err("无法打开备份目录".into())
    }
}
