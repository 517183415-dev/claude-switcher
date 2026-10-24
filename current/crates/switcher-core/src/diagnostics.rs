use crate::network;
use serde::Deserialize;
use std::{net::IpAddr, path::Path};
use switcher_domain::checks::{DiagnosticReport, ProxyDescription, TransportState};

// Deliberately no Debug/Serialize: private proxy credentials never become a DTO.
pub struct ProxyConfig {
    url: String,
    pub description: ProxyDescription,
}

impl ProxyConfig {
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        let value = value.trim();
        if value.is_empty() || value.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return Err("invalid_or_missing_proxy");
        }
        let url = url::Url::parse(value).map_err(|_| "invalid_or_missing_proxy")?;
        if !matches!(url.scheme(), "http" | "https" | "socks5" | "socks5h")
            || url.host_str().is_none()
            || !matches!(url.path(), "" | "/")
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("invalid_or_missing_proxy");
        }
        let authority = value
            .split_once("://")
            .ok_or("invalid_or_missing_proxy")?
            .1
            .split('/')
            .next()
            .ok_or("invalid_or_missing_proxy")?;
        let port = authority
            .rsplit_once(':')
            .and_then(|(_, port)| port.parse::<u16>().ok())
            .filter(|n| *n > 0)
            .ok_or("proxy_explicit_port_required")?;
        if url.port_or_known_default() != Some(port) {
            return Err("invalid_or_missing_proxy");
        }
        let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
        let loopback = host.eq_ignore_ascii_case("localhost")
            || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
        let dns = match url.scheme() {
            "socks5" => "local",
            "socks5h" => "remote",
            _ => "proxy_connect",
        };
        Ok(Self {
            url: value.into(),
            description: ProxyDescription {
                scheme: url.scheme().into(),
                host_scope: if loopback { "loopback" } else { "remote" }.into(),
                dns: dns.into(),
            },
        })
    }
    pub fn from_private_file(path: &Path) -> Result<Self, &'static str> {
        #[derive(Deserialize)]
        struct Private {
            proxy_url: String,
        }
        // This is an explicitly selected, read-only input. Legacy configs may
        // contain unrelated fields, be shared-readable, or use a symlink.
        let bytes = std::fs::read(path).map_err(|_| "proxy_configuration_unreadable")?;
        let data: Private =
            serde_json::from_slice(&bytes).map_err(|_| "proxy_configuration_invalid")?;
        Self::parse(&data.proxy_url)
    }
}

pub async fn diagnose_proxy(config: ProxyConfig) -> Result<DiagnosticReport, &'static str> {
    let client = network::client(Some(&config.url))?;
    let (a, b) = tokio::join!(
        network::transport_probe(
            &client,
            network::SERVICE_ENDPOINTS[0].0,
            network::SERVICE_ENDPOINTS[0].1
        ),
        network::transport_probe(
            &client,
            network::SERVICE_ENDPOINTS[1].0,
            network::SERVICE_ENDPOINTS[1].1
        )
    );
    let probes = vec![a, b];
    Ok(DiagnosticReport {
        schema_version: 1,
        proxy: config.description,
        transport_reached: probes
            .iter()
            .all(|p| p.transport == TransportState::Reached && p.tls_verified),
        probes,
        launch_allowed: None,
        system_modified: false,
    })
}

/// Apply the same redaction to preview, persistence and export. Avoid raw platform output in the first place.
pub fn redact(text: &str) -> String {
    text.lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if [
                "authorization",
                "cookie",
                "password",
                "secret",
                "token",
                "api_key",
                "proxy_url",
            ]
            .iter()
            .any(|key| lower.contains(key))
            {
                return "[敏感字段已隐藏]".into();
            }
            let mut line = line.to_owned();
            if let Some(home) = std::env::var_os("HOME") {
                let home = home.to_string_lossy();
                if !home.is_empty() {
                    line = line.replace(home.as_ref(), "[本机目录]");
                }
            }
            line.split_whitespace()
                .map(|word| {
                    if word.contains("://") {
                        if let Ok(url) = url::Url::parse(word.trim_matches(['(', ')', ',', ';'])) {
                            let public = matches!(
                                url.host_str(),
                                Some(
                                    "claude.ai"
                                        | "api.anthropic.com"
                                        | "ipinfo.io"
                                        | "ipwho.is"
                                        | "api.ip.sb"
                                        | "www.cloudflare.com"
                                )
                            );
                            if !public
                                || !url.username().is_empty()
                                || url.query().is_some()
                                || url.fragment().is_some()
                            {
                                return "[网络地址已隐藏]".to_owned();
                            }
                        } else {
                            return "[网络地址已隐藏]".to_owned();
                        }
                    }
                    let trimmed = word.trim_matches(['[', ']', '(', ')', ',', ';']);
                    if trimmed.parse::<IpAddr>().is_ok()
                        || trimmed.parse::<std::net::SocketAddr>().is_ok()
                        || word
                            .split(['=', '/', ',', ';', '(', ')', '→'])
                            .any(|s| s.trim_matches(['[', ']']).parse::<IpAddr>().is_ok())
                    {
                        "[网络地址已隐藏]".to_owned()
                    } else {
                        redact_ip_literals(word)
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn redact_ip_literals(value: &str) -> String {
    let mut output = String::new();
    let mut token = String::new();
    let flush = |token: &mut String, output: &mut String| {
        if token.is_empty() {
            return;
        }
        let candidate = token.trim_matches(['[', ']']).trim_end_matches('.');
        if candidate.parse::<IpAddr>().is_ok() || token.parse::<std::net::SocketAddr>().is_ok() {
            output.push_str("[网络地址已隐藏]");
        } else {
            output.push_str(token);
        }
        token.clear();
    };
    for character in value.chars() {
        if character.is_ascii_hexdigit() || matches!(character, ':' | '.' | '[' | ']') {
            token.push(character);
        } else {
            flush(&mut token, &mut output);
            output.push(character);
        }
    }
    flush(&mut token, &mut output);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_config_accepts_unknown_fields_permissions_symlink_and_whitespace() {
        let dir = std::env::temp_dir().join(format!(
            "diagnostic-config-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("proxy.json");
        std::fs::write(
            &path,
            r#"{"proxy_url":"  http://127.0.0.1:8080\n","note":"legacy"}"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            let link = dir.join("link.json");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(ProxyConfig::from_private_file(&link).is_ok());
        }
        assert!(ProxyConfig::from_private_file(&path).is_ok());
        assert!(ProxyConfig::parse("http://127.0.0.1:80 bad").is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn explicit_proxy_validation_and_socks_dns_are_private() {
        for value in [
            "file:///secret",
            "http://user:secret@host:bad",
            "http://host:80/path",
            "http://host:80\nproxy-user=secret",
            "http://host",
            "http://host:0",
        ] {
            assert!(ProxyConfig::parse(value).is_err());
        }
        for value in [
            "http://user:private-password@remote.example:80",
            "http://127.0.0.1:18080",
            "socks5h://[::1]:18080",
        ] {
            let proxy = ProxyConfig::parse(value).unwrap();
            let output = serde_json::to_string(&proxy.description).unwrap();
            assert!(!output.contains("private-password"));
            assert!(!output.contains("remote.example"));
        }
        assert_eq!(
            ProxyConfig::parse("socks5://localhost:1080")
                .unwrap()
                .description
                .dns,
            "local"
        );
        assert_eq!(
            ProxyConfig::parse("socks5h://localhost:1080")
                .unwrap()
                .description
                .dns,
            "remote"
        );
    }
    #[test]
    fn redaction_is_shared_and_idempotent() {
        let input = "ip=203.0.113.4 [2001:db8::1] http://user:private@proxy.example:80\nAuthorization: private-token";
        let output = redact(input);
        for secret in ["203.0.113.4", "2001:db8::1", "private", "proxy.example"] {
            assert!(!output.contains(secret));
        }
        assert_eq!(redact(&output), output);
        for input in [
            r#"{"ip":"203.0.113.4"}"#,
            "出口203.0.113.4国家",
            "peer=[2001:db8::1]:443",
            "203.0.113.4.",
        ] {
            assert!(!redact(input).contains("203.0.113.4"));
            assert!(!redact(input).contains("2001:db8::1"));
        }
    }
    #[tokio::test]
    async fn dead_proxy_does_not_fallback_or_modify_state() {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            diagnose_proxy(ProxyConfig::parse("http://127.0.0.1:1").unwrap()),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!result.transport_reached);
        assert_eq!(result.launch_allowed, None);
        assert!(!result.system_modified);
        assert!(
            result
                .probes
                .iter()
                .all(|p| !p.tls_verified && p.http_status.is_none())
        );
    }
}
