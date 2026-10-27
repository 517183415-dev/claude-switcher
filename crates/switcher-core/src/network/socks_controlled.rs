//! Account-free SOCKS5 coverage using only synthetic local endpoints.
use super::{client_builder, transport_probe};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use switcher_domain::checks::TransportState;

struct Fixture {
    root: PathBuf,
    child: Option<Child>,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn trusted_tool(name: &str, root: &Path) -> Command {
    let executable = match name {
        "openssl" => "/usr/bin/openssl",
        "python3" => "/usr/bin/python3",
        _ => panic!("unsupported test tool"),
    };
    let mut command = Command::new(executable);
    command
        .env_clear()
        .env("HOME", root.join("home"))
        .env("TMPDIR", root.join("tmp"))
        .env("LC_ALL", "C")
        .current_dir(root);
    command
}

#[test]
fn test_tools_ignore_writable_path() {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(root) = std::env::var("SWITCHER_FAKE_TOOL_ROOT") {
        let command = trusted_tool("openssl", Path::new(&root));
        assert_eq!(command.get_program(), "/usr/bin/openssl");
        return;
    }
    let root = std::env::temp_dir().join(format!("switcher-fake-tool-{}", std::process::id()));
    crate::storage::create_private_directory(&root).unwrap();
    let fixture = Fixture { root, child: None };
    for dir in ["home", "tmp", "fake"] {
        crate::storage::create_private_directory(&fixture.root.join(dir)).unwrap();
    }
    let fake = fixture.root.join("fake/openssl");
    std::fs::write(&fake, b"#!/bin/sh\nexit 1\n").unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "network::socks_controlled::test_tools_ignore_writable_path",
        ])
        .env("PATH", fixture.root.join("fake"))
        .env("SWITCHER_FAKE_TOOL_ROOT", &fixture.root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[test]
fn evidence_environment_does_not_overwrite_files_or_follow_links() {
    use std::os::unix::fs::symlink;
    let root = std::env::temp_dir().join(format!("switcher-evidence-test-{}", std::process::id()));
    crate::storage::create_private_directory(&root).unwrap();
    let fixture = Fixture { root, child: None };
    let target = fixture.root.join("existing");
    let link = fixture.root.join("link");
    symlink(&target, &link).unwrap();
    let mut preserved = Vec::new();
    for destination in [&target, &link] {
        std::fs::write(&target, b"preserve existing contents").unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "network::socks_controlled::socks_local_and_remote_dns_authentication_and_tls_stay_in_explicit_proxy"])
            .env("SWITCHER_SOCKS_EVIDENCE", destination)
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        preserved.push(std::fs::read(&target).unwrap() == b"preserve existing contents");
    }
    assert_eq!(
        preserved,
        [true, true],
        "existing file and symlink target must remain unchanged"
    );
    let absent = fixture.root.join("absent");
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "network::socks_controlled::socks_local_and_remote_dns_authentication_and_tls_stay_in_explicit_proxy"])
        .env("SWITCHER_SOCKS_EVIDENCE", &absent)
        .output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(!absent.exists());
}

#[test]
fn socks_transport_child() {
    let Ok(case) = std::env::var("SWITCHER_SOCKS_CASE") else {
        return;
    };
    let root = PathBuf::from(std::env::var_os("SWITCHER_SOCKS_ROOT").unwrap());
    let certificate = std::fs::read(root.join(if case == "wrong_ca" {
        "other.pem"
    } else {
        "cert.pem"
    }))
    .unwrap();
    let proxy = std::env::var("SWITCHER_SOCKS_PROXY").unwrap();
    let target = std::env::var("SWITCHER_SOCKS_TARGET").unwrap();
    let client = client_builder(Some(&proxy))
        .unwrap()
        .tls_certs_only([reqwest::Certificate::from_pem(&certificate).unwrap()])
        .build()
        .unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let result = runtime.block_on(transport_probe(&client, "synthetic_socks", &target));
    match case.as_str() {
        "valid_local" | "valid_remote_auth" => {
            assert_eq!(result.transport, TransportState::Reached);
            assert_eq!(result.http_status, Some(200));
            assert!(result.tls_verified);
        }
        "wrong_ca" => {
            assert_eq!(result.transport, TransportState::TlsFailure);
            assert!(!result.tls_verified);
        }
        "wrong_auth" => {
            assert_eq!(result.transport, TransportState::ProxyAuthentication);
            assert!(!result.tls_verified);
        }
        _ => panic!("unknown fixture case"),
    }
}

#[test]
fn socks_local_and_remote_dns_authentication_and_tls_stay_in_explicit_proxy() {
    let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
        "switcher-socks-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    crate::storage::create_private_directory(&root).unwrap();
    let mut fixture = Fixture { root, child: None };
    for dir in ["home", "tmp"] {
        crate::storage::create_private_directory(&fixture.root.join(dir)).unwrap();
    }
    std::fs::write(fixture.root.join("ca.cnf"), "[req]\nprompt=no\ndistinguished_name=dn\nx509_extensions=ext\n[dn]\nCN=synthetic local CA\n[ext]\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\n").unwrap();
    std::fs::write(fixture.root.join("server.cnf"), "[req]\nprompt=no\ndistinguished_name=dn\n[dn]\nCN=localhost\n[ext]\nsubjectAltName=DNS:localhost,IP:127.0.0.1\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n").unwrap();
    for (key, cert) in [("ca.key", "cert.pem"), ("other.key", "other.pem")] {
        let status = trusted_tool("openssl", &fixture.root)
            .args([
                "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1", "-config", "ca.cnf",
                "-keyout", key, "-out", cert,
            ])
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "synthetic certificate generation failed"
        );
    }
    for args in [
        vec![
            "req",
            "-new",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-config",
            "server.cnf",
            "-keyout",
            "server.key",
            "-out",
            "server.csr",
        ],
        vec![
            "x509",
            "-req",
            "-in",
            "server.csr",
            "-CA",
            "cert.pem",
            "-CAkey",
            "ca.key",
            "-CAcreateserial",
            "-days",
            "1",
            "-extfile",
            "server.cnf",
            "-extensions",
            "ext",
            "-out",
            "server.pem",
        ],
    ] {
        assert!(
            trusted_tool("openssl", &fixture.root)
                .args(args)
                .output()
                .unwrap()
                .status
                .success(),
            "synthetic server certificate generation failed"
        );
    }
    let mut child = trusted_tool("python3", &fixture.root)
        .args(["-I", "-u", "-c", include_str!("socks_fixture.py")])
        .arg(&fixture.root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    fixture.child = Some(child);
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut ready = String::new();
        let _ = sender.send(
            std::io::BufReader::new(stdout)
                .read_line(&mut ready)
                .map(|_| ready),
        );
    });
    let ready = receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    let ports: serde_json::Value = serde_json::from_str(&ready).unwrap();
    let port = |name: &str| u16::try_from(ports[name].as_u64().unwrap()).unwrap();
    assert_ne!(port("direct"), port("poison"));
    for (case, scheme, auth) in [
        ("valid_local", "socks5", ""),
        ("valid_remote_auth", "socks5h", "fixture-user:fixture-pass@"),
        ("wrong_ca", "socks5h", ""),
        ("wrong_auth", "socks5h", "fixture-user:wrong@"),
    ] {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "network::socks_controlled::socks_transport_child",
            ])
            .env_clear()
            .env("HOME", fixture.root.join("home"))
            .env("TMPDIR", fixture.root.join("tmp"))
            .env("SWITCHER_SOCKS_CASE", case)
            .env("SWITCHER_SOCKS_ROOT", &fixture.root)
            .env(
                "SWITCHER_SOCKS_PROXY",
                format!("{scheme}://{auth}127.0.0.1:{}", port("proxy")),
            )
            .env(
                "SWITCHER_SOCKS_TARGET",
                format!("https://localhost:{}/controlled", port("direct")),
            )
            .env("HTTP_PROXY", format!("http://127.0.0.1:{}", port("poison")))
            .env(
                "HTTPS_PROXY",
                format!("http://127.0.0.1:{}", port("poison")),
            )
            .env("ALL_PROXY", format!("http://127.0.0.1:{}", port("poison")))
            .env("NO_PROXY", "*")
            .env("no_proxy", "*");
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{case}: {} {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    let events: Vec<serde_json::Value> = loop {
        let rows: Vec<_> = std::fs::read_to_string(fixture.root.join("wire.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        if rows
            .iter()
            .filter(|row: &&serde_json::Value| row["endpoint"] == "tls")
            .count()
            >= 3
        {
            break rows;
        }
        assert!(
            Instant::now() < deadline,
            "fixture did not record TLS completion"
        );
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(
        !events
            .iter()
            .any(|row| row["event"] == "error" || row["event"] == "unexpected_connection")
    );
    let connects: Vec<_> = events
        .iter()
        .filter(|row| row["event"] == "connect")
        .collect();
    assert_eq!(connects.len(), 3);
    assert_eq!(
        connects
            .iter()
            .filter(|row| row["atyp"] == 3 && row["address"] == "localhost")
            .count(),
        2
    );
    assert_eq!(connects.iter().filter(|row| row["atyp"] != 3).count(), 1);
    assert_eq!(
        events
            .iter()
            .filter(|row| row["event"] == "auth" && row["accepted"] == true)
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|row| row["event"] == "auth" && row["accepted"] == false)
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|row| row["event"] == "http" && row["valid"] == true)
            .count(),
        2
    );
    assert_eq!(
        events
            .iter()
            .filter(|row| row["event"] == "handshake_rejected")
            .count(),
        1
    );
    // Positive controls establish both bypass traps are live and observable.
    for name in ["direct", "poison"] {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port(name))).unwrap();
        let _ = stream.write_all(b"positive-control");
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let wire = std::fs::read_to_string(fixture.root.join("wire.jsonl")).unwrap();
        let events: Vec<serde_json::Value> = wire
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let count = |endpoint: &str| {
            events
                .iter()
                .filter(|row| {
                    row["event"] == "unexpected_connection" && row["endpoint"] == endpoint
                })
                .count()
        };
        if count("direct") == 1 && count("environment") == 1 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "bypass positive controls not recorded"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}
