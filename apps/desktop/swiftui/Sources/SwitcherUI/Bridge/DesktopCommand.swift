import Foundation

public enum DesktopEdition: String, Codable, Sendable {
    case personal, `public`
}

/// The native interface uses the same fixed command surface as the Tauri window.
public enum DesktopCommand: String, CaseIterable, Sendable {
    case runtimeInfo = "runtime_info"
    case workspaceSnapshot = "workspace_snapshot"
    case checkEnvironment = "check_environment"
    case cancelCheck = "cancel_check"
    case launchChecked = "launch_checked"
    case accountAction = "account_action"
    case migrationPreparation = "migration_preparation"
    case createMigrationBackup = "create_migration_backup"
    case previewRepair = "preview_repair"
    case executeRepair = "execute_repair"
    case cancelRepair = "cancel_repair"
    case recoverRepair = "recover_repair"
    case savePreferences = "save_preferences"
    case activityLog = "activity_log"
    case exportActivity = "export_activity"
    case softwareUpdate = "software_update"
    case rebuildTimeBaseline = "rebuild_time_baseline"
    case openDateSettings = "open_date_settings"
    case openAdLink = "open_ad_link"
    case adCachedImage = "ad_cached_image"
    case refreshAdContent = "refresh_ad_content"
    case regionView = "region_view"
    case previewRegionChange = "preview_region_change"
    case adDraftRead = "ad_draft_read"
    case adDraftSave = "ad_draft_save"
    case importAdImage = "import_ad_image"
    case adImagePreview = "ad_image_preview"
    case publisherStatus = "publisher_status"
    case publishAd = "publish_ad"
    case publisherRemoteRead = "publisher_remote_read"
    case publisherRemoteSave = "publisher_remote_save"
    case publisherRemoteSelect = "publisher_remote_select"
    case publisherRecoveryPrepare = "publisher_recovery_prepare"
    case publisherRecoveryConfirm = "publisher_recovery_confirm"
    case publisherRecoveryCancel = "publisher_recovery_cancel"
    case publisherArchiveImport = "publisher_archive_import"
    case publisherRetainedList = "publisher_retained_list"
    case publisherRetainedExport = "publisher_retained_export"
    case pauseAd = "pause_ad"
    case restoreAdHistory = "restore_ad_history"
    case createAdSigningKey = "create_ad_signing_key"
    case exportPublisherBackup = "export_publisher_backup"
    case restorePublisherBackup = "restore_publisher_backup"
    case openPublisherBackupFolder = "open_publisher_backup_folder"
    case previewPublisherCleanup = "preview_publisher_cleanup"
    case commitPublisherCleanup = "commit_publisher_cleanup"
    case previewPublisherGC = "preview_publisher_gc"
    case commitPublisherGC = "commit_publisher_gc"

    public func isAvailable(in edition: DesktopEdition) -> Bool {
        if edition == .personal { return true }
        switch self {
        case .runtimeInfo, .workspaceSnapshot, .checkEnvironment, .cancelCheck,
             .launchChecked, .accountAction, .migrationPreparation, .createMigrationBackup,
             .previewRepair, .executeRepair, .cancelRepair, .recoverRepair,
             .savePreferences, .activityLog, .exportActivity, .softwareUpdate,
             .rebuildTimeBaseline, .openDateSettings, .openAdLink, .adCachedImage,
             .refreshAdContent, .regionView, .previewRegionChange:
            return true
        default:
            return false
        }
    }
}
