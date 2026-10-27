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
use switcher_domain::ads::AdManifest;

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
    let publish = |publisher: &mut Publisher, now| {
        publisher.publish_configured_with(
            &root.0,
            AdChannel::Preview,
            now,
            &vault,
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
    };
    let first = publish(&mut publisher, 1002).unwrap();
    let mut draft = publisher.draft_read();
    draft.slots[0].campaigns[0].title = "更新后的合成广告".into();
    publisher.draft_save(draft, 1003).unwrap();
    let second = publish(&mut publisher, 1004).unwrap();
    assert!(second.revision > first.revision);

    let public_key = ed25519_dalek::SigningKey::from_bytes(&[42; 32]).verifying_key();
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
}
