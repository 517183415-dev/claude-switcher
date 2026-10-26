//! Independent E11 checks. Local synthetic data and loopback transport only.
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use switcher_core::{
    accounts::AccountStore, ad_runtime::AdRuntime, ads::ContentSource, config, diagnostics,
    network, storage, updates,
};
use switcher_domain::{
    AdSlot, Edition,
    ads::{AdChannel, AdPreferences, ImageAsset},
    checks::{AccessState, TransportState},
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const MARKERS: [&str; 4] = [
    "E11_SYNTHETIC_ACCOUNT_A_7df3502b",
    "E11_SYNTHETIC_ACCOUNT_B_d0429ab1",
    "E11_SYNTHETIC_LOGIN_COOKIE_9ef0721a",
    "E11_SYNTHETIC_DEVICE_ID_44fef21c",
];
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-e11-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        storage::create_private_directory(&path).unwrap();
        Self(path)
    }
    fn synthetic_accounts(&self) {
        for (slot, marker) in ["A", "B"].into_iter().zip(MARKERS) {
            let directory = self.0.join("accounts").join(slot);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                directory.join("synthetic-login.json"),
                format!(
                    "{{\"account\":\"{marker}\",\"cookie\":\"{}\",\"device_id\":\"{}\"}}",
                    MARKERS[2], MARKERS[3]
                ),
            )
            .unwrap();
        }
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
struct Capture {
    root: Root,
    child: Child,
    port: u16,
}
impl Capture {
    fn new() -> Self {
        Self::with_tls(true)
    }
    fn with_tls(tls: bool) -> Self {
        let root = Root::new();
        std::fs::write(root.0.join("openssl.cnf"), "[req]\nprompt=no\ndistinguished_name=dn\nx509_extensions=ext\n[dn]\nCN=claude.ai\n[ext]\nsubjectAltName=DNS:claude.ai,DNS:api.anthropic.com,DNS:www.cloudflare.com,DNS:ipinfo.io,DNS:ipwho.is,DNS:api.ip.sb\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n").unwrap();
        assert!(
            Command::new("/usr/bin/openssl")
                .args([
                    "req",
                    "-x509",
                    "-newkey",
                    "rsa:2048",
                    "-nodes",
                    "-sha256",
                    "-days",
                    "1",
                    "-config",
                    "openssl.cnf",
                    "-keyout",
                    "key.pem",
                    "-out",
                    "cert.pem"
                ])
                .current_dir(&root.0)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success()
        );
        let script = r#"import http.server,ssl,sys,pathlib,json
root=pathlib.Path(sys.argv[1])
class H(http.server.BaseHTTPRequestHandler):
 def log_message(self,*args): pass
 def do_GET(self):
  body=self.rfile.read(int(self.headers.get('Content-Length','0')))
  item={'method':self.command,'path':self.path,'headers':dict(self.headers),'body':body.decode('utf-8','replace')}
  with (root/'capture.jsonl').open('a') as f: f.write(json.dumps(item)+'\n')
  if (root/'redirect.json').exists() and not self.path.startswith('/sink'):
   redirect=json.loads((root/'redirect.json').read_text())
   self.send_response(redirect['status']);self.send_header('Location',redirect['location']);self.send_header('Content-Length','0');self.end_headers();return
  host=self.headers.get('Host','').split(':')[0]
  status=401 if host=='api.anthropic.com' else 403 if host=='claude.ai' else 200
  data=b'ip=203.0.113.7\nloc=US\n' if host=='www.cloudflare.com' else b'{"ip":"203.0.113.7","country":"US","country_code":"US","timezone":"America/New_York","org":"SYNTHETIC NETWORK"}'
  if (root/'geo-fallback').exists():
   status=503 if host in ('www.cloudflare.com','ipinfo.io','ipwho.is') else 200
   data=b'{"ip":"203.0.113.8","country_code":"JP","timezone":"Asia/Tokyo","organization":"SYNTHETIC THIRD SOURCE"}'
  self.send_response(status);self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
server=http.server.HTTPServer(('127.0.0.1',0),H)
ctx=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);ctx.load_cert_chain(str(root/'cert.pem'),str(root/'key.pem'))
if sys.argv[2]=='tls': server.socket=ctx.wrap_socket(server.socket,server_side=True)
print(server.server_port,flush=True)
server.serve_forever()
"#;
        let mut child = Command::new("python3")
            .args(["-u", "-c", script])
            .arg(&root.0)
            .arg(if tls { "tls" } else { "http" })
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut port = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut port)
            .unwrap();
        Self {
            root,
            child,
            port: port.trim().parse().unwrap(),
        }
    }
    fn client(&self) -> reqwest::Client {
        self.client_trusting(None)
    }
    fn client_trusting(&self, other: Option<&Capture>) -> reqwest::Client {
        let cert =
            reqwest::Certificate::from_pem(&std::fs::read(self.root.0.join("cert.pem")).unwrap())
                .unwrap();
        let mut builder = reqwest::Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("ClaudeSwitcher/0.2")
            .timeout(Duration::from_secs(3))
            .add_root_certificate(cert);
        if let Some(other) = other {
            builder = builder.add_root_certificate(
                reqwest::Certificate::from_pem(
                    &std::fs::read(other.root.0.join("cert.pem")).unwrap(),
                )
                .unwrap(),
            );
        }
        for host in [
            "claude.ai",
            "api.anthropic.com",
            "www.cloudflare.com",
            "ipinfo.io",
            "ipwho.is",
            "api.ip.sb",
        ] {
            builder = builder.resolve(host, ([127, 0, 0, 1], self.port).into());
        }
        builder.build().unwrap()
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test]
async fn synthetic_account_markers_are_absent_from_captured_fixed_probe_requests() {
    // HOME is set only on a dedicated test subprocess, never mutated in this harness.
    const CHILD: &str = "E11_HOME_BOUND_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let home = Root::new();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "synthetic_account_markers_are_absent_from_captured_fixed_probe_requests",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("HOME", &home.0)
            .env(CHILD, &home.0)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "HOME-bound child failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let home = config::home_dir().unwrap();
    assert_eq!(home, PathBuf::from(std::env::var_os(CHILD).unwrap()));
    let account_root = home.join(".claude-switcher");
    let active = home.join("Library/Application Support/Claude");
    storage::create_private_directory(&account_root).unwrap();
    storage::create_private_directory(&home.join("Library")).unwrap();
    storage::create_private_directory(&home.join("Library/Application Support")).unwrap();
    storage::create_private_directory(&active).unwrap();
    let store = AccountStore::new(&account_root, &active, unsafe { libc::geteuid() }).unwrap();
    for (slot, marker) in ["A", "B"].into_iter().zip(MARKERS) {
        let directory = account_root.join("accounts").join(slot);
        storage::create_private_directory(&directory).unwrap();
        storage::atomic_write(
            &directory.join("synthetic-login.json"),
            format!(
                "{{\"account\":\"{marker}\",\"cookie\":\"{}\",\"device_id\":\"{}\"}}",
                MARKERS[2], MARKERS[3]
            )
            .as_bytes(),
        )
        .unwrap();
    }
    storage::atomic_write(
        &active.join("synthetic-login.json"),
        MARKERS.join("\n").as_bytes(),
    )
    .unwrap();
    let status = store.status().unwrap();
    assert!(status.a_saved && status.b_saved);
    assert!(
        AccountStore::inspect(&account_root, &active, unsafe { libc::geteuid() })
            .unwrap()
            .a_saved
    );
    assert_eq!(
        std::fs::read_to_string(active.join("synthetic-login.json")).unwrap(),
        MARKERS.join("\n")
    );
    // AccountStore reads metadata; probes do not accept an account store. This binds
    // real account paths to this process, but is not a full lifecycle/App test.
    let fixture = Capture::new();
    let client = fixture.client();
    let geo = network::lookup_geo(&client).await;
    assert_eq!(geo.country.as_deref(), Some("US"));
    // Exercise every fixed geo fallback, including those not selected by a successful first source.
    for endpoint in network::GEO_ENDPOINTS {
        network::limited_get(&client, endpoint, network::MAX_RESPONSE_BYTES)
            .await
            .unwrap();
    }
    for (name, endpoint) in network::SERVICE_ENDPOINTS {
        let result = network::transport_probe(&client, name, endpoint).await;
        assert_eq!(result.transport, TransportState::Reached);
        assert!(result.tls_verified);
        assert!(matches!(
            result.access,
            AccessState::AuthenticationRequired | AccessState::Restricted
        ));
    }
    let wire = std::fs::read_to_string(fixture.root.0.join("capture.jsonl")).unwrap();
    for marker in MARKERS {
        assert!(
            !wire.contains(marker),
            "synthetic account marker reached transport"
        );
    }
    let records: Vec<serde_json::Value> = wire
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    // CF + first org fallback + three explicitly exercised fallbacks + two service endpoints.
    assert_eq!(records.len(), 7);
    for record in records {
        assert_eq!(record["method"], "GET");
        assert_eq!(record["body"], "");
        assert!(!record["path"].as_str().unwrap().contains('?'));
        let headers = record["headers"].as_object().unwrap();
        for key in headers.keys() {
            assert!(
                [
                    "host",
                    "accept",
                    "accept-encoding",
                    "user-agent",
                    "connection"
                ]
                .contains(&key.to_ascii_lowercase().as_str()),
                "unexpected request header"
            );
            assert!(
                ![
                    "authorization",
                    "cookie",
                    "proxy-authorization",
                    "x-api-key",
                    "x-account-id",
                    "x-device-id",
                    "referer"
                ]
                .contains(&key.to_ascii_lowercase().as_str())
            );
        }
    }
}

#[test]
fn all_ads_disabled_and_personal_unrequested_updates_produce_no_request_targets() {
    let root = Root::new();
    root.synthetic_accounts();
    let off = AdPreferences {
        slots_enabled: [false; 3],
        request_purpose_acknowledged: true,
    };
    let source = ContentSource::new("https://ads.example.com/fixed/").unwrap();
    let asset = ImageAsset {
        sha256: "a".repeat(64),
        mime: "image/png".into(),
        bytes: 100,
        width: 1,
        height: 1,
    };
    let mut constructed = 0;
    for channel in [AdChannel::Preview, AdChannel::Production] {
        constructed += usize::from(source.manifest_request(&off, channel).is_some());
        for slot in [
            AdSlot::ResidentialBroadband,
            AdSlot::AccountService,
            AdSlot::KycService,
        ] {
            constructed += usize::from(
                source
                    .asset_request(slot, &asset, &off, channel)
                    .unwrap()
                    .is_some(),
            );
        }
    }
    assert_eq!(constructed, 0);
    for edition in [Edition::Personal, Edition::Public] {
        let runtime = AdRuntime::new(root.0.clone(), edition).with_clock(|| {
            Ok(switcher_core::time::ClockSample {
                utc_secs: 1000,
                continuous_ms: 1_000_000,
                boot_id: "synthetic-boot".into(),
            })
        });
        for manual in [false, true] {
            let outcome = runtime.refresh_if_due(&off, manual).unwrap();
            assert!(!outcome.refreshed);
            assert_eq!(outcome.reason, "ads_disabled");
        }
    }
    for action in [
        updates::UpdateAction::Check,
        updates::UpdateAction::Download,
        updates::UpdateAction::Install,
    ] {
        let mut schedule = updates::UpdateSchedule::new(Edition::Personal, 0);
        for now in [0, 43_200, 86_400, u64::MAX] {
            assert!(!schedule.automatic_check_due(now));
            assert_eq!(
                schedule.begin(action, false, now),
                Err("personal_requires_user_action")
            );
        }
    }
}

#[test]
fn fixed_ad_source_cannot_be_replaced_by_response_links_or_account_data() {
    let source = ContentSource::new("https://ads.example.com/fixed/").unwrap();
    let prefs = AdPreferences {
        slots_enabled: [true; 3],
        request_purpose_acknowledged: true,
    };
    let requested = source
        .manifest_request(&prefs, AdChannel::Production)
        .unwrap();
    assert_eq!(
        requested.as_str(),
        "https://ads.example.com/fixed/production/current.json"
    );
    for suffix in [
        "https://elsewhere.example.com/fixed/production/current.json",
        "https://ads.example.com/fixed/private.json",
        "https://ads.example.com/fixed/production/current.json?account=E11_SYNTHETIC_ACCOUNT_A_7df3502b",
        "https://ads.example.com/fixed/production/current.json#device",
    ] {
        assert!(!source.permits_response_url(&requested, &url::Url::parse(suffix).unwrap()));
    }
    assert!(source.permits_response_url(&requested, &requested));
}

#[test]
fn common_redaction_survives_persistence_preview_and_export_serialization() {
    let root = Root::new();
    let raw = "出口203.0.113.9 [2001:db8::4]\nCookie: E11_SYNTHETIC_LOGIN_COOKIE_9ef0721a\nhttps://user:E11_SYNTHETIC_ACCOUNT_A_7df3502b@private.example:443/path";
    let preview = diagnostics::redact(raw);
    let path = root.0.join("activity.json");
    storage::atomic_json(&path, &vec![preview.clone()]).unwrap();
    let stored = storage::read_private_bytes(&path, 65536).unwrap();
    let loaded: Vec<String> = serde_json::from_slice(&stored).unwrap();
    let replay: Vec<_> = loaded
        .iter()
        .map(|line| diagnostics::redact(line))
        .collect();
    assert_eq!(replay, vec![preview]);
    let exported = serde_json::to_string_pretty(&replay).unwrap();
    for secret in [
        MARKERS[0],
        MARKERS[2],
        "203.0.113.9",
        "2001:db8::4",
        "private.example",
    ] {
        assert!(!exported.contains(secret));
    }
    // This test verifies the shared rule's round-trip. UI wiring is audited separately.
}

#[tokio::test]
async fn stun_wire_is_only_a_twenty_byte_binding_request_with_no_account_payload() {
    let root = Root::new();
    root.synthetic_accounts();
    let server = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let port = server.local_addr().unwrap().port();
    let capture = tokio::spawn(async move {
        let mut bytes = [0; 1024];
        let (length, peer) =
            tokio::time::timeout(Duration::from_secs(3), server.recv_from(&mut bytes))
                .await
                .unwrap()
                .unwrap();
        assert_eq!(length, 20);
        assert_eq!(&bytes[..8], &[0, 1, 0, 0, 0x21, 0x12, 0xa4, 0x42]);
        for marker in MARKERS {
            assert!(
                !bytes[..length]
                    .windows(marker.len())
                    .any(|part| part == marker.as_bytes())
            );
        }
        let mut response = bytes[..length].to_vec();
        response[..4].copy_from_slice(&[1, 1, 0, 12]);
        response.extend_from_slice(&[0, 1, 0, 8, 0, 1, 0, 0, 203, 0, 113, 7]);
        server.send_to(&response, peer).await.unwrap();
    });
    assert_eq!(
        network::stun_probe("127.0.0.1", port)
            .await
            .unwrap()
            .to_string(),
        "203.0.113.7"
    );
    capture.await.unwrap();
}

#[tokio::test]
async fn geo_production_lookup_reaches_third_ip_sb_source_after_failures() {
    let fixture = Capture::new();
    std::fs::write(fixture.root.0.join("geo-fallback"), b"controlled failures").unwrap();
    let geo = network::lookup_geo(&fixture.client()).await;
    assert_eq!(geo.country.as_deref(), Some("JP"));
    assert_eq!(geo.timezone.as_deref(), Some("Asia/Tokyo"));
    assert_eq!(geo.organization.as_deref(), Some("SYNTHETIC THIRD SOURCE"));
    assert_eq!(geo.ip, Some("203.0.113.8".parse().unwrap()));
    let captured = std::fs::read_to_string(fixture.root.0.join("capture.jsonl")).unwrap();
    let hosts: Vec<String> = captured
        .lines()
        .map(|line| {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(row["method"], "GET");
            assert_eq!(row["body"], "");
            row["headers"]["host"].as_str().unwrap().to_owned()
        })
        .collect();
    assert_eq!(
        hosts,
        ["www.cloudflare.com", "ipinfo.io", "ipwho.is", "api.ip.sb"]
    );
    for marker in MARKERS {
        assert!(!captured.contains(marker));
    }
}

// These controls deliberately keep the production HTTP builder and injected TLS
// builder separate. Neither proves native fixed-HTTPS production-builder coverage.
fn captured_requests(fixture: &Capture) -> Vec<serde_json::Value> {
    std::fs::read_to_string(fixture.root.0.join("capture.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn assert_private_get(record: &serde_json::Value) {
    assert_eq!(record["method"], "GET");
    assert_eq!(record["body"], "");
    assert!(!record["path"].as_str().unwrap().contains('?'));
    let headers = record["headers"].as_object().unwrap();
    for name in headers.keys() {
        assert!(
            [
                "host",
                "accept",
                "accept-encoding",
                "user-agent",
                "connection"
            ]
            .contains(&name.to_ascii_lowercase().as_str()),
            "unexpected request header"
        );
    }
    assert_eq!(
        headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("user-agent"))
            .unwrap()
            .1,
        "ClaudeSwitcher/0.2"
    );
    let wire = serde_json::to_string(record).unwrap();
    for marker in MARKERS {
        assert!(!wire.contains(marker), "synthetic marker reached first hop");
    }
}

async fn redirect_controls(tls: bool) {
    let source = Capture::with_tls(tls);
    let sink = Capture::with_tls(tls);
    let client = if tls {
        source.client_trusting(Some(&sink))
    } else {
        network::client(None).unwrap()
    };
    // Both HTTPS ports use claude.ai. The source client trusts both fixture
    // certificates so a followed cross-origin redirect would reach the recorder.
    let scheme = if tls { "https" } else { "http" };
    let host = if tls { "claude.ai" } else { "127.0.0.1" };
    let origin = format!("{scheme}://{host}:{}", source.port);
    let other = format!("{scheme}://{host}:{}", sink.port);
    for status in [301, 302, 303, 307, 308] {
        for cross_origin in [false, true] {
            let destination = if cross_origin { &other } else { &origin };
            storage::atomic_json(&source.root.0.join("redirect.json"),
                &serde_json::json!({"status":status,"location":format!("{destination}/sink?account={}", MARKERS[0])})).unwrap();
            let url = format!("{origin}/redirect/{status}/{}", u8::from(cross_origin));
            assert_eq!(
                network::limited_get(&client, &url, 1024).await,
                Err(TransportState::InvalidResponse)
            );
            if tls {
                let probe = network::transport_probe(&client, "controlled_redirect", &url).await;
                assert_eq!(probe.transport, TransportState::Reached);
                assert_eq!(probe.access, AccessState::Redirected);
                assert_eq!(probe.http_status, Some(status));
                assert!(probe.tls_verified);
            }
        }
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    let first = captured_requests(&source);
    assert_eq!(first.len(), if tls { 20 } else { 10 });
    for record in &first {
        assert_private_get(record);
        assert!(record["path"].as_str().unwrap().starts_with("/redirect/"));
    }
    assert!(
        captured_requests(&sink).is_empty(),
        "redirect second hop received a request"
    );
    // Positive controls show that both same-origin and second-origin recorders
    // observe a request if the caller explicitly visits the advertised sink.
    assert_eq!(
        client
            .get(format!("{origin}/sink?account={}", MARKERS[0]))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        if tls { 403 } else { 200 }
    );
    assert_eq!(
        client
            .get(format!("{other}/sink?account={}", MARKERS[0]))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16(),
        if tls { 403 } else { 200 }
    );
    assert_eq!(captured_requests(&source).len(), first.len() + 1);
    let observed = captured_requests(&sink);
    assert_eq!(observed.len(), 1);
    assert!(observed[0]["path"].as_str().unwrap().contains(MARKERS[0]));
}

#[tokio::test]
async fn production_http_client_rejects_redirects_without_second_hop() {
    redirect_controls(false).await;
}

#[tokio::test]
async fn injected_tls_fixture_rejects_redirects_without_second_hop() {
    redirect_controls(true).await;
}
