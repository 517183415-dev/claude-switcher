//! Compile-time-only IPC fixture. No DesktopState, Vault, system or background work.
use serde_json::{Value, json};
use std::{io::Write, sync::Mutex};

#[cfg(feature = "personal")]
fn create_fixture_directory(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(path)
}

struct Fixture {
    preflight: bool,
    receipts: Mutex<Vec<Value>>,
    #[cfg(feature = "personal")]
    publisher: Mutex<crate::publisher::Publisher>,
}

fn record(fixture: &Fixture, receipt: Value) -> Value {
    fixture
        .receipts
        .lock()
        .expect("fixture receipts")
        .push(receipt.clone());
    println!("A01_IPC_EVENT {receipt}");
    std::io::stdout().flush().expect("fixture stdout");
    receipt
}

#[tauri::command]
fn ipc_limits_info(fixture: tauri::State<'_, Fixture>) -> Value {
    json!({
        "edition": if cfg!(feature = "personal") { "personal" } else { "public" },
        "preflight": fixture.preflight,
        "fixture": "ipc-limits",
        "ordinary_runtime": false,
    })
}

#[cfg(feature = "personal")]
#[tauri::command]
fn ipc_limits_image(fixture: tauri::State<'_, Fixture>, bytes: Vec<u8>) -> Value {
    let input_len = bytes.len();
    let mut entered = 0;
    let result = crate::publisher_commands::import_ad_image_body(bytes, |bytes, mime| {
        entered += 1;
        let mut publisher = fixture.publisher.lock().expect("fixture publisher");
        let asset = publisher
            .import_image(bytes, mime)
            .map_err(|e| e.to_string())?;
        let preview_data_url = publisher
            .image_preview(&asset.sha256)
            .map_err(|e| e.to_string())?;
        Ok(switcher_domain::desktop::ImportedImage {
            asset,
            preview_data_url,
        })
    });
    record(
        &fixture,
        json!({
            "kind": "image", "input_len": input_len, "entered": entered,
            "error": result.as_ref().err(),
            "asset": result.as_ref().ok().map(|image| &image.asset),
        }),
    )
}

#[cfg(feature = "personal")]
#[tauri::command]
fn ipc_limits_backup(
    fixture: tauri::State<'_, Fixture>,
    bytes: Vec<u8>,
    passphrase: String,
) -> Value {
    let input_len = bytes.len();
    let mut entered = 0;
    let result =
        crate::publisher_commands::restore_publisher_backup_body(&bytes, passphrase, |_, _| {
            entered += 1;
            Err("acceptance_restore_entered".into())
        });
    record(
        &fixture,
        json!({
            "kind": "backup", "input_len": input_len, "entered": entered,
            "error": result.err(),
            "decryption_or_vault": false,
        }),
    )
}

#[tauri::command]
fn ipc_limits_forbidden() -> Result<(), String> {
    Err("fixture_acl_unexpectedly_allowed".into())
}

#[tauri::command]
fn ipc_limits_finish(
    app: tauri::AppHandle,
    fixture: tauri::State<'_, Fixture>,
    ok: bool,
    denials: Vec<Value>,
    error: Option<String>,
) {
    println!(
        "A01_IPC_RESULT {}",
        json!({
            "ok": ok, "denials": denials, "error": error,
            "receipts": *fixture.receipts.lock().expect("fixture receipts"),
            "preflight": fixture.preflight,
            "edition": if cfg!(feature = "personal") { "personal" } else { "public" },
            "ordinary_runtime": false,
        })
    );
    std::io::stdout().flush().expect("fixture stdout");
    app.exit(if ok { 0 } else { 1 });
}

pub fn run() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 1 || !matches!(args[0].as_str(), "--preflight" | "--bounds") {
        eprintln!("ipc-limits requires --preflight or --bounds");
        std::process::exit(64);
    }
    let fixture = Fixture {
        preflight: args[0] == "--preflight",
        receipts: Mutex::new(Vec::new()),
        #[cfg(feature = "personal")]
        publisher: {
            // A new caller-owned temporary root only; never resolve the user's home.
            let root = std::env::temp_dir().join(format!("ipc-limits-{}", std::process::id()));
            create_fixture_directory(&root).expect("new fixture root");
            Mutex::new(
                crate::publisher::Publisher::open(&root, "fixture", None, 1_700_000_000)
                    .expect("synthetic publisher"),
            )
        },
    };
    let builder = tauri::Builder::default().manage(fixture);
    #[cfg(feature = "personal")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        ipc_limits_info,
        ipc_limits_image,
        ipc_limits_backup,
        ipc_limits_forbidden,
        ipc_limits_finish
    ]);
    #[cfg(feature = "public")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        ipc_limits_info,
        ipc_limits_forbidden,
        ipc_limits_finish
    ]);
    builder
        .run(tauri::generate_context!())
        .expect("isolated IPC runtime");
}

#[cfg(all(test, feature = "personal"))]
mod tests {
    use super::create_fixture_directory;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn fixture_directory_is_private_and_never_reused() {
        let root = std::env::temp_dir().join(format!(
            "ipc-private-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        create_fixture_directory(&root).unwrap();
        assert_eq!(
            std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let marker = root.join("synthetic-marker");
        std::fs::write(&marker, b"preserve").unwrap();
        assert_eq!(
            create_fixture_directory(&root).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(marker).unwrap(), b"preserve");
        crate::publisher::Publisher::open(&root, "fixture", None, 1_700_000_000).unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
