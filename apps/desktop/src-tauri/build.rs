fn main() {
    if std::env::var_os("CARGO_FEATURE_NATIVE_UI").is_some() {
        build_native_ui();
    }
    let commands: &[&str] = if std::env::var_os("CARGO_FEATURE_IPC_BUSINESS").is_some()
        && std::env::var_os("CARGO_FEATURE_NATIVE_UI").is_none()
    {
        if std::env::var_os("CARGO_FEATURE_PERSONAL").is_some() {
            &[
                "import_ad_image",
                "ad_image_preview",
                "ad_draft_read",
                "ad_draft_save",
                "pause_ad",
                "ipc_business_draft_snapshot",
                "ipc_business_publish_init",
                "ipc_business_publish",
                "publisher_status",
                "restore_ad_history",
                "preview_publisher_cleanup",
                "commit_publisher_cleanup",
                "preview_publisher_gc",
                "commit_publisher_gc",
                "ipc_business_backup_export",
                "ipc_business_backup_restore",
                "activity_log",
                "export_activity",
                "save_preferences",
                "ipc_business_preferences_snapshot",
                "ipc_business_info",
                "ipc_business_finish",
                "ipc_business_forbidden",
            ]
        } else {
            &[
                "ipc_business_publish_init",
                "ipc_business_publish",
                "publisher_status",
                "restore_ad_history",
                "preview_publisher_cleanup",
                "commit_publisher_cleanup",
                "preview_publisher_gc",
                "commit_publisher_gc",
                "activity_log",
                "export_activity",
                "save_preferences",
                "ipc_business_preferences_snapshot",
                "ipc_business_info",
                "ipc_business_finish",
                "ipc_business_forbidden",
            ]
        }
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
            "publisher_remote_read",
            "publisher_remote_save",
            "publisher_remote_select",
            "publisher_recovery_prepare",
            "publisher_recovery_confirm",
            "publisher_recovery_cancel",
            "publisher_archive_import",
            "publisher_retained_list",
            "publisher_retained_export",
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

fn build_native_ui() {
    use std::{path::Path, process::Command};

    fn collect(directory: &Path, sources: &mut Vec<std::path::PathBuf>) {
        println!("cargo:rerun-if-changed={}", directory.display());
        for item in std::fs::read_dir(directory).expect("SwiftUI sources") {
            let path = item.expect("SwiftUI source entry").path();
            if path.is_dir() {
                collect(&path, sources);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "swift")
            {
                sources.push(path);
            }
        }
    }

    let target = std::env::var("TARGET").expect("Rust target");
    let architecture = match target.as_str() {
        "aarch64-apple-darwin" => "arm64",
        "x86_64-apple-darwin" => "x86_64",
        _ => panic!("native-ui requires a supported macOS target"),
    };
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("build output"));
    let mut sources = Vec::new();
    collect(Path::new("../swiftui/Sources/SwitcherUI"), &mut sources);
    sources.sort();
    let mut command = Command::new("/usr/bin/xcrun");
    command
        .arg("swiftc")
        .args([
            "-parse-as-library",
            "-emit-library",
            "-static",
            "-whole-module-optimization",
            "-module-name",
            "SwitcherNative",
            "-target",
        ])
        .arg(format!("{architecture}-apple-macos13.0"));
    if std::env::var_os("CARGO_FEATURE_IPC_BUSINESS").is_some() {
        command.args(["-D", "SWITCHER_NATIVE_FIXTURE"]);
    }
    let status = command
        .args(&sources)
        .arg("-o")
        .arg(output.join("libswitcher_native.a"))
        .status()
        .expect("start Swift compiler");
    assert!(status.success(), "SwiftUI static library build failed");
    println!("cargo:rustc-link-search=native={}", output.display());
    println!("cargo:rustc-link-lib=static=switcher_native");
    // Rust links with -nodefaultlibs. Swift's availability checks still need
    // the Apple compiler runtime that clang would ordinarily add itself.
    let runtime = Command::new("/usr/bin/xcrun")
        .args(["clang", "--print-libgcc-file-name"])
        .output()
        .expect("find Apple compiler runtime");
    assert!(runtime.status.success(), "find Apple compiler runtime");
    let runtime = String::from_utf8(runtime.stdout).expect("Apple compiler runtime path");
    let runtime = runtime.trim();
    assert!(
        Path::new(runtime).is_file(),
        "Apple compiler runtime missing"
    );
    println!("cargo:rustc-link-arg={runtime}");
    for framework in [
        "AppKit",
        "SwiftUI",
        "WebKit",
        "Foundation",
        "Combine",
        "UniformTypeIdentifiers",
    ] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
}
