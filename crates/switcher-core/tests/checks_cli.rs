use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-checks-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn run(&self, args: &[&str], explicit: Option<&str>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_claude-switcher"));
        command
            .env_clear()
            .env("HOME", &self.0)
            .env("PATH", "/nonexistent")
            .args(args);
        // Deliberately poisoned ambient proxy settings must have no effect.
        command
            .env("HTTPS_PROXY", "http://ambient:ambient-password@127.0.0.1:1")
            .env("ALL_PROXY", "http://ambient:ambient-password@127.0.0.1:1");
        if let Some(proxy) = explicit {
            command.env("CLAUDE_SWITCHER_PROXY_URL", proxy);
        }
        command.output().unwrap()
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn invalid_arguments_and_config_do_not_echo_paths_or_secrets() {
    let sandbox = Sandbox::new();
    for args in [
        vec!["diagnose-proxy", "--secret", "private-password"],
        vec!["private-password"],
        vec![
            "diagnose-proxy",
            "--config",
            "/private/password-location.json",
        ],
        vec![
            "diagnose-proxy",
            "--proxy",
            "http://private-password:bad@host:bad",
        ],
    ] {
        let output = sandbox.run(&args, None);
        assert!(!output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        for private in [
            "private-password",
            "password-location",
            "http://",
            "/private/",
        ] {
            assert!(!text.contains(private));
        }
        assert!(output.stderr.is_empty());
        assert!(serde_json::from_str::<serde_json::Value>(&text).is_ok());
    }
    // Ambient proxies alone cannot authorize a diagnostic request.
    let output = sandbox.run(&["diagnose-proxy"], None);
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("explicit_proxy_required")
    );
}

#[test]
fn explicit_environment_and_private_file_need_no_curl_or_helpers_and_never_grant_launch() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new();
    let file = sandbox.0.join("private-config.json");
    std::fs::write(
        &file,
        br#"{"proxy_url":"http://account:private-password@127.0.0.1:1"}"#,
    )
    .unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    for output in [
        sandbox.run(&["diagnose-proxy"], Some("socks5h://[::1]:1")),
        sandbox.run(
            &["diagnose-proxy", "--config", file.to_str().unwrap()],
            None,
        ),
    ] {
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        for private in [
            "private-password",
            "account",
            "127.0.0.1",
            "::1",
            "ambient-password",
        ] {
            assert!(!text.contains(private));
        }
        let report: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(report["transport_reached"], false);
        assert_eq!(report["launch_allowed"], serde_json::Value::Null);
        assert_eq!(report["system_modified"], false);
        assert_eq!(report["probes"].as_array().unwrap().len(), 2);
    }
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    let readable = sandbox.run(
        &["diagnose-proxy", "--config", file.to_str().unwrap()],
        None,
    );
    assert_eq!(readable.status.code(), Some(1));
    assert!(readable.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&readable.stdout).unwrap();
    assert_eq!(report["transport_reached"], false);
    // No account snapshots or persistent configuration were created.
    assert_eq!(std::fs::read_dir(&sandbox.0).unwrap().count(), 1);
}

#[test]
fn unavailable_lifecycle_commands_never_start_or_modify_accounts() {
    let sandbox = Sandbox::new();
    for command in ["start", "switch", "fix"] {
        assert!(!sandbox.run(&[command, "invalid"], None).status.success());
    }
    assert_eq!(std::fs::read_dir(&sandbox.0).unwrap().count(), 0);
}

#[test]
fn status_is_readonly_reports_login_unknown_and_rejects_arguments() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new();
    for prefix in [
        vec![],
        vec!["--edition", "personal"],
        vec!["--edition", "public"],
    ] {
        let mut args = prefix;
        args.push("status");
        let output = sandbox.run(&args, None);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(result["login"], "unknown");
        assert_eq!(result["accounts"]["current"], "A");
        assert_eq!(result["accounts"]["a_saved"], false);
        assert_eq!(result["accounts"]["b_saved"], false);
        assert_eq!(result["accounts"]["recovery_pending"], false);
        assert_eq!(std::fs::read_dir(&sandbox.0).unwrap().count(), 0);
    }
    let root = sandbox.0.join(".claude-switcher");
    let archived = root.join("accounts/A");
    std::fs::create_dir_all(&archived).unwrap();
    for path in [&root, &root.join("accounts"), &archived] {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    std::fs::write(root.join("current"), b"B\n").unwrap();
    let output = sandbox.run(&["status"], None);
    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["accounts"]["a_saved"], true);
    assert_eq!(result["accounts"]["current"], "B");
    assert_eq!(std::fs::read(root.join("current")).unwrap(), b"B\n");
    assert!(!root.join("v2").exists());
    for args in [vec!["status", "private-secret"], vec!["status", "--apply"]] {
        let output = sandbox.run(&args, None);
        assert!(!output.status.success());
        assert!(
            !String::from_utf8(output.stdout)
                .unwrap()
                .contains("private-secret")
        );
        assert!(output.stderr.is_empty());
    }
    assert!(!root.join("v2").exists());
}
