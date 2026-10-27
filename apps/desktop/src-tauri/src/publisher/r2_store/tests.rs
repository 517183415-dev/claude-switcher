use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
    thread,
};

#[test]
fn r2_target_identity_tracks_namespace_and_survives_credential_rotation() {
    fn identity(
        account: &str,
        bucket: &str,
        origin: &str,
        channel: AdChannel,
        key: &str,
    ) -> String {
        R2Store::open(
            R2Config {
                account_id: account.into(),
                bucket: bucket.into(),
                public_origin: origin.into(),
                channel,
            },
            key,
            key,
        )
        .unwrap()
        .target_identity()
        .expect("a remote target must have a stable identity")
    }

    let account = "abcdef0123456789abcdef0123456789";
    let base = identity(
        account,
        "ads-preview",
        "https://ads.example.com",
        AdChannel::Preview,
        "key-a",
    );
    assert!(base.starts_with("r2:") && base.len() == 67);
    assert_eq!(
        base,
        identity(
            &account.to_ascii_uppercase(),
            "ads-preview",
            "https://ADS.EXAMPLE.COM.",
            AdChannel::Preview,
            "key-b"
        )
    );
    assert_ne!(
        base,
        identity(
            "abcdef0123456789abcdef0123456780",
            "ads-preview",
            "https://ads.example.com",
            AdChannel::Preview,
            "key-a"
        )
    );
    assert_ne!(
        base,
        identity(
            account,
            "ads-other",
            "https://ads.example.com",
            AdChannel::Preview,
            "key-a"
        )
    );
    assert_ne!(
        base,
        identity(
            account,
            "ads-preview",
            "https://other.example.com",
            AdChannel::Preview,
            "key-a"
        )
    );
    assert_ne!(
        base,
        identity(
            account,
            "ads-preview",
            "https://ads.example.com",
            AdChannel::Production,
            "key-a"
        )
    );
}

fn server(
    responses: Vec<&'static str>,
) -> (String, mpsc::Receiver<String>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        for response in responses {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 4096];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = stream.read(&mut buf).unwrap();
                assert!(n > 0);
                request.extend_from_slice(&buf[..n]);
            }
            tx.send(String::from_utf8_lossy(&request).into_owned())
                .unwrap();
            stream.write_all(response.as_bytes()).unwrap();
        }
    });
    (url, rx, handle)
}

fn store(endpoint: &str) -> R2Store {
    let config = aws_sdk_s3::config::Builder::new()
        .behavior_version(no_proxy_behavior())
        .region(Region::new("auto"))
        .endpoint_url(endpoint)
        .force_path_style(true)
        .credentials_provider(Credentials::new(
            "synthetic-key",
            "synthetic-secret",
            None,
            None,
            "test",
        ))
        .retry_config(RetryConfig::standard().with_max_attempts(1))
        .timeout_config(
            TimeoutConfig::builder()
                .operation_timeout(Duration::from_secs(5))
                .build(),
        )
        .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
        .build();
    R2Store {
        channel: AdChannel::Preview,
        bucket: "test-bucket".into(),
        origin: reqwest::Url::parse(&format!("{endpoint}/")).unwrap(),
        target: "r2:synthetic".into(),
        lock_target: format!("r2-lock:{}", "a".repeat(64)),
        client: Client::from_conf(config),
        public_client: reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap(),
        runtime: tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap(),
    }
}

#[test]
fn signed_conditional_writes_and_error_classification() {
    let (url, rx, handle) = server(vec![
        "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        "HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    ]);
    let mut s = store(&url);
    assert_eq!(s.commit_current(None, b"new"), Ok(()));
    assert_eq!(
        s.commit_current(Some("\"old\""), b"new"),
        Err(StoreError::Conflict)
    );
    assert_eq!(
        s.commit_current(Some("\"old\""), b"new"),
        Err(StoreError::OutcomeUnknown)
    );
    assert_eq!(
        s.commit_current(Some("\"old\""), b"new"),
        Err(StoreError::Permission)
    );
    let first = rx.recv().unwrap().to_ascii_lowercase();
    let second = rx.recv().unwrap().to_ascii_lowercase();
    assert!(first.contains("authorization: aws4-hmac-sha256"));
    assert!(first.contains("if-none-match: *"));
    assert!(second.contains("if-match: \"old\""));
    handle.join().unwrap();
}

#[test]
fn reserved_lock_transport_uses_authenticated_conditional_requests_only() {
    let (url, rx, handle) = server(vec![
        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nETag: \"lock\"\r\nConnection: close\r\n\r\n{}",
        "HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    ]);
    let mut s = store(&url);
    assert!(s.read_lock().unwrap().is_none());
    s.commit_lock(None, b"{}").unwrap();
    assert_eq!(s.read_lock().unwrap().unwrap().etag, "\"lock\"");
    assert_eq!(
        s.commit_lock(Some("\"lock\""), b"{}"),
        Err(StoreError::Conflict)
    );
    for index in 0..4 {
        let request = rx.recv().unwrap().to_ascii_lowercase();
        assert!(request.contains("/test-bucket/preview/publisher-lock.json"));
        assert!(request.contains("authorization: aws4-hmac-sha256"));
        if index == 1 {
            assert!(request.contains("if-none-match: *"));
        }
        if index == 3 {
            assert!(request.contains("if-match: \"lock\""));
        }
    }
    let key = "preview/publisher-lock.json";
    assert_eq!(s.put_immutable(key, b"{}"), Err(StoreError::Permission));
    assert!(matches!(
        s.public_read(key, 1024),
        Err(StoreError::Permission)
    ));
    assert_eq!(
        s.commit_lock(None, &vec![0; super::super::remote_lock::WIRE_LIMIT + 1]),
        Err(StoreError::Permission)
    );
    handle.join().unwrap();
}

#[test]
fn lock_identity_coalesces_domain_aliases_but_separates_accounts_buckets_and_channels() {
    let mut c = R2Config {
        account_id: "abcdef0123456789abcdef0123456789".into(),
        bucket: "test-bucket".into(),
        public_origin: "https://ads.example.com".into(),
        channel: AdChannel::Preview,
    };
    let first = c.lock_identity().unwrap();
    c.public_origin = "https://other.example.com".into();
    c.account_id = c.account_id.to_ascii_uppercase();
    assert_eq!(first, c.lock_identity().unwrap());
    for variant in 0..3 {
        let mut different = c.clone();
        match variant {
            0 => different.bucket = "other-bucket".into(),
            1 => different.channel = AdChannel::Production,
            _ => different.account_id = "abcdef0123456789abcdef0123456780".into(),
        }
        assert_ne!(first, different.lock_identity().unwrap());
    }
}

#[test]
fn immutable_collision_checks_existing_bytes() {
    let (url, _rx, handle) = server(vec![
        "HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: 3\r\nETag: \"one\"\r\nConnection: close\r\n\r\none",
        "HTTP/1.1 412 Precondition Failed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nETag: \"two\"\r\nConnection: close\r\n\r\ntwoo",
    ]);
    let mut s = store(&url);
    assert_eq!(s.put_immutable("preview/versions/a.json", b"one"), Ok(()));
    assert_eq!(
        s.put_immutable("preview/versions/a.json", b"one"),
        Err(StoreError::Conflict)
    );
    handle.join().unwrap();
}

#[test]
fn public_read_is_anonymous_bounded_and_refuses_redirects() {
    let (url, rx, handle) = server(vec![
        "HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\none",
        "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\ntwoo",
        "HTTP/1.1 302 Found\r\nLocation: http://example.invalid/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    ]);
    let mut s = store(&url);
    assert_eq!(
        s.public_read("preview/current.json", 3)
            .unwrap()
            .unwrap()
            .bytes,
        b"one"
    );
    assert!(matches!(
        s.public_read("preview/current.json", 3),
        Err(StoreError::Unavailable)
    ));
    assert!(matches!(
        s.public_read("preview/current.json", 3),
        Err(StoreError::Unavailable)
    ));
    let request = rx.recv().unwrap().to_ascii_lowercase();
    assert!(!request.contains("authorization:"));
    handle.join().unwrap();
}

#[test]
fn source_requires_etag_and_enforces_size() {
    let (url, _rx, handle) = server(vec![
        "HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\none",
        "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nETag: \"large\"\r\nConnection: close\r\n\r\nlarge",
        "HTTP/1.1 200 OK\r\nContent-Length: 3\r\nETag: \"ok\"\r\nConnection: close\r\n\r\none",
    ]);
    let s = store(&url);
    assert!(matches!(
        s.source_read("preview/current.json", 3),
        Err(StoreError::Unavailable)
    ));
    assert!(matches!(
        s.source_read("preview/current.json", 3),
        Err(StoreError::Unavailable)
    ));
    let obj = s.source_read("preview/current.json", 3).unwrap().unwrap();
    assert_eq!(obj.etag, "\"ok\"");
    assert_eq!(obj.bytes, b"one");
    handle.join().unwrap();
}

#[test]
fn production_configuration_rejects_untrusted_endpoints() {
    for origin in [
        "http://example.com",
        "https://127.0.0.1",
        "https://test.r2.dev",
        "https://example.com/path",
        "https://user@example.com",
    ] {
        let config = R2Config {
            account_id: "a".repeat(32),
            bucket: "test-bucket".into(),
            public_origin: origin.into(),
            channel: AdChannel::Preview,
        };
        assert!(matches!(
            R2Store::open(config, "key", "secret"),
            Err(StoreError::Permission)
        ));
    }
}

#[test]
fn production_rejects_provider_domains_even_with_dns_root_dot() {
    for origin in [
        "https://bucket.r2.dev.",
        "https://r2.dev",
        "https://r2.cloudflarestorage.com",
        "https://bucket.r2.cloudflarestorage.com.",
    ] {
        let config = R2Config {
            account_id: "a".repeat(32),
            bucket: "test-bucket".into(),
            public_origin: origin.into(),
            channel: AdChannel::Preview,
        };
        assert!(
            matches!(
                R2Store::open(config, "key", "secret"),
                Err(StoreError::Permission)
            ),
            "origin must be rejected: {origin}"
        );
    }
}

#[test]
fn production_configuration_rejects_ipv6_literal_origins() {
    for origin in [
        "https://[::1]",
        "https://[::ffff:127.0.0.1]",
        "https://[2001:db8::1]",
    ] {
        let config = R2Config {
            account_id: "a".repeat(32),
            bucket: "test-bucket".into(),
            public_origin: origin.into(),
            channel: AdChannel::Preview,
        };
        assert!(
            matches!(config.validate(), Err(StoreError::Permission)),
            "IP literal must not be accepted as a public domain: {origin}"
        );
    }
}

#[test]
fn source_read_deadline_covers_slow_body_even_while_chunks_arrive() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = [0; 4096];
        assert!(socket.read(&mut request).unwrap() > 0);
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nETag: \"slow\"\r\nConnection: close\r\n\r\n").unwrap();
        for _ in 0..10 {
            thread::sleep(Duration::from_millis(50));
            if socket.write_all(b"a").is_err() {
                break;
            }
        }
    });
    let s = store(&url);
    let start = std::time::Instant::now();
    assert!(matches!(
        s.source_read_with_timeout("preview/current.json", 10, Duration::from_millis(180)),
        Err(StoreError::Unavailable)
    ));
    assert!(start.elapsed() >= Duration::from_millis(120));
    assert!(start.elapsed() < Duration::from_secs(2));
    worker.join().unwrap();
}
