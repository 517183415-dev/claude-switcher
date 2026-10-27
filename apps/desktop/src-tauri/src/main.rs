#![cfg_attr(
    any(feature = "ipc-limits", feature = "ipc-business"),
    allow(dead_code)
)]

#[cfg(all(feature = "personal", feature = "public"))]
compile_error!("Select exactly one edition");
#[cfg(not(any(feature = "personal", feature = "public")))]
compile_error!("Select an edition");
#[cfg(any(
    all(feature = "ipc-smoke", feature = "ipc-limits"),
    all(feature = "ipc-smoke", feature = "ipc-business"),
    all(feature = "ipc-limits", feature = "ipc-business")
))]
compile_error!("Select exactly one IPC test fixture");

#[cfg(feature = "ipc-business")]
mod ipc_business;
#[cfg(feature = "ipc-limits")]
mod ipc_limits;

mod bridge;
mod commands;
mod native_probe;
#[cfg(feature = "personal")]
mod publisher;
#[cfg(feature = "personal")]
mod publisher_commands;
#[cfg(feature = "personal")]
mod publisher_recovery_commands;
mod update_runtime;

#[cfg(feature = "ipc-smoke")]
#[tauri::command]
fn forbidden_probe() {
    panic!("ACL failure: forbidden command reached Rust");
}

#[cfg(feature = "ipc-smoke")]
#[tauri::command]
fn finish_smoke(window: tauri::WebviewWindow, ok: bool, denied: bool, rendered: bool) {
    println!(
        "E01_WEBVIEW_RESULT {}",
        serde_json::json!({
            "ok": ok, "custom_command_denied": denied, "three_slots_rendered": rendered,
            "ui_to_rust_ipc": true, "visible": window.is_visible().unwrap_or(false),
            "width": window.inner_size().map(|s| s.width).unwrap_or(0),
            "height": window.inner_size().map(|s| s.height).unwrap_or(0)
        })
    );
}

#[cfg(feature = "ipc-limits")]
fn main() {
    ipc_limits::run();
}

#[cfg(feature = "ipc-business")]
fn main() {
    ipc_business::run();
}

#[cfg(not(any(feature = "ipc-limits", feature = "ipc-business")))]
fn main() {
    let edition = if cfg!(feature = "public") {
        switcher_domain::Edition::Public
    } else {
        switcher_domain::Edition::Personal
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = native_probe::run(&args, edition) {
        std::process::exit(code);
    }
    if args.first().is_some_and(|arg| arg == "--repair-cli") {
        let result =
            switcher_core::repair_cli::RepairCommand::parse(&args[1..]).and_then(|command| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|_| "repair_runtime_unavailable".to_owned())?
                    .block_on(switcher_core::repair_cli::run(command, edition))
            });
        let code = match result {
            Ok(code) => code,
            Err(error) => {
                let _ = switcher_core::repair_cli::print_json(
                    &serde_json::json!({"schema_version":1,"error":error,"recovery":"按错误提示检查正式安装和修复状态"}),
                );
                2
            }
        };
        std::process::exit(code);
    }
    if args
        .first()
        .is_some_and(|arg| arg == "--update-task" || arg == "--update-manual")
    {
        let action = match args.get(1).map(String::as_str) {
            Some("check") => switcher_core::updates::UpdateAction::Check,
            Some("download") => switcher_core::updates::UpdateAction::Download,
            Some("install") => switcher_core::updates::UpdateAction::Install,
            _ => std::process::exit(64),
        };
        if args.len() != 2 {
            std::process::exit(64);
        }
        let root = switcher_core::config::home_dir()
            .expect("user home")
            .join(".claude-switcher");
        let outcome = update_runtime::UpdateRuntime::new(root, edition)
            .run_task(action, args[0] == "--update-manual");
        println!(
            "{}",
            serde_json::to_string(&outcome).expect("operation result")
        );
        std::process::exit(if outcome.completed { 0 } else { 78 });
    }
    if args == ["--version"] {
        println!(
            "{} {}",
            env!("CARGO_PKG_VERSION"),
            bridge::edition_name(edition)
        );
        return;
    }
    if !args.is_empty() {
        std::process::exit(64);
    }
    let state =
        std::sync::Arc::new(bridge::DesktopState::new(edition).expect("desktop data context"));
    #[cfg(not(feature = "ipc-smoke"))]
    let background = state.clone();
    let builder = tauri::Builder::default().manage(state).setup(move |_| {
        #[cfg(not(feature = "ipc-smoke"))]
        tauri::async_runtime::spawn(async move {
            loop {
                let state = background.clone();
                let _ = tokio::task::spawn_blocking(move || state.refresh_ads(false)).await;
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            }
        });
        Ok(())
    });
    #[cfg(feature = "personal")]
    let builder = builder.manage(std::sync::Arc::new(
        publisher_recovery_commands::RecoverySessions::default(),
    ));
    #[cfg(feature = "ipc-smoke")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::runtime_info,
        forbidden_probe,
        finish_smoke
    ]);
    #[cfg(all(not(feature = "ipc-smoke"), feature = "public"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::runtime_info,
        commands::workspace_snapshot,
        commands::check_environment,
        commands::cancel_check,
        commands::launch_checked,
        commands::account_action,
        commands::migration_preparation,
        commands::create_migration_backup,
        commands::preview_repair,
        commands::execute_repair,
        commands::cancel_repair,
        commands::recover_repair,
        commands::save_preferences,
        commands::activity_log,
        commands::export_activity,
        commands::software_update,
        commands::rebuild_time_baseline,
        commands::open_date_settings,
        commands::open_ad_link,
        commands::ad_cached_image,
        commands::refresh_ad_content,
        commands::region_view,
        commands::preview_region_change
    ]);
    #[cfg(all(not(feature = "ipc-smoke"), feature = "personal"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        commands::runtime_info,
        commands::workspace_snapshot,
        commands::check_environment,
        commands::cancel_check,
        commands::launch_checked,
        commands::account_action,
        commands::migration_preparation,
        commands::create_migration_backup,
        commands::preview_repair,
        commands::execute_repair,
        commands::cancel_repair,
        commands::recover_repair,
        commands::save_preferences,
        commands::activity_log,
        commands::export_activity,
        commands::software_update,
        commands::rebuild_time_baseline,
        commands::open_date_settings,
        commands::open_ad_link,
        commands::ad_cached_image,
        commands::refresh_ad_content,
        commands::region_view,
        commands::preview_region_change,
        publisher_commands::ad_draft_read,
        publisher_commands::ad_draft_save,
        publisher_commands::import_ad_image,
        publisher_commands::ad_image_preview,
        publisher_commands::publisher_status,
        publisher_commands::publish_ad,
        publisher_commands::publisher_remote_read,
        publisher_commands::publisher_remote_save,
        publisher_recovery_commands::publisher_recovery_prepare,
        publisher_recovery_commands::publisher_recovery_confirm,
        publisher_recovery_commands::publisher_recovery_cancel,
        publisher_recovery_commands::publisher_archive_import,
        publisher_recovery_commands::publisher_retained_list,
        publisher_recovery_commands::publisher_retained_export,
        publisher_commands::pause_ad,
        publisher_commands::restore_ad_history,
        publisher_commands::create_ad_signing_key,
        publisher_commands::export_publisher_backup,
        publisher_commands::restore_publisher_backup,
        publisher_commands::open_publisher_backup_folder,
        publisher_commands::preview_publisher_cleanup,
        publisher_commands::commit_publisher_cleanup,
        publisher_commands::preview_publisher_gc,
        publisher_commands::commit_publisher_gc
    ]);
    let mut context = tauri::generate_context!();
    let mut builder = builder;
    if let Some(pins) = update_runtime::compiled_release_pins() {
        context.config_mut().plugins.0.insert(
            "updater".into(),
            update_runtime::strict_updater_settings(pins.minisign_public_key().into()),
        );
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }
    builder.run(context).expect("desktop runtime");
}
