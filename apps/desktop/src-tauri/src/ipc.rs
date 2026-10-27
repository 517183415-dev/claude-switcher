//! The ordinary product and its dispatch tests share this command registry.
use crate::commands;
#[cfg(feature = "personal")]
use crate::{publisher_commands, publisher_recovery_commands};

macro_rules! common {
    ($consumer:ident; $($extra:path),* $(,)?) => {
        $consumer![
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
            $($extra),*
        ]
    };
}

#[cfg(any(feature = "personal", test))]
macro_rules! personal {
    ($consumer:ident) => {
        common![$consumer;
            publisher_commands::ad_draft_read,
            publisher_commands::ad_draft_save,
            publisher_commands::import_ad_image,
            publisher_commands::ad_image_preview,
            publisher_commands::publisher_status,
            publisher_commands::publish_ad,
            publisher_commands::publisher_remote_read,
            publisher_commands::publisher_remote_save,
            publisher_commands::publisher_remote_select,
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
            publisher_commands::commit_publisher_gc,
        ]
    };
}

macro_rules! dispatch {
    ($($command:path),* $(,)?) => { tauri::generate_handler![$($command),*] };
}

pub(crate) fn handler<R: tauri::Runtime>()
-> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    #[cfg(feature = "personal")]
    {
        personal![dispatch]
    }
    #[cfg(feature = "public")]
    {
        common![dispatch;]
    }
}

#[cfg(test)]
pub(crate) fn registered_names(is_personal: bool) -> std::collections::BTreeSet<String> {
    macro_rules! names {
        ($($command:path),* $(,)?) => {
            [$(stringify!($command).rsplit("::").next().unwrap().trim().to_owned()),*].into_iter().collect()
        };
    }
    if is_personal {
        personal![names]
    } else {
        common![names;]
    }
}
