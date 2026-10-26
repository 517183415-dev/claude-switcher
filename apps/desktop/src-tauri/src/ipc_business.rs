//! Feature-only native fixture for real activity and preferences commands. No ordinary startup.
use crate::bridge::DesktopState;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::Arc,
};
use switcher_core::storage;
use switcher_domain::{Edition, desktop::ActivityEntry};

struct Fixture {
    preflight: bool,
    preferences: bool,
    persistence: Option<&'static str>,
}

#[tauri::command]
fn ipc_business_info(
    fixture: tauri::State<'_, Fixture>,
    state: tauri::State<'_, Arc<DesktopState>>,
) -> Result<Value, String> {
    let saved = if fixture.persistence == Some("read") {
        Some(state.preferences()?)
    } else {
        None
    };
    Ok(
        json!({"fixture": "ipc-business", "preflight": fixture.preflight,
        "edition": if cfg!(feature = "personal") { "personal" } else { "public" },
        "ordinary_runtime": false, "preferences": fixture.preferences, "persistence": fixture.persistence, "saved": saved}),
    )
}

#[tauri::command]
fn ipc_business_forbidden() -> Result<(), String> {
    Err("fixture_acl_unexpectedly_allowed".into())
}

#[tauri::command]
fn ipc_business_preferences_snapshot(
    state: tauri::State<'_, Arc<DesktopState>>,
    label: String,
) -> Result<(), String> {
    if label.len() > 80 || !label.is_ascii() {
        return Err("invalid_snapshot_label".into());
    }
    let read = |name: &str| -> Result<Option<Vec<u8>>, String> {
        let file = match std::fs::File::open(state.directory().join(name)) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("snapshot_open".into()),
        };
        let mut bytes = Vec::new();
        file.take(65537)
            .read_to_end(&mut bytes)
            .map_err(|_| "snapshot_read")?;
        if bytes.len() > 65536 {
            return Err("snapshot_size".into());
        }
        Ok(Some(bytes))
    };
    println!(
        "BUSINESS_IPC_PREFERENCES {}",
        json!({
            "label": label, "preferences": read("preferences.json")?, "activity": read("activity.json")?
        })
    );
    std::io::stdout().flush().map_err(|_| "snapshot_stdout")?;
    Ok(())
}

#[tauri::command]
fn ipc_business_finish(
    app: tauri::AppHandle,
    fixture: tauri::State<'_, Fixture>,
    ok: bool,
    responses: Value,
    denials: Vec<Value>,
    error: Option<String>,
) {
    println!(
        "BUSINESS_IPC_RESULT {}",
        json!({
            "fixture": "ipc-business", "ordinary_runtime": false,
            "edition": if cfg!(feature = "personal") { "personal" } else { "public" },
            "preflight": fixture.preflight, "ok": ok, "responses": responses,
            "denials": denials, "error": error,
        })
    );
    std::io::stdout().flush().expect("fixture stdout");
    app.exit(if ok { 0 } else { 1 });
}

fn synthetic_root() -> PathBuf {
    let tmp = std::env::temp_dir().canonicalize().expect("fixture TMPDIR");
    let root = PathBuf::from(std::env::var_os("IPC_BUSINESS_ROOT").expect("explicit fixture root"));
    assert_eq!(
        root,
        tmp.join("business-root"),
        "root must be inside isolated TMPDIR"
    );
    storage::validate_no_symlinks(&root).expect("fixture root must not contain links");
    storage::create_private_directory(&root).expect("private fixture root");
    root
}

fn seed_activity(state: &DesktopState) {
    // Exercise real persistence first, then append historical raw input to verify read redaction.
    let home = std::env::var("HOME").expect("isolated HOME");
    let raw = format!(
        "正常业务\nIP 203.0.113.9\nIPv6 2001:db8::1\nhttps://private.example/path\nCookie: BUSINESS_SYNTHETIC_ONLY\n路径 {home}/synthetic-profile"
    );
    state
        .log("info", &raw)
        .expect("normal redacted persistence");
    let path = state.directory().join("activity.json");
    let mut entries: Vec<ActivityEntry> = storage::read_json(&path).expect("persisted activity");
    assert_eq!(entries.len(), 1);
    entries.push(ActivityEntry {
        observed_at: 1,
        level: "warning".into(),
        message: raw,
    });
    storage::atomic_json(&path, &entries).expect("synthetic historical raw activity");
}

pub fn run() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1
        || !matches!(
            args[0].as_str(),
            "--preflight"
                | "--business"
                | "--preferences"
                | "--persistence-write"
                | "--persistence-read"
        )
    {
        eprintln!("ipc-business requires one reviewed fixture mode");
        std::process::exit(64);
    }
    let preflight = args[0] == "--preflight";
    let persistence = match args[0].as_str() {
        "--persistence-write" => Some("write"),
        "--persistence-read" => Some("read"),
        _ => None,
    };
    let edition = if cfg!(feature = "personal") {
        Edition::Personal
    } else {
        Edition::Public
    };
    let state = DesktopState::isolated_activity_fixture(synthetic_root(), edition);
    if !preflight && persistence.is_none() {
        seed_activity(&state);
    }
    tauri::Builder::default()
        .manage(Arc::new(state))
        .manage(Fixture {
            preflight,
            persistence,
            preferences: args[0] == "--preferences",
        })
        .invoke_handler(tauri::generate_handler![
            crate::commands::activity_log,
            crate::commands::export_activity,
            crate::commands::save_preferences,
            ipc_business_preferences_snapshot,
            ipc_business_info,
            ipc_business_finish,
            ipc_business_forbidden
        ])
        .run(tauri::generate_context!())
        .expect("isolated business IPC runtime");
}
