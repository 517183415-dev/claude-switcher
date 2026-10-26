//! Explicit diagnostic clients and legacy environment-proxy check clients.
use reqwest::{Client, redirect::Policy};
use serde_json::Value;
use std::{
    error::Error,
    io::Read,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::{Duration, Instant},
};
use switcher_domain::checks::{AccessState, TransportProbe, TransportState};

pub const SERVICE_ENDPOINTS: [(&str, &str); 2] = [
    ("anthropic_api", "https://api.anthropic.com/v1/models"),
    ("claude_web", "https://claude.ai/"),
];
pub const GEO_ENDPOINTS: [&str; 3] = [
    "https://ipinfo.io/json",
    "https://ipwho.is/",
    "https://api.ip.sb/geoip",
];
pub const STUN_SERVERS: [(&str, u16); 4] = [
    ("stun.miwifi.com", 3478),
    ("stun.chat.bilibili.com", 3478),
    ("stun.l.google.com", 19302),
    ("stun.cloudflare.com", 3478),
];
pub const MAX_RESPONSE_BYTES: usize = 64 * 1024;

pub fn client(proxy: Option<&str>) -> Result<Client, &'static str> {
    let mut builder = Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(8))
        .user_agent("ClaudeSwitcher/0.2");
    if let Some(proxy) = proxy {
        builder = builder.proxy(reqwest::Proxy::all(proxy).map_err(|_| "proxy_invalid")?);
    }
    builder.build().map_err(|_| "network_context_unavailable")
}

/// Ordinary legacy checks inherit environment proxies. Explicit diagnostics use client().
pub fn inherited_client() -> Result<Client, &'static str> {
    let mut builder = Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(8))
        .user_agent("ClaudeSwitcher/0.2");
    for (lower, upper, scheme) in [
        ("https_proxy", "HTTPS_PROXY", "https"),
        ("http_proxy", "HTTP_PROXY", "http"),
        ("all_proxy", "ALL_PROXY", "all"),
    ] {
        let value = std::env::var(lower)
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| std::env::var(upper).ok().filter(|s| !s.is_empty()));
        if let Some(value) = value {
            let proxy = match scheme {
                "https" => reqwest::Proxy::https(&value),
                "http" => reqwest::Proxy::http(&value),
                _ => reqwest::Proxy::all(&value),
            }
            .map_err(|_| "proxy_invalid")?
            .no_proxy(reqwest::NoProxy::from_env());
            builder = builder.proxy(proxy);
        }
    }
    builder.build().map_err(|_| "network_context_unavailable")
}

pub async fn limited_get(
    client: &Client,
    url: &str,
    limit: usize,
) -> Result<Vec<u8>, TransportState> {
    #[cfg(test)]
    let test_url = test_support::endpoint(url);
    #[cfg(test)]
    let url = test_url.as_deref().unwrap_or(url);
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| classify_error(&e))?;
    if !response.status().is_success() {
        return Err(TransportState::InvalidResponse);
    }
    if response.content_length().is_some_and(|v| v > limit as u64) {
        return Err(TransportState::InvalidResponse);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| classify_error(&e))? {
        if chunk.len() > limit.saturating_sub(body.len()) {
            return Err(TransportState::InvalidResponse);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

pub fn access_state(status: u16) -> AccessState {
    match status {
        401 => AccessState::AuthenticationRequired,
        403 | 407 => AccessState::Restricted,
        300..=399 => AccessState::Redirected,
        500..=599 => AccessState::ServerError,
        100..=499 => AccessState::Available,
        _ => AccessState::Unknown,
    }
}

pub fn classify_error(error: &reqwest::Error) -> TransportState {
    if error.is_timeout() {
        return TransportState::Timeout;
    }
    if error.is_dns() {
        return TransportState::DnsFailure;
    }
    let mut chain = String::new();
    let mut current: Option<&(dyn Error + 'static)> = Some(error);
    while let Some(e) = current {
        chain.push_str(&e.to_string().to_ascii_lowercase());
        current = e.source();
    }
    // Error detail stays in memory. Reports only expose the fixed enum.
    if chain.contains("could not resolve to acceptable address type") {
        TransportState::DnsFailure
    } else if chain.contains("407")
        || chain.contains("proxy authentication")
        || chain.contains("proxy authorization required")
        || chain.contains("credentials not accepted")
        || chain.contains("user/pass authentication")
        || chain.contains("server implements authentication incorrectly")
    {
        TransportState::ProxyAuthentication
    } else if ["certificate", "tls", "ssl", "certnotvalid", "unknownissuer"]
        .iter()
        .any(|s| chain.contains(s))
    {
        TransportState::TlsFailure
    } else if error.is_connect() {
        TransportState::ConnectFailure
    } else {
        TransportState::InvalidResponse
    }
}

pub async fn transport_probe(client: &Client, name: &str, url: &str) -> TransportProbe {
    let start = Instant::now();
    let mut result = TransportProbe {
        endpoint: name.into(),
        transport: TransportState::InvalidResponse,
        access: AccessState::Unknown,
        http_status: None,
        tls_verified: false,
        duration_ms: 0,
    };
    match client.get(url).send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            result.http_status = Some(status);
            if status == 407 {
                result.transport = TransportState::ProxyAuthentication;
            } else if url.starts_with("https://") {
                result.transport = TransportState::Reached;
                result.tls_verified = true;
                result.access = access_state(status);
            }
        }
        Err(error) => result.transport = classify_error(&error),
    }
    result.duration_ms = start.elapsed().as_millis().min(u64::MAX as u128) as u64;
    result
}

#[derive(Clone, Default)]
pub struct GeoInfo {
    pub ip: Option<IpAddr>,
    pub country: Option<String>,
    pub timezone: Option<String>,
    pub organization: Option<String>,
}

pub fn parse_geo(bytes: &[u8]) -> Option<GeoInfo> {
    let data: Value = serde_json::from_slice(bytes).ok()?;
    if data.get("success") == Some(&Value::Bool(false)) {
        return None;
    }
    let country = data
        .get("country_code")
        .or_else(|| data.get("country"))?
        .as_str()?;
    if country.len() != 2 || !country.bytes().all(|b| b.is_ascii_uppercase()) {
        return None;
    }
    let timezone = data
        .get("timezone")
        .and_then(|v| v.as_str().or_else(|| v.get("id")?.as_str()))
        .map(str::to_owned);
    let ip = data.get("ip")?.as_str()?.parse().ok()?;
    let organization = data
        .get("org")
        .or_else(|| data.get("organization"))
        .or_else(|| data.get("connection").and_then(|v| v.get("org")))
        .and_then(Value::as_str)
        .filter(|s| s.len() <= 256)
        .map(str::to_owned);
    Some(GeoInfo {
        ip: Some(ip),
        country: Some(country.into()),
        timezone,
        organization,
    })
}

/// Obtain the egress identity without waiting for advisory ASN enrichment.
pub(crate) async fn lookup_egress(client: &Client) -> GeoInfo {
    let mut info = GeoInfo::default();
    // Cloudflare supplies the egress decision; the remaining sources are bounded fallbacks.
    if let Ok(bytes) = limited_get(client, "https://www.cloudflare.com/cdn-cgi/trace", 8192).await
        && let Ok(text) = std::str::from_utf8(&bytes)
    {
        for line in text.lines() {
            if let Some(ip) = line.strip_prefix("ip=") {
                info.ip = ip.parse().ok();
            }
            if let Some(country) = line.strip_prefix("loc=")
                && country.len() == 2
                && country.bytes().all(|b| b.is_ascii_uppercase())
            {
                info.country = Some(country.into());
            }
        }
    }
    if info.ip.is_some() {
        return info;
    }
    // A country without its source IP cannot be attached to another source's identity.
    lookup_geo_details(client, GeoInfo::default()).await
}

/// Enrich only the same observed egress. Existing fields retain their source's precedence.
pub(crate) async fn lookup_geo_details(client: &Client, mut info: GeoInfo) -> GeoInfo {
    if info.country.is_some() && info.timezone.is_some() && info.organization.is_some() {
        return info;
    }
    for endpoint in GEO_ENDPOINTS {
        if let Ok(bytes) = limited_get(client, endpoint, MAX_RESPONSE_BYTES).await
            && let Some(fallback) = parse_geo(&bytes)
        {
            // Do not attach ASN/timezone from a different egress address.
            if info.ip.is_some() && info.ip != fallback.ip {
                continue;
            }
            info.ip = info.ip.or(fallback.ip);
            info.country = info.country.or(fallback.country);
            info.timezone = info.timezone.or(fallback.timezone);
            info.organization = info.organization.or(fallback.organization);
            break;
        }
    }
    info
}

/// Complete geo lookup retained for callers that need region and advisory information.
pub async fn lookup_geo(client: &Client) -> GeoInfo {
    lookup_geo_details(client, lookup_egress(client).await).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StunError {
    Timeout,
    Dns,
    Io,
    WrongSource,
    Header,
    Transaction,
    Length,
    AddressFamily,
    MissingAddress,
}

pub fn parse_stun(
    bytes: &[u8],
    transaction: &[u8; 12],
    expected: SocketAddr,
    source: SocketAddr,
) -> Result<IpAddr, StunError> {
    if expected != source {
        return Err(StunError::WrongSource);
    }
    if bytes.len() < 20 || bytes[0..2] != [1, 1] || bytes[4..8] != [0x21, 0x12, 0xa4, 0x42] {
        return Err(StunError::Header);
    }
    if bytes[8..20] != transaction[..] {
        return Err(StunError::Transaction);
    }
    let len = u16::from_be_bytes([bytes[2], bytes[3]]) as usize;
    if !len.is_multiple_of(4) || len + 20 != bytes.len() {
        return Err(StunError::Length);
    }
    let mut offset = 20;
    let mut mapped = None;
    while offset < bytes.len() {
        if offset + 4 > bytes.len() {
            return Err(StunError::Length);
        }
        let kind = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
        let size = u16::from_be_bytes([bytes[offset + 2], bytes[offset + 3]]) as usize;
        let end = offset
            .checked_add(4)
            .and_then(|n| n.checked_add(size))
            .ok_or(StunError::Length)?;
        if end > bytes.len() {
            return Err(StunError::Length);
        }
        let value = &bytes[offset + 4..end];
        if kind == 0x0020 || kind == 0x0001 {
            if value.len() < 4 || value[0] != 0 {
                return Err(StunError::AddressFamily);
            }
            let xor = kind == 0x0020;
            let ip = match (value[1], value.len()) {
                (1, 8) => {
                    let mut octets: [u8; 4] =
                        value[4..8].try_into().map_err(|_| StunError::Length)?;
                    if xor {
                        for (n, key) in octets.iter_mut().zip([0x21, 0x12, 0xa4, 0x42]) {
                            *n ^= key;
                        }
                    }
                    IpAddr::V4(Ipv4Addr::from(octets))
                }
                (2, 20) => {
                    let mut octets: [u8; 16] =
                        value[4..20].try_into().map_err(|_| StunError::Length)?;
                    if xor {
                        let mut mask = [0; 16];
                        mask[..4].copy_from_slice(&[0x21, 0x12, 0xa4, 0x42]);
                        mask[4..].copy_from_slice(transaction);
                        for (n, key) in octets.iter_mut().zip(mask) {
                            *n ^= key;
                        }
                    }
                    IpAddr::V6(Ipv6Addr::from(octets))
                }
                _ => return Err(StunError::AddressFamily),
            };
            if mapped.is_some_and(|old| old != ip) {
                return Err(StunError::AddressFamily);
            }
            mapped = Some(ip);
        }
        offset = end
            .checked_add((4 - size % 4) % 4)
            .ok_or(StunError::Length)?;
        if offset > bytes.len() {
            return Err(StunError::Length);
        }
    }
    mapped.ok_or(StunError::MissingAddress)
}

pub async fn stun_probe(host: &str, port: u16) -> Result<IpAddr, StunError> {
    tokio::time::timeout(Duration::from_secs(3), async {
        let mut addrs = tokio::net::lookup_host((host, port))
            .await
            .map_err(|_| StunError::Dns)?;
        let target = addrs.find(SocketAddr::is_ipv4).ok_or(StunError::Dns)?;
        let socket = tokio::net::UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|_| StunError::Io)?;
        let mut transaction = [0; 12];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut transaction))
            .map_err(|_| StunError::Io)?;
        let mut request = vec![0, 1, 0, 0, 0x21, 0x12, 0xa4, 0x42];
        request.extend_from_slice(&transaction);
        socket
            .send_to(&request, target)
            .await
            .map_err(|_| StunError::Io)?;
        let mut bytes = [0; 2048];
        let (len, source) = socket
            .recv_from(&mut bytes)
            .await
            .map_err(|_| StunError::Io)?;
        parse_stun(&bytes[..len], &transaction, target, source)
    })
    .await
    .unwrap_or(Err(StunError::Timeout))
}

pub async fn stun_all() -> Vec<Result<IpAddr, StunError>> {
    let mut tasks = tokio::task::JoinSet::new();
    let servers: Vec<_> = STUN_SERVERS
        .into_iter()
        .map(|(host, port)| (host.to_owned(), port))
        .collect();
    #[cfg(test)]
    let servers = test_support::SETTINGS
        .try_with(|s| s.stun_servers.clone())
        .unwrap_or(servers);
    for (host, port) in servers {
        tasks.spawn(async move { stun_probe(&host, port).await });
    }
    let mut results = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !tasks.is_empty() {
        match tokio::time::timeout_at(deadline, tasks.join_next()).await {
            Ok(Some(Ok(result))) => results.push(result),
            Ok(Some(Err(_))) => results.push(Err(StunError::Io)),
            _ => {
                for _ in 0..tasks.len() {
                    results.push(Err(StunError::Timeout));
                }
                tasks.abort_all();
                break;
            }
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_inherited_proxy_child() {
        if std::env::var_os("SWITCHER_PROXY_FIXTURE_CHILD").is_none() {
            return;
        }
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let response = inherited_client()
                .unwrap()
                .get("http://switcher-fixture.invalid/")
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), 407);
        });
    }
    #[test]
    fn legacy_checks_inherit_proxy_in_an_isolated_child() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let proxy = format!("http://{}", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0; 4096];
            let count = stream.read(&mut request).unwrap();
            assert!(
                String::from_utf8_lossy(&request[..count])
                    .starts_with("GET http://switcher-fixture.invalid/")
            );
            stream
                .write_all(
                    b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\n\r\n",
                )
                .unwrap();
        });
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "network::tests::legacy_inherited_proxy_child"])
            .env_clear()
            .env("SWITCHER_PROXY_FIXTURE_CHILD", "1")
            .env("http_proxy", proxy)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        worker.join().unwrap();
    }
    #[test]
    fn explicit_proxy_isolation_child() {
        let Some(proxy) = std::env::var_os("SWITCHER_EXPLICIT_PROXY_CHILD") else {
            return;
        };
        let target = std::env::var("SWITCHER_EXPLICIT_PROXY_TARGET").unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let context = client(Some(proxy.to_str().unwrap())).unwrap();
            let probe = transport_probe(&context, "controlled_explicit_proxy", &target).await;
            assert!(!probe.tls_verified);
            if std::env::var("SWITCHER_EXPLICIT_PROXY_REPLY").unwrap() == "407" {
                assert_eq!(probe.transport, TransportState::ProxyAuthentication);
            } else {
                assert_ne!(probe.transport, TransportState::Reached);
                assert_eq!(probe.http_status, None);
            }
        });
    }

    #[test]
    fn explicit_proxy_ignores_environment_and_never_falls_back_to_direct() {
        use std::io::{Read, Write};
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
            mpsc,
        };
        for reply in ["407", "502"] {
            let listeners: Vec<_> = (0..3)
                .map(|_| std::net::TcpListener::bind("127.0.0.1:0").unwrap())
                .collect();
            let addresses: Vec<_> = listeners.iter().map(|s| s.local_addr().unwrap()).collect();
            let stop = Arc::new(AtomicBool::new(false));
            let (sender, receiver) = mpsc::channel();
            let workers: Vec<_> = listeners.into_iter().enumerate().map(|(index, listener)| {
                listener.set_nonblocking(true).unwrap();
                let stop = stop.clone();
                let sender = sender.clone();
                std::thread::spawn(move || {
                    let deadline = Instant::now() + Duration::from_secs(15);
                    while !stop.load(Ordering::Relaxed) && Instant::now() < deadline {
                        let (mut stream, _) = match listener.accept() {
                            Ok(value) => value,
                            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                                std::thread::sleep(Duration::from_millis(2));
                                continue;
                            }
                            Err(error) => panic!("controlled listener: {error}"),
                        };
                        stream.set_nonblocking(false).unwrap();
                        stream.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
                        let mut request = Vec::new();
                        let mut chunk = [0; 1024];
                        while request.len() < 4096 && !request.windows(4).any(|v| v == b"\r\n\r\n") {
                            match stream.read(&mut chunk) {
                                Ok(0) | Err(_) => break,
                                Ok(count) => request.extend_from_slice(&chunk[..count]),
                            }
                        }
                        sender.send((index, request)).unwrap();
                        let response = if reply == "407" {
                            b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\n\r\n".as_slice()
                        } else {
                            b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n".as_slice()
                        };
                        let _ = stream.write_all(response);
                    }
                })
            }).collect();
            drop(sender);
            let explicit = format!("http://{}", addresses[0]);
            let poison = format!("http://{}", addresses[1]);
            let target = format!("https://{}/controlled", addresses[2]);
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "network::tests::explicit_proxy_isolation_child",
                    "--nocapture",
                ])
                .env_clear()
                .env("SWITCHER_EXPLICIT_PROXY_CHILD", explicit)
                .env("SWITCHER_EXPLICIT_PROXY_TARGET", target)
                .env("SWITCHER_EXPLICIT_PROXY_REPLY", reply);
            // Preserve only the already isolated harness directories, never the
            // caller's proxy environment or real HOME in this child.
            for name in ["HOME", "TMPDIR"] {
                if let Some(value) = std::env::var_os(name) {
                    command.env(name, value);
                }
            }
            for name in [
                "http_proxy",
                "HTTP_PROXY",
                "https_proxy",
                "HTTPS_PROXY",
                "all_proxy",
                "ALL_PROXY",
            ] {
                command.env(name, &poison);
            }
            command.env("no_proxy", "*").env("NO_PROXY", "*");
            let output = command.output().unwrap();
            // Positive controls prove every recorder, including the unused
            // environment and direct sinks, can observe a real TCP request.
            for address in &addresses {
                let mut stream = std::net::TcpStream::connect(address).unwrap();
                stream
                    .write_all(b"GET /positive-control HTTP/1.1\r\nHost: fixture\r\n\r\n")
                    .unwrap();
            }
            let mut records = Vec::new();
            for _ in 0..4 {
                records.push(receiver.recv_timeout(Duration::from_secs(3)).unwrap());
            }
            while let Ok(record) = receiver.recv_timeout(Duration::from_millis(100)) {
                records.push(record);
            }
            stop.store(true, Ordering::Relaxed);
            for worker in workers {
                worker.join().unwrap();
            }
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            for index in 0..3 {
                assert_eq!(
                    records
                        .iter()
                        .filter(|(endpoint, bytes)| *endpoint == index
                            && bytes.starts_with(b"GET /positive-control "))
                        .count(),
                    1
                );
            }
            let probes: Vec<_> = records
                .iter()
                .filter(|(_, bytes)| !bytes.starts_with(b"GET /positive-control "))
                .collect();
            assert_eq!(
                probes.len(),
                1,
                "proxy failure must not cause an extra request"
            );
            assert_eq!(
                probes[0].0, 0,
                "only the explicit proxy may receive the probe"
            );
            assert!(
                probes[0]
                    .1
                    .starts_with(format!("CONNECT {} HTTP/1.1", addresses[2]).as_bytes())
            );
        }
    }

    fn response() -> (Vec<u8>, [u8; 12], SocketAddr) {
        let transaction = [4; 12];
        let peer = "127.0.0.1:3478".parse().unwrap();
        let mut packet = vec![1, 1, 0, 12, 0x21, 0x12, 0xa4, 0x42];
        packet.extend(transaction);
        packet.extend([
            0,
            0x20,
            0,
            8,
            0,
            1,
            0x21,
            0x13,
            203 ^ 0x21,
            0x12,
            113 ^ 0xa4,
            4 ^ 0x42,
        ]);
        (packet, transaction, peer)
    }
    #[test]
    fn stun_validates_identity_source_lengths_and_family() {
        let (packet, txn, peer) = response();
        assert_eq!(
            parse_stun(&packet, &txn, peer, peer),
            Ok("203.0.113.4".parse().unwrap())
        );
        assert_eq!(
            parse_stun(&packet, &[5; 12], peer, peer),
            Err(StunError::Transaction)
        );
        assert_eq!(
            parse_stun(&packet, &txn, peer, "127.0.0.2:3478".parse().unwrap()),
            Err(StunError::WrongSource)
        );
        for i in [0, 3, 7, 19, 23, 31] {
            assert!(parse_stun(&packet[..i], &txn, peer, peer).is_err());
        }
        let mut bad = packet.clone();
        bad[25] = 3;
        assert_eq!(
            parse_stun(&bad, &txn, peer, peer),
            Err(StunError::AddressFamily)
        );
        let mut bad = packet;
        bad[23] = 12;
        assert_eq!(parse_stun(&bad, &txn, peer, peer), Err(StunError::Length));
    }
    #[test]
    fn geo_ip_sb_fallback_and_failure_are_distinct() {
        let info = parse_geo(
            br#"{"country_code":"US","timezone":"America/Los_Angeles","ip":"203.0.113.10"}"#,
        )
        .unwrap();
        assert_eq!(info.country.as_deref(), Some("US"));
        assert_eq!(info.timezone.as_deref(), Some("America/Los_Angeles"));
        assert!(
            parse_geo(br#"{"success":false,"country_code":"US","ip":"203.0.113.10"}"#).is_none()
        );
    }
    #[tokio::test]
    async fn geo_same_ip_fallback_fills_missing_country() {
        let fixture = test_support::Fixture::new([
            Some("ip=203.0.113.8\n"),
            Some(r#"{"ip":"203.0.113.8","country":"JP","org":"controlled"}"#),
            None,
            None,
        ])
        .await;
        test_support::SETTINGS
            .scope(fixture.settings.clone(), async {
                let info = lookup_geo(&fixture.client()).await;
                assert_eq!(info.country.as_deref(), Some("JP"));
                assert_eq!(info.ip, Some("203.0.113.8".parse().unwrap()));
            })
            .await;
    }
    #[tokio::test]
    async fn geo_enrichment_keeps_source_identity_and_never_defaults_country() {
        for (trace, fallback, country, organization) in [
            (
                "ip=203.0.113.8\n",
                r#"{"ip":"203.0.113.9","country":"US","org":"other-egress"}"#,
                None,
                None,
            ),
            (
                "ip=203.0.113.8\nloc=JP\n",
                r#"{"ip":"203.0.113.8","country":"US","org":"same-egress"}"#,
                Some("JP"),
                Some("same-egress"),
            ),
            (
                "loc=JP\n",
                r#"{"ip":"203.0.113.9","country":"US","org":"new-egress","timezone":"America/New_York"}"#,
                Some("US"),
                Some("new-egress"),
            ),
            ("ip=203.0.113.8\n", r#"{"success":false}"#, None, None),
        ] {
            let fixture = test_support::Fixture::new([
                Some(trace),
                Some(fallback),
                Some(fallback),
                Some(fallback),
            ])
            .await;
            test_support::SETTINGS
                .scope(fixture.settings.clone(), async {
                    let info = lookup_geo(&fixture.client()).await;
                    assert_eq!(info.country.as_deref(), country);
                    assert_eq!(info.organization.as_deref(), organization);
                })
                .await;
        }
    }
    #[test]
    fn authentication_and_access_do_not_assert_account_usability() {
        assert_eq!(access_state(401), AccessState::AuthenticationRequired);
        assert_eq!(access_state(403), AccessState::Restricted);
        assert_eq!(access_state(503), AccessState::ServerError);
    }
    #[test]
    fn stun_ipv6_xor_padding_and_magic_cookie_are_checked() {
        let transaction = [7; 12];
        let peer = "[::1]:3478".parse().unwrap();
        let ip: Ipv6Addr = "2001:db8::1234".parse().unwrap();
        let mut packet = vec![1, 1, 0, 32, 0x21, 0x12, 0xa4, 0x42];
        packet.extend(transaction);
        // An unknown three-byte attribute must consume its one-byte padding.
        packet.extend([0x80, 0x22, 0, 3, 1, 2, 3, 0]);
        packet.extend([0, 0x20, 0, 20, 0, 2, 0x21, 0x13]);
        let mut mask = [7; 16];
        mask[..4].copy_from_slice(&[0x21, 0x12, 0xa4, 0x42]);
        packet.extend(ip.octets().into_iter().zip(mask).map(|(a, b)| a ^ b));
        assert_eq!(
            parse_stun(&packet, &transaction, peer, peer),
            Ok(IpAddr::V6(ip))
        );
        packet[4] = 0;
        assert_eq!(
            parse_stun(&packet, &transaction, peer, peer),
            Err(StunError::Header)
        );
    }

    async fn local_proxy(reply: &'static [u8]) -> (u16, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let length = socket.read(&mut request).await.unwrap();
            assert!(request[..length].starts_with(b"CONNECT service.example:443 HTTP/1.1"));
            socket.write_all(reply).await.unwrap();
            if reply.starts_with(b"HTTP/1.1 200") {
                let _ = socket.read(&mut request).await;
            }
        });
        (port, task)
    }

    #[tokio::test]
    async fn explicit_proxy_authentication_tls_and_timeout_are_distinct() {
        for (reply, expected) in [
            (
                &b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\n\r\n"[..],
                TransportState::ProxyAuthentication,
            ),
            (
                &b"HTTP/1.1 200 Connection Established\r\n\r\nthis is not TLS"[..],
                TransportState::TlsFailure,
            ),
        ] {
            let (port, server) = local_proxy(reply).await;
            let context = client(Some(&format!("http://127.0.0.1:{port}"))).unwrap();
            let probe = transport_probe(&context, "controlled", "https://service.example/").await;
            server.await.unwrap();
            assert_eq!(probe.transport, expected);
            assert!(!probe.tls_verified);
        }
        use tokio::io::AsyncReadExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut byte = [0];
            let _ = socket.read(&mut byte).await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        });
        let context = Client::builder()
            .no_proxy()
            .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}")).unwrap())
            .timeout(Duration::from_millis(20))
            .build()
            .unwrap();
        let probe = transport_probe(&context, "controlled", "https://service.example/").await;
        server.await.unwrap();
        assert_eq!(probe.transport, TransportState::Timeout);
        assert!(!probe.tls_verified);
    }

    #[tokio::test]
    async fn resolver_failure_has_a_dns_classification_without_network_requests() {
        struct Refuse;
        impl reqwest::dns::Resolve for Refuse {
            fn resolve(&self, _: reqwest::dns::Name) -> reqwest::dns::Resolving {
                Box::pin(async {
                    Err(
                        std::io::Error::new(std::io::ErrorKind::NotFound, "controlled DNS failure")
                            .into(),
                    )
                })
            }
        }
        let context = Client::builder()
            .no_proxy()
            .dns_resolver(std::sync::Arc::new(Refuse))
            .build()
            .unwrap();
        let probe = transport_probe(&context, "controlled", "https://service.example/").await;
        assert_eq!(probe.transport, TransportState::DnsFailure);
        assert!(!probe.tls_verified);
        assert!(probe.http_status.is_none());
    }

    #[tokio::test]
    async fn socks_ipv6_uses_remote_domain_dns_without_http_connect_and_classifies_authentication()
    {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for authenticate in [false, true] {
            let listener = tokio::net::TcpListener::bind("[::1]:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut greeting = [0; 2];
                socket.read_exact(&mut greeting).await.unwrap();
                assert_eq!(greeting[0], 5);
                assert!(greeting[1] <= 8);
                let mut methods = vec![0; greeting[1] as usize];
                socket.read_exact(&mut methods).await.unwrap();
                socket
                    .write_all(&[5, if authenticate { 2 } else { 0 }])
                    .await
                    .unwrap();
                if authenticate {
                    let mut header = [0; 2];
                    socket.read_exact(&mut header).await.unwrap();
                    assert_eq!(header[0], 1);
                    let mut user = vec![0; header[1] as usize];
                    socket.read_exact(&mut user).await.unwrap();
                    let mut length = [0];
                    socket.read_exact(&mut length).await.unwrap();
                    let mut password = vec![0; length[0] as usize];
                    socket.read_exact(&mut password).await.unwrap();
                    assert!(
                        user.as_slice() == b"account" && password.as_slice() == b"private-password"
                    );
                    socket.write_all(&[1, 1]).await.unwrap();
                } else {
                    let mut header = [0; 5];
                    socket.read_exact(&mut header).await.unwrap();
                    assert_eq!(&header[..4], [5, 1, 0, 3]);
                    let mut domain = vec![0; header[4] as usize];
                    socket.read_exact(&mut domain).await.unwrap();
                    assert!(domain.as_slice() == b"service.example");
                    let mut port = [0; 2];
                    socket.read_exact(&mut port).await.unwrap();
                    assert_eq!(u16::from_be_bytes(port), 443);
                    socket
                        .write_all(&[5, 4, 0, 1, 0, 0, 0, 0, 0, 0])
                        .await
                        .unwrap();
                }
            });
            let credentials = if authenticate {
                "account:private-password@"
            } else {
                ""
            };
            let context = client(Some(&format!("socks5h://{credentials}[::1]:{port}"))).unwrap();
            let probe = transport_probe(&context, "controlled", "https://service.example/").await;
            tokio::time::timeout(Duration::from_secs(2), server)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                probe.transport,
                if authenticate {
                    TransportState::ProxyAuthentication
                } else {
                    TransportState::ConnectFailure
                }
            );
            assert!(probe.http_status.is_none());
            assert!(!probe.tls_verified);
        }
    }
}

// Test substitutions are task-local and absent from release builds. Production endpoints stay fixed.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[derive(Clone)]
    pub struct Settings {
        urls: [String; 4],
        pub stun_servers: Vec<(String, u16)>,
    }
    tokio::task_local! { pub static SETTINGS: Settings; }
    pub fn endpoint(url: &str) -> Option<String> {
        let index = std::iter::once("https://www.cloudflare.com/cdn-cgi/trace")
            .chain(GEO_ENDPOINTS)
            .position(|fixed| fixed == url)?;
        SETTINGS.try_with(|s| s.urls[index].clone()).ok()
    }
    pub struct Fixture {
        pub settings: Settings,
        pub requests: Arc<Mutex<Vec<usize>>>,
        tasks: Vec<tokio::task::JoinHandle<()>>,
    }
    impl Fixture {
        pub async fn new(responses: [Option<&'static str>; 4]) -> Self {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let requests = Arc::new(Mutex::new(Vec::new()));
            let captured = requests.clone();
            let http = tokio::spawn(async move {
                let mut children = tokio::task::JoinSet::new();
                loop {
                    tokio::select! {
                        accepted = listener.accept() => {
                            let Ok((mut socket, _)) = accepted else { break; };
                            let captured = captured.clone();
                            children.spawn(async move {
                                let mut bytes = [0; 2048];
                                let length = socket.read(&mut bytes).await.unwrap();
                                let request = std::str::from_utf8(&bytes[..length]).unwrap();
                                let index: usize = request.split_whitespace().nth(1).unwrap()
                                    .trim_start_matches('/').parse().unwrap();
                                captured.lock().unwrap().push(index);
                                if let Some(body) = responses[index] {
                                    let reply = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
                                    let _ = socket.write_all(reply.as_bytes()).await;
                                } else {
                                    tokio::time::sleep(Duration::from_secs(60)).await;
                                }
                            });
                        }
                        _ = children.join_next(), if !children.is_empty() => {}
                    }
                }
            });
            let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let port = socket.local_addr().unwrap().port();
            let udp = tokio::spawn(async move {
                let mut bytes = [0; 1024];
                loop {
                    let Ok((length, peer)) = socket.recv_from(&mut bytes).await else {
                        break;
                    };
                    assert_eq!(length, 20);
                    let mut response = vec![1, 1, 0, 12, 0x21, 0x12, 0xa4, 0x42];
                    response.extend_from_slice(&bytes[8..20]);
                    response.extend_from_slice(&[0, 1, 0, 8, 0, 1, 0, 0, 203, 0, 113, 8]);
                    let _ = socket.send_to(&response, peer).await;
                }
            });
            Self {
                settings: Settings {
                    urls: std::array::from_fn(|n| format!("{base}/{n}")),
                    stun_servers: vec![("127.0.0.1".into(), port); 4],
                },
                requests,
                tasks: vec![http, udp],
            }
        }
        pub fn client(&self) -> Client {
            Client::builder()
                .no_proxy()
                .redirect(Policy::none())
                .timeout(Duration::from_millis(200))
                .build()
                .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            for task in &self.tasks {
                task.abort();
            }
        }
    }
}
