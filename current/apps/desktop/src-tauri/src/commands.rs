use crate::bridge::{self, DesktopState};
use std::sync::Arc;
use switcher_domain::{AdSlot, checks::CheckReport, desktop::*};
use tauri::State;

type Desktop<'a> = State<'a, Arc<DesktopState>>;

#[tauri::command]
pub fn runtime_info(state: Desktop<'_>) -> switcher_domain::RuntimeInfo {
    switcher_core::runtime_info(state.edition)
}
#[tauri::command]
pub fn workspace_snapshot(state: Desktop<'_>) -> Result<WorkspaceSnapshot, String> {
    state.snapshot()
}
#[tauri::command]
pub async fn check_environment(state: Desktop<'_>) -> Result<CheckReport, String> {
    state.check().await
}
#[tauri::command]
pub fn cancel_check(state: Desktop<'_>) -> OperationOutcome {
    state.cancel_check()
}
#[tauri::command]
pub async fn launch_checked(
    state: Desktop<'_>,
    app: tauri::AppHandle,
) -> Result<OperationOutcome, String> {
    let outcome = state.launch_checked().await;
    close_after_launch(&app, &outcome);
    Ok(outcome)
}

fn close_after_launch(app: &tauri::AppHandle, outcome: &OperationOutcome) {
    if outcome.completed {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(2200)).await;
            app.exit(0);
        });
    }
}
#[tauri::command]
pub async fn migration_preparation(state: Desktop<'_>) -> Result<AccountPreparation, String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || state.migration_preparation())
        .await
        .map_err(|_| "准备状态预览被中断".to_owned())?
}
#[tauri::command]
pub async fn create_migration_backup(state: Desktop<'_>) -> Result<OperationOutcome, String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || state.create_migration_backup())
        .await
        .map_err(|_| "迁移备份操作被中断，请保留资料并重新预览准备状态".to_owned())
}
#[tauri::command]
pub async fn account_action(
    state: Desktop<'_>,
    action: AccountAction,
    app: tauri::AppHandle,
) -> Result<OperationOutcome, String> {
    let state = state.inner().clone();
    let target = match action {
        AccountAction::SelectA => Some(switcher_core::accounts::AccountId::A),
        AccountAction::SelectB => Some(switcher_core::accounts::AccountId::B),
        _ => None,
    };
    if let Some(target) = target {
        let outcome = state.switch_account(target).await;
        close_after_launch(&app, &outcome);
        return Ok(outcome);
    }
    tokio::task::spawn_blocking(move || state.account_action(action))
        .await
        .map_err(|_| "账号操作被中断".into())
}
#[tauri::command]
pub async fn preview_repair(
    state: Desktop<'_>,
    request_id: String,
) -> Result<RepairPreview, String> {
    let state = state.inner().clone();
    state.begin_repair_preview(request_id.clone())?;
    tokio::task::spawn_blocking(move || state.preview_repair(request_id))
        .await
        .map_err(|_| "修复预览被中断".into())
}
#[tauri::command]
pub async fn execute_repair(
    state: Desktop<'_>,
    request_id: String,
) -> Result<OperationOutcome, String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || state.execute_repair(&request_id))
        .await
        .map_err(|_| "修复操作被中断".into())
}
#[tauri::command]
pub async fn recover_repair(state: Desktop<'_>) -> Result<OperationOutcome, String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || state.recover_repair())
        .await
        .map_err(|_| "用户配置恢复被中断".into())
}
#[tauri::command]
pub fn save_preferences(
    state: Desktop<'_>,
    preferences: Preferences,
) -> Result<Preferences, String> {
    state.save_preferences(preferences)
}
#[tauri::command]
pub fn activity_log(state: Desktop<'_>) -> Result<Vec<ActivityEntry>, String> {
    state.activity()
}
#[tauri::command]
pub async fn export_activity(state: Desktop<'_>) -> Result<(String, Vec<ActivityEntry>), String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || state.export_activity())
        .await
        .map_err(|_| "日志导出被中断".to_owned())?
}
#[tauri::command]
pub async fn software_update(
    state: Desktop<'_>,
    action: UpdateAction,
) -> Result<OperationOutcome, String> {
    let (root, edition) = (state.root.clone(), state.edition);
    tokio::task::spawn_blocking(move || {
        crate::update_runtime::UpdateRuntime::new(root, edition).act(action)
    })
    .await
    .map_err(|_| "更新操作被中断".into())
}
#[tauri::command]
pub fn rebuild_time_baseline(state: Desktop<'_>) -> Result<OperationOutcome, String> {
    state.rebuild_time()
}
#[tauri::command]
pub fn open_date_settings() -> OperationOutcome {
    bridge::open_date_settings()
}
#[tauri::command]
pub async fn open_ad_link(
    state: Desktop<'_>,
    slot: AdSlot,
    confirmed: bool,
) -> Result<OperationOutcome, String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || state.open_ad(slot, confirmed))
        .await
        .map_err(|_| "商家链接操作被中断".into())
}
#[tauri::command]
pub fn ad_cached_image(state: Desktop<'_>, slot: AdSlot, sha256: String) -> Result<String, String> {
    state.cached_image(slot, &sha256)
}
#[tauri::command]
pub async fn refresh_ad_content(state: Desktop<'_>) -> Result<OperationOutcome, String> {
    let state = state.inner().clone();
    tokio::task::spawn_blocking(move || state.refresh_ads(true))
        .await
        .map_err(|_| "广告刷新被中断".to_owned())?
}
#[tauri::command]
pub fn region_view(state: Desktop<'_>) -> Result<RegionView, String> {
    state.region_view()
}
#[tauri::command]
pub async fn preview_region_change(
    state: Desktop<'_>,
    region: RegionPreset,
    request_id: String,
) -> Result<RepairPreview, String> {
    let state = state.inner().clone();
    state.begin_repair_preview(request_id.clone())?;
    tokio::task::spawn_blocking(move || state.preview_region(Some(region), request_id))
        .await
        .map_err(|_| "地区预览被中断".into())
}

#[tauri::command]
pub fn cancel_repair(state: Desktop<'_>, request_id: String) -> OperationOutcome {
    state.cancel_repair(&request_id)
}
