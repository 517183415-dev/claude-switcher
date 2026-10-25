//! Compiled into Personal only. No input accepts a filesystem path or content-source URL.
use crate::{
    bridge::{self, DesktopState},
    publisher::{self, Publisher, local_store::LocalObjectStore, security::KeychainVault},
};
use std::sync::Arc;
use switcher_domain::{
    AdSlot,
    ads::{AdChannel, AdManifest},
    desktop::*,
};
use tauri::State;

type Desktop<'a> = State<'a, Arc<DesktopState>>;
fn with_publisher<T>(
    state: &DesktopState,
    run: impl FnOnce(&mut Publisher) -> publisher::Result<T>,
) -> Result<T, String> {
    state.prepare_directory()?;
    let directory = state.directory().join("publisher");
    let mut publisher = Publisher::open(
        &directory,
        "ads-personal-v1",
        None,
        switcher_core::checks::unix_now(),
    )
    .map_err(|e| e.to_string())?;
    run(&mut publisher).map_err(|e| e.to_string())
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
pub fn import_ad_image(state: Desktop<'_>, bytes: Vec<u8>) -> Result<ImportedImage, String> {
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
    with_publisher(&state, |p| {
        let asset = p.import_image(&bytes, mime)?;
        let preview_data_url = p.image_preview(&asset.sha256)?;
        Ok(ImportedImage {
            asset,
            preview_data_url,
        })
    })
}
#[tauri::command]
pub fn ad_image_preview(state: Desktop<'_>, sha256: String) -> Result<String, String> {
    with_publisher(&state, |p| p.image_preview(&sha256))
}
#[tauri::command]
pub fn publisher_status(state: Desktop<'_>) -> Result<PublisherView, String> {
    with_publisher(&state, |p| {
        Ok(publisher_view(p.status(), p.configured_public_key().is_some()))
    })
}
fn publisher_view(status: publisher::Status, has_signing_key: bool) -> PublisherView {
        let key_status = if has_signing_key {
            "签名公钥已配置。"
        } else {
            "尚未生成广告签名密钥。"
        };
        let pending_status = status
            .pending_release_id
            .as_ref()
            .map_or_else(String::new, |id| {
                format!(" 未完成发布：{id}；重新发布时先核对本机结果。")
            });
        PublisherView {
            source: Some("本机私有目录".into()),
            status: if status.recovery_required {
                "recovery_required"
            } else if status.phase.is_some() {
                "pending"
            } else if !has_signing_key {
                "wait_signing_key"
            } else {
                "ready_local"
            }
            .into(),
            message: format!(
                "本地草稿可用。{key_status}{pending_status} 本机预览与正式目录已可运营；不会发布到远程用户。"
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
fn publication_outcome(result: publisher::Result<publisher::Publication>) -> publisher::Result<OperationOutcome> {
    result?;
    Ok(bridge::completed("本机签名内容及素材已通过回读验证"))
}
#[cfg(test)]
mod f16_feedback_tests {
    use super::*;
    use publisher::{ResolutionKind, ResolutionSummary};

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
        for kind in [ResolutionKind::Superseded, ResolutionKind::ExpiredCommitted, ResolutionKind::CommittedAssetsUnavailable] {
            let result = publication_outcome(Err(publisher::Error::Resolved(resolution(kind)))).unwrap();
            assert!(!result.completed);
            assert!(result.message.contains("synthetic-pending"));
            assert!(result.message.contains("未记入已验证历史"));
            assert!(result.recovery.is_some());
        }
    }
    #[test]
    fn only_verified_publication_is_completed() {
        let result = publication_outcome(Ok(publisher::Publication {
            release_id: "synthetic-verified".into(), revision: 3, channel: AdChannel::Preview,
        })).unwrap();
        assert!(result.completed);
        assert!(result.message.contains("回读验证"));
    }
    #[test]
    fn ordinary_publish_errors_remain_errors() {
        for error in [publisher::Error::PendingOperation, publisher::Error::ResolutionOccupied, publisher::Error::Store(publisher::protocol::StoreError::Unavailable)] {
            assert!(publication_outcome(Err(error)).is_err());
        }
    }
    #[test]
    fn status_keeps_recovery_and_pending_priority_without_inventing_history() {
        let view = publisher_view(status(), true);
        assert_eq!(view.status, "resolution_recorded");
        assert!(view.message.contains("synthetic-pending"));
        assert!(view.history.is_empty());
        let mut pending = status();
        pending.phase = Some(publisher::protocol::Phase::CommitUnknown);
        pending.pending_release_id = Some("synthetic-second".into());
        assert_eq!(publisher_view(pending.clone(), true).status, "pending");
        pending.recovery_required = true;
        assert_eq!(publisher_view(pending, true).status, "recovery_required");
        assert_eq!(publisher_view(status(), false).status, "wait_signing_key");
    }
}
#[tauri::command]
pub async fn publish_ad(
    state: Desktop<'_>,
    channel: AdChannel,
) -> Result<OperationOutcome, String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        with_publisher(&state, |p| {
            let public_key = p
                .configured_public_key()
                .ok_or(publisher::Error::MissingSigningKey)?;
            let trust = switcher_core::ad_runtime::local::LocalTrust {
                schema: 1,
                key_id: "ads-personal-v1".into(),
                public_key,
            };
            let mut source = LocalObjectStore::open(&state.root, channel, &trust)?;
            publication_outcome(p.publish(
                channel,
                switcher_core::checks::unix_now(),
                Some(&mut source),
                &KeychainVault,
            ))
        })
    })
    .await
    .map_err(|_| "发布操作被中断".to_owned())?
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
pub fn create_ad_signing_key(state: Desktop<'_>) -> Result<OperationOutcome, String> {
    with_publisher(&state, |p| {
        let key = p.generate_signing_key(&mut KeychainVault, Vec::new(), Vec::new())?;
        let hex: String = key.iter().map(|byte| format!("{byte:02x}")).collect();
        Ok(bridge::completed(format!(
            "广告签名私钥已保存到本机钥匙串。公开验证公钥：{hex}"
        )))
    })
}
#[tauri::command]
pub async fn export_publisher_backup(
    state: Desktop<'_>,
    passphrase: String,
) -> Result<OperationOutcome, String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let passphrase = zeroize::Zeroizing::new(passphrase);
        with_publisher(&state, |p| {
            let bytes = p.export_backup(passphrase.as_bytes(), &KeychainVault)?;
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
    bytes: Vec<u8>,
    passphrase: String,
) -> Result<OperationOutcome, String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        restore_publisher_backup_body(&bytes, passphrase, |bytes, passphrase| {
            with_publisher(&state, |p| {
                p.restore_backup(bytes, passphrase, &mut KeychainVault)
            })
        })
    })
    .await
    .map_err(|_| "恢复操作被中断".to_owned())?
}

pub(crate) fn restore_publisher_backup_body(
    bytes: &[u8],
    passphrase: String,
    restore: impl FnOnce(&[u8], &[u8]) -> Result<(), String>,
) -> Result<OperationOutcome, String> {
    if bytes.len() > publisher::backup::LIMIT {
        return Err("备份文件超过 64 MiB 上限".into());
    }
    let passphrase = zeroize::Zeroizing::new(passphrase);
    restore(bytes, passphrase.as_bytes())?;
    Ok(bridge::waiting(
        "已恢复加密备份中的本地资料",
        "再次发布到本机正式目录时，将先回读最高序号，并补齐缺失素材",
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
