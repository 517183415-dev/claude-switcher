fn main() {
    let commands: &[&str] = if std::env::var_os("CARGO_FEATURE_IPC_BUSINESS").is_some() {
        &[
            "activity_log",
            "export_activity",
            "ipc_business_info",
            "ipc_business_finish",
            "ipc_business_forbidden",
        ]
    } else if std::env::var_os("CARGO_FEATURE_IPC_LIMITS").is_some() {
        if std::env::var_os("CARGO_FEATURE_PERSONAL").is_some() {
            &[
                "ipc_limits_info",
                "ipc_limits_image",
                "ipc_limits_backup",
                "ipc_limits_forbidden",
                "ipc_limits_finish",
            ]
        } else {
            &[
                "ipc_limits_info",
                "ipc_limits_forbidden",
                "ipc_limits_finish",
            ]
        }
    } else if std::env::var_os("CARGO_FEATURE_IPC_SMOKE").is_some() {
        &["runtime_info", "forbidden_probe", "finish_smoke"]
    } else if std::env::var_os("CARGO_FEATURE_PERSONAL").is_some() {
        &[
            "runtime_info",
            "workspace_snapshot",
            "check_environment",
            "cancel_check",
            "launch_checked",
            "account_action",
            "migration_preparation",
            "create_migration_backup",
            "preview_repair",
            "execute_repair",
            "cancel_repair",
            "recover_repair",
            "save_preferences",
            "activity_log",
            "export_activity",
            "software_update",
            "rebuild_time_baseline",
            "open_date_settings",
            "open_ad_link",
            "ad_cached_image",
            "refresh_ad_content",
            "region_view",
            "preview_region_change",
            "ad_draft_read",
            "ad_draft_save",
            "import_ad_image",
            "ad_image_preview",
            "publisher_status",
            "publish_ad",
            "pause_ad",
            "restore_ad_history",
            "create_ad_signing_key",
            "export_publisher_backup",
            "restore_publisher_backup",
            "open_publisher_backup_folder",
            "preview_publisher_cleanup",
            "commit_publisher_cleanup",
            "preview_publisher_gc",
            "commit_publisher_gc",
        ]
    } else {
        &[
            "runtime_info",
            "workspace_snapshot",
            "check_environment",
            "cancel_check",
            "launch_checked",
            "account_action",
            "migration_preparation",
            "create_migration_backup",
            "preview_repair",
            "execute_repair",
            "cancel_repair",
            "recover_repair",
            "save_preferences",
            "activity_log",
            "export_activity",
            "software_update",
            "rebuild_time_baseline",
            "open_date_settings",
            "open_ad_link",
            "ad_cached_image",
            "refresh_ad_content",
            "region_view",
            "preview_region_change",
        ]
    };
    let manifest = tauri_build::AppManifest::new().commands(commands);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .expect("Tauri manifest");
}
