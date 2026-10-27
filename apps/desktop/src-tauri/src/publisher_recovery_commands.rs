//! Personal-only, explicitly confirmed recovery; never publishes remotely.
use crate::{
    bridge::{self, DesktopState},
    publisher::{
        self,
        local_store::LocalObjectStore,
        migration::recovery::{PreparedRecovery, RecoveryPreview},
    },
    publisher_commands::{self, PublisherVault},
};
use std::sync::{Arc, Mutex};
use switcher_domain::{ads::AdChannel, desktop::OperationOutcome};
use tauri::State;

#[derive(Default)]
struct SessionState {
    epoch: u64,
    preparing: bool,
    prepared: Option<PreparedRecovery>,
}
#[derive(Default)]
pub(crate) struct RecoverySessions(Mutex<SessionState>);
type Sessions<'a> = State<'a, Arc<RecoverySessions>>;

#[tauri::command]
pub async fn publisher_retained_list(
    state: State<'_, Arc<DesktopState>>,
) -> Result<Vec<publisher::migration::RetainedView>, String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        publisher_commands::with_publisher(&state, |p| p.retained_views())
    })
    .await
    .map_err(|_| "保留资料读取被中断".to_owned())?
}

#[tauri::command]
pub async fn publisher_retained_export(
    state: State<'_, Arc<DesktopState>>,
    vault: PublisherVault<'_>,
    generation: String,
    passphrase: String,
) -> Result<OperationOutcome, String> {
    let passphrase = zeroize::Zeroizing::new(passphrase);
    let state = state.inner().clone();
    let vault = vault.inner().clone();
    tokio::task::spawn_blocking(move || {
        publisher_commands::with_publisher_vault(&state, &vault, |p, vault| {
            let bytes = p.export_retained_generation(&generation, passphrase.as_bytes(), vault)?;
            publisher::migration::recovery::save_retained_archive(p, &generation, &bytes)?;
            Ok(bridge::completed(
                "保留资料已加密导出到备份目录；原资料和当前代次继续保留。",
            ))
        })
    })
    .await
    .map_err(|_| "保留资料导出被中断".to_owned())?
}

#[tauri::command]
pub async fn publisher_recovery_prepare(
    state: State<'_, Arc<DesktopState>>,
    vault: PublisherVault<'_>,
    sessions: Sessions<'_>,
    passphrase: String,
) -> Result<RecoveryPreview, String> {
    let passphrase = zeroize::Zeroizing::new(passphrase.into_bytes());
    let sessions = sessions.inner().clone();
    let epoch = {
        let mut guard = sessions
            .0
            .lock()
            .map_err(|_| "恢复会话不可用，请重启应用")?;
        if guard.preparing || guard.prepared.is_some() {
            return Err("已有恢复会话，请先取消或确认".into());
        }
        guard.epoch = guard.epoch.wrapping_add(1);
        guard.preparing = true;
        guard.epoch
    };
    let state = state.inner().clone();
    let vault = vault.inner().clone();
    tokio::task::spawn_blocking(move || {
        let result = (|| {
            let publisher = publisher_commands::open_publisher(&state)?;
            let trust = switcher_core::ad_runtime::local::LocalTrust {
                schema: 1,
                key_id: "ads-personal-v1".into(),
                public_key: publisher
                    .configured_public_key()
                    .ok_or(publisher::Error::MissingSigningKey)
                    .map_err(|e| e.to_string())?,
            };
            let source = LocalObjectStore::open(&state.root, AdChannel::Preview, &trust)
                .map_err(|e| e.to_string())?;
            vault
                .with(|vault| {
                    PreparedRecovery::prepare(
                        publisher,
                        source,
                        passphrase,
                        vault,
                        switcher_core::checks::unix_now(),
                    )
                })
                .map_err(|e| e.to_string())
        })();
        let mut guard = sessions
            .0
            .lock()
            .map_err(|_| "恢复会话不可用，请重启应用")?;
        if epoch != guard.epoch {
            return Err("恢复准备已取消；已生成的备份会继续保留".into());
        }
        guard.preparing = false;
        let prepared = result?;
        let preview = prepared.preview();
        guard.prepared = Some(prepared);
        Ok(preview)
    })
    .await
    .map_err(|_| "恢复准备被中断，请取消会话后重试".to_owned())?
}

#[tauri::command]
pub async fn publisher_recovery_confirm(
    vault: PublisherVault<'_>,
    sessions: Sessions<'_>,
    digest: String,
) -> Result<OperationOutcome, String> {
    let session = sessions
        .0
        .lock()
        .map_err(|_| "恢复会话不可用，请重启应用")?
        .prepared
        .take()
        .ok_or("恢复会话不存在，请重新准备")?;
    let vault = vault.inner().clone();
    tokio::task::spawn_blocking(move || {
        let view = vault.with(|vault| session.confirm(&digest, switcher_core::checks::unix_now(), vault)).map_err(|e| e.to_string())?;
        Ok(bridge::completed(format!("已建立新的本机发布代次。旧资料和旧密钥继续保留。完整加密备份：{}。尚未发布，请核对草稿后明确发布。", view.backup_filename)))
    }).await.map_err(|_| "恢复确认被中断，请重新读取状态；不要假定切换失败".to_owned())?
}

#[tauri::command]
pub fn publisher_recovery_cancel(sessions: Sessions<'_>) -> Result<(), String> {
    let mut guard = sessions
        .0
        .lock()
        .map_err(|_| "恢复会话不可用，请重启应用")?;
    guard.epoch = guard.epoch.wrapping_add(1);
    guard.preparing = false;
    guard.prepared = None;
    Ok(())
}

#[tauri::command]
pub async fn publisher_archive_import(
    state: State<'_, Arc<DesktopState>>,
    vault: PublisherVault<'_>,
    bytes: Vec<u8>,
    passphrase: String,
) -> Result<OperationOutcome, String> {
    let passphrase = zeroize::Zeroizing::new(passphrase);
    if bytes.len() > publisher::backup::LIMIT {
        return Err("备份文件超过 64 MiB 上限".into());
    }
    let state = state.inner().clone();
    let vault = vault.inner().clone();
    tokio::task::spawn_blocking(move || {
        publisher_commands::with_publisher_vault(&state, &vault, |p, vault| {
            p.import_retained_generation(&bytes, passphrase.as_bytes(), vault)?;
            Ok(bridge::completed(
                "完整归档已导入保留目录。当前代次保持原样；旧发布事务不会自动重放。",
            ))
        })
    })
    .await
    .map_err(|_| "归档导入被中断；原资料继续保留".to_owned())?
}
