use super::*;
use ed25519_dalek::{Signer, SigningKey};
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
};
use switcher_domain::ads::{Campaign, SlotContent};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        Self(std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-ad-runtime-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn sample(now: i64) -> ClockSample {
    ClockSample {
        utc_secs: now,
        continuous_ms: now as u64 * 1000,
        boot_id: "synthetic-boot".into(),
    }
}
fn prefs() -> AdPreferences {
    AdPreferences {
        slots_enabled: [true; 3],
        request_purpose_acknowledged: true,
    }
}
fn signer() -> SigningKey {
    SigningKey::from_bytes(&[0x55; 32])
}
fn configured_runtime(root: &Root) -> AdRuntime {
    AdRuntime {
        root: root.0.clone(),
        edition: Edition::Public,
        local_error: None,
        pins: Some(ContentPins {
            source: ContentSource::new("https://ads.example.com/content/").unwrap(),
            key: signer().verifying_key(),
            key_id: "synthetic-key".into(),
            channel: AdChannel::Production,
            local: false,
        }),
    }
}
fn png() -> Vec<u8> {
    vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2,
        0, 0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 96, 96, 96, 0, 0, 0,
        4, 0, 1, 246, 23, 56, 85, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ]
}
fn manifest(revision: u64) -> AdManifest {
    let bytes = png();
    let asset = ImageAsset {
        sha256: signed::digest(&bytes),
        mime: "image/png".into(),
        bytes: bytes.len() as u32,
        width: 1,
        height: 1,
    };
    AdManifest {
        schema_version: 1,
        channel: AdChannel::Production,
        content_revision: revision,
        key_id: "synthetic-key".into(),
        issued_at: 1000,
        expires_at: 200000,
        release_id: format!("release-{revision}"),
        slots: AdSlot::ALL
            .into_iter()
            .enumerate()
            .map(|(index, slot)| SlotContent {
                slot,
                enabled: true,
                campaigns: vec![Campaign {
                    campaign_id: format!("campaign-{index}"),
                    merchant_id: "merchant-one".into(),
                    merchant_name: "合成商家".into(),
                    title: "合成标题".into(),
                    description: "合成测试".into(),
                    image: asset.clone(),
                    button_text: "详情".into(),
                    target_url: "https://merchant.example.com/offer".into(),
                    starts_at: 1000,
                    ends_at: 190000,
                    enabled: true,
                }],
            })
            .collect(),
    }
}
fn wire(value: &AdManifest) -> Vec<u8> {
    let payload = serde_json::to_string(value).unwrap();
    serde_json::to_vec(&signed::Envelope {
        signature: signer().sign(payload.as_bytes()).to_bytes().to_vec(),
        payload,
    })
    .unwrap()
}
#[derive(Default)]
struct MemoryFetch {
    objects: HashMap<String, (Vec<u8>, String)>,
    requests: Vec<String>,
}
impl MemoryFetch {
    fn set(&mut self, runtime: &AdRuntime, value: &AdManifest) {
        let pins = runtime.pins.as_ref().unwrap();
        self.objects.insert(
            pins.source
                .manifest_request(&prefs(), AdChannel::Production)
                .unwrap()
                .to_string(),
            (wire(value), "application/json".into()),
        );
        for slot in &value.slots {
            for campaign in &slot.campaigns {
                let url = pins
                    .source
                    .asset_request(slot.slot, &campaign.image, &prefs(), AdChannel::Production)
                    .unwrap()
                    .unwrap();
                self.objects
                    .insert(url.to_string(), (png(), "image/png".into()));
            }
        }
    }
}
impl Fetcher for MemoryFetch {
    fn get(&mut self, url: &Url, limit: usize) -> Result<Download> {
        self.requests.push(url.to_string());
        let (bytes, mime) = self.objects.get(url.as_str()).ok_or("synthetic_missing")?;
        if bytes.len() > limit {
            return Err("ad_download_size");
        }
        Ok(Download {
            bytes: bytes.clone(),
            mime: Some(mime.clone()),
            final_url: url.clone(),
        })
    }
}
fn refresh(runtime: &AdRuntime, fetcher: &mut MemoryFetch, now: i64) -> Result<RefreshResult> {
    let _lock = runtime.lock()?;
    runtime.refresh_locked(&prefs(), true, sample(now), Some(fetcher), Some(17))
}

#[test]
fn missing_pins_and_all_off_make_zero_requests() {
    let root = Root::new();
    let runtime = AdRuntime::new(root.0.clone(), Edition::Public);
    let mut fetcher = MemoryFetch::default();
    let _lock = runtime.lock().unwrap();
    assert_eq!(
        runtime
            .refresh_locked(&prefs(), true, sample(1000), Some(&mut fetcher), Some(0))
            .unwrap()
            .reason,
        "wait_ad_content_materials"
    );
    assert_eq!(
        runtime
            .refresh_locked(
                &AdPreferences::default(),
                true,
                sample(1000),
                Some(&mut fetcher),
                Some(0)
            )
            .unwrap()
            .reason,
        "ads_disabled"
    );
    assert!(fetcher.requests.is_empty());
}

#[test]
fn configured_pins_with_all_slots_off_make_zero_requests() {
    let root = Root::new();
    let runtime = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&runtime, &manifest(1));
    let preferences = AdPreferences {
        slots_enabled: [false; 3],
        request_purpose_acknowledged: true,
    };
    let _lock = runtime.lock().unwrap();
    let result = runtime
        .refresh_locked(
            &preferences,
            true,
            sample(1000),
            Some(&mut fetcher),
            Some(0),
        )
        .unwrap();
    assert_eq!(result.reason, "ads_disabled");
    assert!(runtime.pins.is_some());
    assert!(!fetcher.objects.is_empty());
    assert!(fetcher.requests.is_empty());
    assert_eq!(runtime.load(&sample(1000)).unwrap().0.sequence.highest, 0);
}

#[test]
fn successful_refresh_persists_deadline_filters_slots_and_confirms_clicks() {
    let root = Root::new();
    let runtime = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    let mut value = manifest(7);
    value.slots[1].campaigns[0].image.sha256 = "c".repeat(64);
    value.slots[2].campaigns[0].image.sha256 = "d".repeat(64);
    fetcher.set(&runtime, &value);
    let mut preferences = prefs();
    preferences.slots_enabled = [true, false, false];
    {
        let _lock = runtime.lock().unwrap();
        assert_eq!(
            runtime
                .refresh_locked(
                    &preferences,
                    true,
                    sample(1000),
                    Some(&mut fetcher),
                    Some(17)
                )
                .unwrap()
                .next_check_at,
            Some(1917)
        );
    }
    assert_eq!(fetcher.requests.len(), 2);
    let (state, clock) = runtime.load(&sample(1001)).unwrap();
    let visible = runtime.visible(&state, &clock, &preferences, 1001).unwrap();
    assert_eq!(
        visible
            .slots
            .iter()
            .map(|slot| slot.campaigns.len())
            .collect::<Vec<_>>(),
        vec![1, 0, 0]
    );
    assert_eq!(clock.last_success_utc, Some(1000));
    assert!(matches!(
        runtime
            .click_locked(
                AdSlot::ResidentialBroadband,
                true,
                &preferences,
                &sample(1001)
            )
            .unwrap(),
        ClickDecision::NeedsConfirmation { .. }
    ));
    assert!(matches!(
        runtime
            .click_locked(
                AdSlot::ResidentialBroadband,
                true,
                &preferences,
                &sample(1002)
            )
            .unwrap(),
        ClickDecision::Ready { .. }
    ));
    let before = fetcher.requests.len();
    assert!(!refresh(&runtime, &mut fetcher, 1003).unwrap().refreshed);
    assert_eq!(fetcher.requests.len(), before);
    let reloaded = configured_runtime(&root);
    let (state, _) = reloaded.load(&sample(1003)).unwrap();
    assert_eq!(state.schedule.next_check_at, Some(1917));
    assert_eq!(state.sequence.highest, 7);
}

#[test]
fn failed_asset_keeps_authenticated_watermark_and_hides_old_content() {
    let root = Root::new();
    let runtime = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&runtime, &manifest(7));
    refresh(&runtime, &mut fetcher, 1000).unwrap();
    let mut higher = manifest(8);
    higher.slots[0].campaigns[0].image.sha256 = "b".repeat(64);
    fetcher.set(&runtime, &higher);
    assert!(refresh(&runtime, &mut fetcher, 1031).is_err());
    let (state, clock) = runtime.load(&sample(1031)).unwrap();
    assert_eq!(state.sequence.highest, 8);
    assert_eq!(state.cache.highest_revision, 7);
    assert_eq!(clock.last_success_utc, Some(1000));
    assert!(runtime.visible(&state, &clock, &prefs(), 1031).is_none());
    fetcher.set(&runtime, &manifest(7));
    assert!(refresh(&runtime, &mut fetcher, 1062).is_err());
    assert_eq!(runtime.load(&sample(1062)).unwrap().0.sequence.highest, 8);
}

#[test]
fn clock_jump_hides_content_and_baseline_needs_signed_refresh() {
    let root = Root::new();
    let runtime = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&runtime, &manifest(5));
    refresh(&runtime, &mut fetcher, 1000).unwrap();
    let mut jumped = sample(2000);
    jumped.continuous_ms = 1001000;
    let (state, mut clock) = runtime.load(&jumped).unwrap();
    assert!(!clock.trusted());
    assert!(runtime.visible(&state, &clock, &prefs(), 2000).is_none());
    runtime.save(&state, &clock).unwrap();
    let count = fetcher.requests.len();
    assert_eq!(
        runtime
            .refresh_locked(&prefs(), true, jumped.clone(), Some(&mut fetcher), Some(0))
            .unwrap()
            .reason,
        "clock_recovery_required"
    );
    assert_eq!(fetcher.requests.len(), count);
    clock.establish_baseline(sample(2000));
    runtime.save(&state, &clock).unwrap();
    assert!(runtime.visible(&state, &clock, &prefs(), 2000).is_none());
    refresh(&runtime, &mut fetcher, 2000).unwrap();
    let (state, clock) = runtime.load(&sample(2001)).unwrap();
    assert!(clock.trusted());
    assert_eq!(state.sequence.highest, 5);
    assert!(runtime.visible(&state, &clock, &prefs(), 2001).is_some());
    assert!(runtime.directory().join("clock.json").exists());
}

#[test]
fn malformed_state_is_not_reset_and_offline_content_expires() {
    let root = Root::new();
    let runtime = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&runtime, &manifest(1));
    refresh(&runtime, &mut fetcher, 1000).unwrap();
    let (state, clock) = runtime.load(&sample(1000)).unwrap();
    assert!(runtime.visible(&state, &clock, &prefs(), 87400).is_none());
    storage::atomic_write(
        &runtime.ads_directory().join("state.json"),
        b"{\"schema\":1,\"schema\":1}",
    )
    .unwrap();
    assert!(runtime.load(&sample(1001)).is_err());
}

fn response_once(response: &'static [u8]) -> (Url, std::thread::JoinHandle<String>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut buffer = [0u8; 4096];
        let count = stream.read(&mut buffer).unwrap();
        stream.write_all(response).unwrap();
        String::from_utf8_lossy(&buffer[..count]).into_owned()
    });
    (
        Url::parse(&format!("http://{address}/content/")).unwrap(),
        handle,
    )
}

#[test]
fn transport_bounds_body_rejects_redirect_and_sends_no_identity() {
    // HTTP is enabled only for this private loopback transport harness.
    // The production constructor always uses https_only(true).
    let mut fetcher = HttpsFetcher {
        client: reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
    };
    let (base, handle) = response_once(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
    let source = ContentSource::fixture(base);
    let request = source
        .manifest_request(&prefs(), AdChannel::Production)
        .unwrap();
    assert_eq!(fetcher.get(&request, 2).unwrap().bytes, b"{}");
    let received = handle.join().unwrap().to_ascii_lowercase();
    assert!(received.starts_with("get /content/production/current.json http/1.1"));
    for forbidden in [
        "authorization:",
        "cookie:",
        "referer:",
        "user-agent:",
        "?",
        "account",
        "device",
    ] {
        assert!(!received.contains(forbidden), "{forbidden}");
    }
    for response in [
        b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/never\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".as_slice(),
        b"HTTP/1.1 200 OK\r\nContent-Length: 99\r\nConnection: close\r\n\r\noversize".as_slice(),
        b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\noversize".as_slice(),
    ] {
        let (url, handle) = response_once(response);
        assert!(fetcher.get(&url, 2).is_err());
        handle.join().unwrap();
    }
    assert!(
        HttpsFetcher::new()
            .unwrap()
            .get(&Url::parse("http://127.0.0.1:1/forbidden").unwrap(), 2)
            .is_err()
    );
}

#[test]
fn real_loopback_https_rejects_non_tls_peer_without_downgrade() {
    // A real HTTPS request sends a TLS ClientHello to this controlled loopback
    // peer. Plain HTTP bytes cannot become an accepted advertisement response.
    let (mut url, handle) =
        response_once(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
    url.set_scheme("https").unwrap();
    assert_eq!(
        HttpsFetcher::new().unwrap().get(&url, 256).err(),
        Some("ad_network")
    );
    let received = handle.join().unwrap();
    assert!(!received.starts_with("GET "));
    assert_eq!(received.as_bytes().first(), Some(&0x16));
}

#[test]
fn stale_confirmation_and_slot_disabled_never_open_cached_target() {
    let root = Root::new();
    let runtime = configured_runtime(&root);
    let mut fetcher = MemoryFetch::default();
    fetcher.set(&runtime, &manifest(1));
    refresh(&runtime, &mut fetcher, 1000).unwrap();
    assert!(matches!(
        runtime
            .click_locked(AdSlot::ResidentialBroadband, false, &prefs(), &sample(1001))
            .unwrap(),
        ClickDecision::NeedsConfirmation { .. }
    ));
    let mut changed = manifest(2);
    changed.slots[0].campaigns[0].merchant_id = "merchant-two".into();
    fetcher.set(&runtime, &changed);
    refresh(&runtime, &mut fetcher, 1031).unwrap();
    assert!(matches!(
        runtime
            .click_locked(AdSlot::ResidentialBroadband, true, &prefs(), &sample(1032))
            .unwrap(),
        ClickDecision::NeedsConfirmation { .. }
    ));
    let mut off = prefs();
    off.slots_enabled[0] = false;
    assert_eq!(
        runtime.click_locked(AdSlot::ResidentialBroadband, true, &off, &sample(1033)),
        Err("ad_not_current")
    );
    assert!(
        runtime
            .visible(
                &runtime.load(&sample(1033)).unwrap().0,
                &runtime.load(&sample(1033)).unwrap().1,
                &AdPreferences::default(),
                1033
            )
            .is_none()
    );
}

struct TlsFixture {
    root: Root,
    child: std::process::Child,
    port: u16,
}
impl TlsFixture {
    fn new() -> Self {
        use std::io::BufRead;
        use std::process::{Command, Stdio};
        let root = Root::new();
        storage::create_private_directory(&root.0).unwrap();
        // Follow the updates fixture's local OpenSSL + Python server pattern,
        // with a separate CA certificate and a CA:FALSE server leaf.
        std::fs::write(root.0.join("ca.cnf"), "[req]\nprompt=no\ndistinguished_name=dn\nx509_extensions=ext\n[dn]\nCN=Switcher local test CA\n[ext]\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\n").unwrap();
        std::fs::write(root.0.join("leaf.cnf"), "[req]\nprompt=no\ndistinguished_name=dn\n[dn]\nCN=ads.example.com\n[ext]\nsubjectAltName=DNS:ads.example.com\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n").unwrap();
        for args in [
            vec![
                "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-sha256", "-days", "1",
                "-config", "ca.cnf", "-keyout", "ca.key", "-out", "ca.pem",
            ],
            vec![
                "req", "-new", "-newkey", "rsa:2048", "-nodes", "-sha256", "-config", "leaf.cnf",
                "-keyout", "leaf.key", "-out", "leaf.csr",
            ],
            vec![
                "x509",
                "-req",
                "-in",
                "leaf.csr",
                "-CA",
                "ca.pem",
                "-CAkey",
                "ca.key",
                "-CAcreateserial",
                "-days",
                "1",
                "-sha256",
                "-extfile",
                "leaf.cnf",
                "-extensions",
                "ext",
                "-out",
                "leaf.pem",
            ],
        ] {
            assert!(
                Command::new("/usr/bin/openssl")
                    .args(args)
                    .current_dir(&root.0)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let script = r#"import http.server,ssl,sys,pathlib,json
root=pathlib.Path(sys.argv[1])
class H(http.server.BaseHTTPRequestHandler):
 def log_message(self,*args): pass
 def do_GET(self):
  with (root/'requests.log').open('a') as f: f.write(json.dumps({'path':self.path,'headers':dict(self.headers)})+'\n')
  if (root/'redirect').exists():
   self.send_response(302);self.send_header('Location','https://ads.example.com/never-follow');self.end_headers();return
  name=self.path.rsplit('/',1)[-1]
  path=root/name
  if not path.is_file(): self.send_error(404);return
  data=path.read_bytes()
  self.send_response(200);self.send_header('Content-Type','application/json' if name.endswith('.json') else 'image/png');self.send_header('Content-Length',str(len(data)));self.end_headers();self.wfile.write(data)
server=http.server.HTTPServer(('127.0.0.1',0),H)
ctx=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);ctx.load_cert_chain(str(root/'leaf.pem'),str(root/'leaf.key'))
server.socket=ctx.wrap_socket(server.socket,server_side=True)
print(server.server_port,flush=True)
server.serve_forever()
"#;
        let mut child = Command::new("python3")
            .args(["-u", "-c", script])
            .arg(&root.0)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut port = String::new();
        std::io::BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut port)
            .unwrap();
        Self {
            root,
            child,
            port: port.trim().parse().unwrap(),
        }
    }
    fn fetcher(&self, trust: bool) -> HttpsFetcher {
        let mut builder = reqwest::blocking::Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .timeout(Duration::from_secs(5))
            .resolve(
                "ads.example.com",
                std::net::SocketAddr::from(([127, 0, 0, 1], self.port)),
            );
        if trust {
            builder = builder.add_root_certificate(
                reqwest::Certificate::from_pem(&std::fs::read(self.root.0.join("ca.pem")).unwrap())
                    .unwrap(),
            );
        }
        HttpsFetcher {
            client: builder.build().unwrap(),
        }
    }
    fn source(&self) -> ContentSource {
        ContentSource::fixture(
            Url::parse(&format!("https://ads.example.com:{}/content/", self.port)).unwrap(),
        )
    }
}
impl Drop for TlsFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn controlled_https_ca_signed_manifest_assets_privacy_and_negative_cases() {
    let tls = TlsFixture::new();
    let root = Root::new();
    let mut runtime = configured_runtime(&root);
    runtime.pins.as_mut().unwrap().source = tls.source();
    let value = manifest(11);
    let wire = wire(&value);
    std::fs::write(tls.root.0.join("current.json"), &wire).unwrap();
    std::fs::write(
        tls.root
            .0
            .join(format!("{}.png", value.slots[0].campaigns[0].image.sha256)),
        png(),
    )
    .unwrap();
    let request = runtime
        .pins
        .as_ref()
        .unwrap()
        .source
        .manifest_request(&prefs(), AdChannel::Production)
        .unwrap();
    assert_eq!(
        tls.fetcher(false)
            .get(&request, ads::MAX_MANIFEST_BYTES)
            .err(),
        Some("ad_network")
    );
    let _lock = runtime.lock().unwrap();
    let mut fetcher = tls.fetcher(true);
    assert!(
        runtime
            .refresh_locked(&prefs(), true, sample(1000), Some(&mut fetcher), Some(0))
            .unwrap()
            .refreshed
    );
    let (state, clock) = runtime.load(&sample(1001)).unwrap();
    assert_eq!(state.sequence.highest, 11);
    assert_eq!(clock.last_success_utc, Some(1000));
    assert_eq!(
        runtime
            .visible(&state, &clock, &prefs(), 1001)
            .unwrap()
            .slots
            .iter()
            .map(|slot| slot.campaigns.len())
            .sum::<usize>(),
        3
    );
    let log = std::fs::read_to_string(tls.root.0.join("requests.log")).unwrap();
    assert_eq!(log.lines().count(), 2);
    for line in log.lines() {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        let path = row["path"].as_str().unwrap();
        assert!(!path.contains('?'));
        assert!(path.starts_with("/content/production/"));
        let headers = row["headers"].as_object().unwrap();
        for key in headers.keys() {
            assert!(
                ![
                    "authorization",
                    "cookie",
                    "referer",
                    "user-agent",
                    "x-account-id",
                    "x-device-id"
                ]
                .contains(&key.to_ascii_lowercase().as_str())
            );
        }
    }
    std::fs::write(tls.root.0.join("redirect"), b"yes").unwrap();
    assert_eq!(
        runtime
            .refresh_locked(&prefs(), true, sample(1031), Some(&mut fetcher), Some(0))
            .err(),
        Some("ad_http_status_or_redirect")
    );
    assert!(
        !std::fs::read_to_string(tls.root.0.join("requests.log"))
            .unwrap()
            .contains("never-follow")
    );
    std::fs::remove_file(tls.root.0.join("redirect")).unwrap();
    let mut tampered: serde_json::Value = serde_json::from_slice(&wire).unwrap();
    tampered["payload"] = "{}".into();
    std::fs::write(
        tls.root.0.join("current.json"),
        serde_json::to_vec(&tampered).unwrap(),
    )
    .unwrap();
    assert_eq!(
        runtime
            .refresh_locked(&prefs(), true, sample(1062), Some(&mut fetcher), Some(0))
            .err(),
        Some("signature")
    );
    std::fs::write(
        tls.root.0.join("current.json"),
        vec![b' '; ads::MAX_MANIFEST_BYTES + 1],
    )
    .unwrap();
    assert_eq!(
        runtime
            .refresh_locked(&prefs(), true, sample(1093), Some(&mut fetcher), Some(0))
            .err(),
        Some("ad_download_size")
    );
    let (state, clock) = runtime.load(&sample(1093)).unwrap();
    assert_eq!(state.sequence.highest, 11);
    assert_eq!(clock.last_success_utc, Some(1000));
}

#[test]
fn local_trust_errors_are_explicit_and_public_open_never_creates_publication() {
    let root = Root::new();
    let runtime = AdRuntime::new(root.0.clone(), Edition::Public);
    assert!(!local::directory(&root.0).exists());
    assert!(runtime.snapshot(&prefs()).unwrap().is_none());
    assert!(!local::directory(&root.0).exists());
    storage::create_private_directory(&local::directory(&root.0)).unwrap();
    storage::atomic_write(&local::directory(&root.0).join("trust.json"), b"{}").unwrap();
    assert!(
        AdRuntime::new(root.0.clone(), Edition::Public)
            .snapshot(&prefs())
            .is_err()
    );
    let trust = local::LocalTrust {
        schema: 1,
        key_id: "ads-personal-v1".into(),
        public_key: signer().verifying_key().to_bytes(),
    };
    storage::atomic_json(&local::directory(&root.0).join("trust.json"), &trust).unwrap();
    storage::create_private_directory(&local::directory(&root.0).join("production")).unwrap();
    storage::atomic_write(
        &local::directory(&root.0).join("production/current.json"),
        b"{}",
    )
    .unwrap();
    // Fully disabled advertisements do not inspect malformed manifest or assets.
    let off = AdPreferences {
        slots_enabled: [false; 3],
        request_purpose_acknowledged: true,
    };
    let runtime = AdRuntime::new(root.0.clone(), Edition::Public);
    assert!(!runtime.refresh_if_due(&off, false).unwrap().refreshed);
    assert!(runtime.snapshot(&off).unwrap().is_none());
    assert!(runtime.snapshot(&prefs()).is_err());
}

fn passive_file_snapshot(path: &std::path::Path) -> (Vec<u8>, u64, i64, i64) {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::symlink_metadata(path).unwrap();
    (
        std::fs::read(path).unwrap(),
        m.ino(),
        m.mtime(),
        m.mtime_nsec(),
    )
}

#[test]
fn passive_snapshots_and_images_preserve_initialized_files_both_editions() {
    for edition in [Edition::Personal, Edition::Public] {
        let root = Root::new();
        let runtime = AdRuntime::new(root.0.clone(), edition);
        let disabled = AdPreferences::default();
        runtime.snapshot(&disabled).unwrap();
        let paths = [
            runtime.directory().join("clock.json"),
            runtime.ads_directory().join("state.json"),
        ];
        let before = paths.each_ref().map(|p| passive_file_snapshot(p));
        for _ in 0..3 {
            std::thread::sleep(Duration::from_millis(3));
            runtime.snapshot(&disabled).unwrap();
            assert!(
                runtime
                    .image_bytes(AdSlot::ALL[0], &disabled)
                    .unwrap()
                    .is_none()
            );
        }
        assert_eq!(before, paths.each_ref().map(|p| passive_file_snapshot(p)));
    }
}

#[test]
fn passive_no_work_refresh_preserves_clock_and_authenticated_history() {
    for local in [false, true] {
        let root = Root::new();
        let mut runtime = configured_runtime(&root);
        let mut fetcher = MemoryFetch::default();
        fetcher.set(&runtime, &manifest(7));
        refresh(&runtime, &mut fetcher, 1000).unwrap();
        if local {
            let directory = local::directory(&root.0);
            storage::create_private_directory(&directory).unwrap();
            storage::create_private_directory(&directory.join("production")).unwrap();
            storage::atomic_write(
                &directory.join("production/current.json"),
                b"synthetic-current",
            )
            .unwrap();
            runtime.pins.as_mut().unwrap().local = true;
            let (mut state, clock) = runtime.load(&sample(1000)).unwrap();
            state.local_attempt_hash = local::current_digest(&root.0).unwrap();
            state.local_wire_hash = state.local_attempt_hash.clone();
            runtime.save(&state, &clock).unwrap();
        }
        let paths = [
            runtime.directory().join("clock.json"),
            runtime.ads_directory().join("state.json"),
        ];
        let before = paths.each_ref().map(|path| passive_file_snapshot(path));
        let requests = fetcher.requests.len();
        for now in [1001, 1002, 1003] {
            let _lock = runtime.lock().unwrap();
            let result = runtime
                .refresh_locked(&prefs(), false, sample(now), Some(&mut fetcher), Some(0))
                .unwrap();
            assert_eq!(result.reason, "ad_refresh_not_due_or_merged");
        }
        assert_eq!(fetcher.requests.len(), requests);
        assert_eq!(
            before,
            paths.each_ref().map(|path| passive_file_snapshot(path))
        );
        let (state, clock) = runtime.load(&sample(1003)).unwrap();
        assert_eq!(state.sequence.highest, 7);
        assert_eq!(clock.last_success_utc, Some(1000));
    }
}

#[test]
fn passive_disabled_and_missing_materials_preserve_initialized_clock() {
    for configured in [false, true] {
        let root = Root::new();
        let runtime = if configured {
            configured_runtime(&root)
        } else {
            AdRuntime::new(root.0.clone(), Edition::Public)
        };
        let _lock = runtime.lock().unwrap();
        let (state, clock) = runtime.load(&sample(1000)).unwrap();
        runtime.save(&state, &clock).unwrap();
        let path = runtime.directory().join("clock.json");
        let before = passive_file_snapshot(&path);
        let mut fetcher = MemoryFetch::default();
        for now in [1001, 1002] {
            let preferences = if configured {
                AdPreferences::default()
            } else {
                prefs()
            };
            let result = runtime
                .refresh_locked(
                    &preferences,
                    false,
                    sample(now),
                    Some(&mut fetcher),
                    Some(0),
                )
                .unwrap();
            assert!(!result.refreshed);
        }
        assert!(fetcher.requests.is_empty());
        assert_eq!(before, passive_file_snapshot(&path));
    }
}
