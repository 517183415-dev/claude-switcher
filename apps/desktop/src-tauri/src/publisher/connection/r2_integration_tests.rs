//! One local, stateful S3-compatible peer for the real publisher and SDK adapter.
use super::*;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use switcher_core::{ads, signed};
use switcher_domain::{
    AdSlot,
    ads::{AdManifest, AdPreferences},
};

#[derive(Clone)]
struct Stored {
    bytes: Vec<u8>,
    etag: String,
}

struct Request {
    method: String,
    key: String,
    authenticated: bool,
    expected: Option<String>,
    absent: bool,
}

#[derive(Default)]
struct State {
    objects: BTreeMap<String, Stored>,
    requests: Vec<Request>,
    version: usize,
    conflict_next_current: bool,
    drop_current_response_after_commit: bool,
}

struct LocalS3 {
    endpoint: String,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl LocalS3 {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(Mutex::new(State::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_state = Arc::clone(&state);
        let worker_stop = Arc::clone(&stop);
        let worker = thread::spawn(move || {
            while !worker_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((socket, _)) => handle(socket, &worker_state),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => panic!("local test server failed: {error}"),
                }
            }
        });
        Self {
            endpoint,
            state,
            stop,
            worker: Some(worker),
        }
    }
}

impl Drop for LocalS3 {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let result = worker.join();
            if !thread::panicking() {
                result.unwrap();
            }
        }
    }
}

fn publish_with_sdk(
    publisher: &mut Publisher,
    root: &Root,
    vault: &MemoryVault,
    peer: &LocalS3,
    now: u64,
) -> Result<Publication> {
    publisher.publish_configured_with(
        &root.0,
        AdChannel::Preview,
        now,
        vault,
        |config, access, secret| {
            assert_eq!((access, secret), ("synthetic-key", "secret-synthetic-key"));
            Ok(Box::new(
                super::super::super::r2_store::tests::configured_store(
                    &peer.endpoint,
                    &config,
                    access,
                    secret,
                ),
            ))
        },
    )
}

fn handle(mut socket: TcpStream, shared: &Mutex<State>) {
    socket.set_nonblocking(false).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut bytes = Vec::new();
    let header_end = loop {
        let mut chunk = [0_u8; 8192];
        let n = socket.read(&mut chunk).unwrap();
        assert!(n > 0 && bytes.len() + n <= 8 * 1024 * 1024);
        bytes.extend_from_slice(&chunk[..n]);
        if let Some(pos) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };
    let headers = std::str::from_utf8(&bytes[..header_end]).unwrap();
    let mut lines = headers.split("\r\n");
    let mut request_line = lines.next().unwrap().split_whitespace();
    let method = request_line.next().unwrap().to_owned();
    let path = request_line
        .next()
        .unwrap()
        .split('?')
        .next()
        .unwrap()
        .to_owned();
    let mut length = 0;
    let mut authorization = None;
    let mut expected = None;
    let mut absent = false;
    for line in lines.filter(|line| !line.is_empty()) {
        let (name, value) = line.split_once(':').unwrap();
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            length = value.parse::<usize>().unwrap();
        } else if name.eq_ignore_ascii_case("authorization") {
            authorization = Some(value.to_owned());
        } else if name.eq_ignore_ascii_case("if-match") {
            expected = Some(value.to_owned());
        } else if name.eq_ignore_ascii_case("if-none-match") {
            absent = value == "*";
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            panic!("test server requires a bounded content length");
        }
    }
    assert!(length <= 8 * 1024 * 1024 - header_end);
    while bytes.len() < header_end + length {
        let mut chunk = [0_u8; 8192];
        let n = socket.read(&mut chunk).unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&chunk[..n]);
    }
    let body = &bytes[header_end..header_end + length];
    let (authenticated, key) = if let Some(key) = path.strip_prefix("/ads-preview/") {
        let auth = authorization.as_deref().unwrap_or("");
        assert!(
            auth.starts_with("AWS4-HMAC-SHA256 ") && auth.contains("Credential=synthetic-key/")
        );
        (true, key)
    } else {
        assert!(authorization.is_none() && method == "GET");
        (false, path.strip_prefix('/').unwrap())
    };
    let mut state = shared.lock().unwrap();
    state.requests.push(Request {
        method: method.clone(),
        key: key.to_owned(),
        authenticated,
        expected: expected.clone(),
        absent,
    });
    let (status, response, etag) = match method.as_str() {
        "GET" => match state.objects.get(key) {
            Some(value) => ("200 OK", value.bytes.clone(), Some(value.etag.clone())),
            None => (
                "404 Not Found",
                b"<Error><Code>NoSuchKey</Code></Error>".to_vec(),
                None,
            ),
        },
        "PUT" => {
            if key == "preview/current.json" && state.conflict_next_current {
                state.conflict_next_current = false;
                state.version += 1;
                let version = state.version;
                state.objects.get_mut(key).unwrap().etag = format!("\"version-{version}\"");
            }
            let existing = state.objects.get(key);
            let matches = (absent && existing.is_none())
                || expected
                    .as_ref()
                    .is_some_and(|expected| existing.is_some_and(|v| &v.etag == expected));
            if !matches {
                ("412 Precondition Failed", Vec::new(), None)
            } else {
                state.version += 1;
                let etag = format!("\"version-{}\"", state.version);
                state.objects.insert(
                    key.to_owned(),
                    Stored {
                        bytes: body.to_vec(),
                        etag: etag.clone(),
                    },
                );
                if key == "preview/current.json" && state.drop_current_response_after_commit {
                    state.drop_current_response_after_commit = false;
                    return;
                }
                ("200 OK", Vec::new(), Some(etag))
            }
        }
        _ => panic!("unexpected local test method"),
    };
    drop(state);
    let etag = etag
        .map(|value| format!("ETag: {value}\r\n"))
        .unwrap_or_default();
    write!(
        socket,
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{etag}Connection: close\r\n\r\n",
        response.len()
    )
    .unwrap();
    socket.write_all(&response).unwrap();
}

#[test]
fn configured_sdk_recovers_committed_current_after_response_is_lost() {
    let peer = LocalS3::start();
    let (root, mut publisher, mut vault) = fixture();
    configure(
        &mut publisher,
        &mut vault,
        AdChannel::Preview,
        "synthetic-key",
    )
    .unwrap();
    super::super::super::tests::attach_campaign(&mut publisher, 1001);
    let publish =
        |publisher: &mut Publisher, now| publish_with_sdk(publisher, &root, &vault, &peer, now);

    peer.state
        .lock()
        .unwrap()
        .drop_current_response_after_commit = true;
    assert!(matches!(
        publish(&mut publisher, 1002),
        Err(Error::Store(StoreError::OutcomeUnknown))
    ));
    let pending_id = publisher.status().pending_release_id.unwrap();
    assert_eq!(
        publisher.status().phase,
        Some(super::super::super::protocol::Phase::CommitUnknown)
    );
    assert!(publisher.status().history.is_empty());
    let (wire, requests_before_recovery) = {
        let state = peer.state.lock().unwrap();
        let wire = state.objects["preview/current.json"].bytes.clone();
        assert_eq!(
            wire,
            state.objects[&format!("preview/versions/{pending_id}.json")].bytes
        );
        let lock: serde_json::Value =
            serde_json::from_slice(&state.objects["preview/publisher-lock.json"].bytes).unwrap();
        assert!(lock["owner"].is_string());
        (wire, state.requests.len())
    };
    let public_key = ed25519_dalek::SigningKey::from_bytes(&[42; 32]).verifying_key();
    let manifest = checked_manifest(&wire, &public_key, 1002);
    assert_eq!(manifest.release_id, pending_id);

    drop(publisher);
    let mut publisher =
        Publisher::open(&root.0.join("publisher"), "ads-personal-v1", None, 1003).unwrap();
    assert_eq!(
        publisher.status().pending_release_id.as_deref(),
        Some(pending_id.as_str())
    );
    let recovered = publish(&mut publisher, 1003).unwrap();
    assert_eq!(recovered.release_id, pending_id);
    assert_eq!(recovered.revision, manifest.content_revision);
    assert!(publisher.status().phase.is_none());
    assert_eq!(publisher.status().history.len(), 1);

    let state = peer.state.lock().unwrap();
    assert_eq!(state.objects["preview/current.json"].bytes, wire);
    assert_eq!(
        state
            .objects
            .keys()
            .filter(|key| key.starts_with("preview/versions/"))
            .count(),
        1
    );
    let lock: serde_json::Value =
        serde_json::from_slice(&state.objects["preview/publisher-lock.json"].bytes).unwrap();
    assert!(lock["owner"].is_null());
    for key in [
        "preview/current.json".to_owned(),
        format!("preview/versions/{pending_id}.json"),
        super::super::super::protocol::asset_key(
            AdChannel::Preview,
            &manifest.slots[0].campaigns[0].image,
        ),
    ] {
        assert!(
            state.requests[requests_before_recovery..]
                .iter()
                .any(|request| {
                    request.method == "GET" && request.key == key && !request.authenticated
                })
        );
    }
}

#[test]
fn local_s3_accepts_delayed_request_data() {
    let peer = LocalS3::start();
    let mut socket = TcpStream::connect(peer.endpoint.trim_start_matches("http://")).unwrap();
    thread::sleep(Duration::from_millis(30));
    socket
        .write_all(b"GET /preview/missing HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).unwrap();
    assert!(response.starts_with("HTTP/1.1 404 Not Found\r\n"));
}

fn checked_manifest(wire: &[u8], public_key: &VerifyingKey, now: u64) -> AdManifest {
    let envelope: signed::Envelope = serde_json::from_slice(wire).unwrap();
    let signature = Signature::from_slice(&envelope.signature).unwrap();
    public_key
        .verify(envelope.payload.as_bytes(), &signature)
        .unwrap();
    let manifest: AdManifest = serde_json::from_str(&envelope.payload).unwrap();
    ads::validate_manifest(&manifest, AdChannel::Preview, "ads-personal-v1", now).unwrap();
    manifest
}

// The URL is built by the real client. Only its origin is redirected to the
// loopback peer; this fixture does not exercise the production HTTPS client.
fn recipient_refresh(
    peer: &LocalS3,
    source: &ads::ContentSource,
    preferences: &AdPreferences,
    cache: &mut ads::AdCache,
    key: &VerifyingKey,
    now: u64,
) -> std::result::Result<(), &'static str> {
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .build()
        .unwrap();
    let read = |url: reqwest::Url| {
        assert!(source.permits_response_url(&url, &url));
        client
            .get(format!("{}{}", peer.endpoint, url.path()))
            .send()
            .unwrap()
            .error_for_status()
            .unwrap()
            .bytes()
            .unwrap()
    };
    let current = source
        .manifest_request(preferences, AdChannel::Preview)
        .unwrap();
    let wire = read(current);
    cache.accept(&wire, key, AdChannel::Preview, "ads-personal-v1", now, true)?;
    let campaign = cache
        .current_campaign(AdSlot::ResidentialBroadband, now, preferences, true)
        .unwrap();
    let image = source
        .asset_request(
            AdSlot::ResidentialBroadband,
            &campaign.image,
            preferences,
            AdChannel::Preview,
        )
        .unwrap()
        .unwrap();
    ads::validate_image(&read(image), &campaign.image).unwrap();
    Ok(())
}

#[test]
fn configured_publication_uses_sdk_upload_readback_and_conditional_updates() {
    let peer = LocalS3::start();
    let (root, mut publisher, mut vault) = fixture();
    configure(
        &mut publisher,
        &mut vault,
        AdChannel::Preview,
        "synthetic-key",
    )
    .unwrap();
    super::super::super::tests::attach_campaign(&mut publisher, 1001);
    let publish =
        |publisher: &mut Publisher, now| publish_with_sdk(publisher, &root, &vault, &peer, now);
    let source = ads::ContentSource::new("https://preview.example.com/").unwrap();
    let preferences = AdPreferences {
        slots_enabled: [true, false, false],
        request_purpose_acknowledged: true,
    };
    let public_key = ed25519_dalek::SigningKey::from_bytes(&[42; 32]).verifying_key();
    let mut recipient = ads::AdCache::default();
    let first = publish(&mut publisher, 1002).unwrap();
    recipient_refresh(
        &peer,
        &source,
        &preferences,
        &mut recipient,
        &public_key,
        1002,
    )
    .unwrap();
    assert_eq!(recipient.highest_revision, first.revision);
    let mut draft = publisher.draft_read();
    draft.slots[0].campaigns[0].title = "更新后的合成广告".into();
    publisher.draft_save(draft, 1003).unwrap();
    let second = publish(&mut publisher, 1004).unwrap();
    assert!(second.revision > first.revision);
    recipient_refresh(
        &peer,
        &source,
        &preferences,
        &mut recipient,
        &public_key,
        1004,
    )
    .unwrap();
    assert_eq!(recipient.highest_revision, second.revision);
    assert_eq!(
        recipient
            .current_campaign(AdSlot::ResidentialBroadband, 1004, &preferences, true)
            .unwrap()
            .title,
        "更新后的合成广告"
    );

    let current_before_conflict = {
        let state = peer.state.lock().unwrap();
        let first_key = format!("preview/versions/{}.json", first.release_id);
        let second_key = format!("preview/versions/{}.json", second.release_id);
        let first_wire = &state.objects[&first_key].bytes;
        let current = &state.objects["preview/current.json"].bytes;
        assert_eq!(current, &state.objects[&second_key].bytes);
        let first_manifest = checked_manifest(first_wire, &public_key, 1004);
        let second_manifest = checked_manifest(current, &public_key, 1004);
        assert_eq!(first_manifest.content_revision, first.revision);
        assert_eq!(second_manifest.content_revision, second.revision);
        assert_eq!(
            second_manifest.slots[0].campaigns[0].title,
            "更新后的合成广告"
        );
        let asset = &second_manifest.slots[0].campaigns[0].image;
        let asset_key = super::super::super::protocol::asset_key(AdChannel::Preview, asset);
        ads::validate_image(&state.objects[&asset_key].bytes, asset).unwrap();
        assert!(
            state
                .requests
                .iter()
                .any(|r| r.key == asset_key && r.method == "GET" && !r.authenticated)
        );
        assert!(
            state
                .requests
                .iter()
                .any(|r| r.key == second_key && r.method == "GET" && !r.authenticated)
        );
        assert!(
            state
                .requests
                .iter()
                .any(|r| r.key == "preview/current.json" && r.method == "GET" && !r.authenticated)
        );
        current.clone()
    };

    let mut draft = publisher.draft_read();
    draft.slots[0].campaigns[0].title = "并发冲突测试".into();
    publisher.draft_save(draft, 1005).unwrap();
    peer.state.lock().unwrap().conflict_next_current = true;
    assert!(matches!(
        publish(&mut publisher, 1006),
        Err(Error::Store(StoreError::Conflict))
    ));
    assert_eq!(
        publisher.status().phase,
        Some(super::super::super::protocol::Phase::Conflict)
    );
    assert_eq!(publisher.status().history.len(), 2);
    let state = peer.state.lock().unwrap();
    assert_eq!(
        state.objects["preview/current.json"].bytes,
        current_before_conflict
    );
    let current_writes: Vec<_> = state
        .requests
        .iter()
        .filter(|r| r.authenticated && r.method == "PUT" && r.key == "preview/current.json")
        .collect();
    assert_eq!(current_writes.len(), 3);
    assert!(current_writes[0].absent && current_writes[0].expected.is_none());
    assert!(
        current_writes[1..]
            .iter()
            .all(|r| !r.absent && r.expected.is_some())
    );
    assert!(
        state.requests.iter().any(|r| r.authenticated
            && r.method == "PUT"
            && r.key == "preview/publisher-lock.json")
    );
    let first_wire = state.objects[&format!("preview/versions/{}.json", first.release_id)]
        .bytes
        .clone();
    drop(state);

    // Simulate a stale or tampered public response, after the client accepted v2.
    peer.state
        .lock()
        .unwrap()
        .objects
        .get_mut("preview/current.json")
        .unwrap()
        .bytes = first_wire;
    assert_eq!(
        recipient_refresh(
            &peer,
            &source,
            &preferences,
            &mut recipient,
            &public_key,
            1005
        ),
        Err("old_revision")
    );
    let mut forged: signed::Envelope = serde_json::from_slice(&current_before_conflict).unwrap();
    forged.payload.push(' ');
    peer.state
        .lock()
        .unwrap()
        .objects
        .get_mut("preview/current.json")
        .unwrap()
        .bytes = serde_json::to_vec(&forged).unwrap();
    assert_eq!(
        recipient_refresh(
            &peer,
            &source,
            &preferences,
            &mut recipient,
            &public_key,
            1005
        ),
        Err("signature")
    );
    assert_eq!(recipient.highest_revision, second.revision);
    assert_eq!(
        recipient.cached_wire.as_deref(),
        Some(current_before_conflict.as_slice())
    );
}
