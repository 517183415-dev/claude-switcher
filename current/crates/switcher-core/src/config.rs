//! Read-only configuration discovery. Never source a shell file.
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckConfig {
    pub country_target: Option<String>,
    pub timezone_target: Option<String>,
    pub locale_target: Option<String>,
    #[serde(default = "auto")]
    pub proxy_mode: String,
    #[serde(default = "wifi")]
    pub network_service: String,
    #[serde(default)]
    pub previous_egress_hash: Option<String>,
}

fn auto() -> String {
    "auto".into()
}
fn wifi() -> String {
    "Wi-Fi".into()
}

impl Default for CheckConfig {
    fn default() -> Self {
        Self {
            country_target: None,
            timezone_target: None,
            locale_target: None,
            proxy_mode: auto(),
            network_service: wifi(),
            previous_egress_hash: None,
        }
    }
}

impl CheckConfig {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.country_target.as_ref().is_some_and(|v| {
            v.len() != 2 || !v.bytes().all(|b| b.is_ascii_uppercase()) || v == "CN"
        }) {
            return Err("country_target_invalid");
        }
        if self
            .timezone_target
            .as_ref()
            .is_some_and(|v| !valid_timezone(v))
        {
            return Err("timezone_target_invalid");
        }
        if self.locale_target.as_ref().is_some_and(|v| {
            v.len() > 32
                || !v
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'@' | b'='))
        }) {
            return Err("locale_target_invalid");
        }
        if !matches!(
            self.proxy_mode.as_str(),
            "auto" | "clash-verge" | "0dcloud" | "shadowrocket" | "sing-box" | "clashx" | "surge"
        ) {
            return Err("proxy_mode_invalid");
        }
        if self.network_service.is_empty()
            || self.network_service.len() > 128
            || self.network_service.chars().any(char::is_control)
        {
            return Err("network_service_invalid");
        }
        if self
            .previous_egress_hash
            .as_ref()
            .is_some_and(|s| s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("previous_egress_hash_invalid");
        }
        Ok(())
    }
}

pub fn valid_timezone(zone: &str) -> bool {
    !zone.is_empty()
        && zone.len() <= 80
        && !zone.starts_with('/')
        && !zone
            .split('/')
            .any(|s| s == ".." || s == "." || s.is_empty())
        && zone
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'-' | b'+'))
        && Path::new("/usr/share/zoneinfo").join(zone).is_file()
}

pub fn home_dir() -> Result<PathBuf, &'static str> {
    std::env::var_os("HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .ok_or("home_unavailable")
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, &'static str> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|_| "configuration_unreadable")?;
    if !file
        .metadata()
        .map_err(|_| "configuration_unreadable")?
        .is_file()
    {
        return Err("configuration_not_regular");
    }
    let mut data = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut data)
        .map_err(|_| "configuration_unreadable")?;
    if data.len() as u64 > limit {
        return Err("configuration_too_large");
    }
    Ok(data)
}

pub fn load_read_only(home: &Path) -> Result<CheckConfig, &'static str> {
    let path = home.join(".claude-switcher/v2/config.json");
    if path.exists() {
        let data = read_bounded(&path, 16 * 1024)?;
        let config: CheckConfig =
            serde_json::from_slice(&data).map_err(|_| "configuration_invalid")?;
        config.validate()?;
        return Ok(config);
    }
    let legacy = home.join(".local/bin/claude-switch.sh");
    let mut config = CheckConfig::default();
    if legacy.exists() {
        let data = String::from_utf8(read_bounded(&legacy, 512 * 1024)?)
            .map_err(|_| "legacy_configuration_unreadable")?;
        for line in data.lines() {
            for (key, target) in [
                ("COUNTRY_TARGET", &mut config.country_target),
                ("TZ_TARGET", &mut config.timezone_target),
                ("LOCALE_TARGET", &mut config.locale_target),
            ] {
                if let Some(value) = literal_assignment(line, key) {
                    *target = Some(value);
                }
            }
            if let Some(value) = literal_assignment(line, "PROXY_MODE") {
                config.proxy_mode = value;
            }
        }
    }
    config.validate()?;
    Ok(config)
}

pub(crate) fn literal_assignment(line: &str, key: &str) -> Option<String> {
    let source = line.strip_prefix(key)?.strip_prefix('=')?;
    let (value, tail) = if source.starts_with('\'') {
        // Decode only the single-quoted literal form emitted by the local installer.
        let mut remaining = source;
        let mut value = String::new();
        loop {
            let quoted = remaining.strip_prefix('\'')?;
            let (part, tail) = quoted.split_once('\'')?;
            value.push_str(part);
            if let Some(next) = tail.strip_prefix("\\'") {
                value.push('\'');
                remaining = next;
            } else {
                break (value, tail);
            }
        }
    } else {
        let (value, tail) = source.strip_prefix('"')?.split_once('"')?;
        if key != "PROXY_GROUP" && value.contains(['$', '`', '\\']) {
            return None;
        }
        (value.to_owned(), tail)
    };
    if !tail.trim_start().is_empty() && !tail.trim_start().starts_with('#') {
        return None;
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shell_configuration_is_never_evaluated() {
        assert_eq!(
            literal_assignment("COUNTRY_TARGET=\"US\" # saved", "COUNTRY_TARGET"),
            Some("US".into())
        );
        assert_eq!(
            literal_assignment("COUNTRY_TARGET=\"$(touch /tmp/no)\"", "COUNTRY_TARGET"),
            None
        );
        assert_eq!(
            literal_assignment("COUNTRY_TARGET=\"US\"; evil", "COUNTRY_TARGET"),
            None
        );
        assert!(!valid_timezone("../../etc/passwd"));
    }

    #[test]
    fn installed_shim_literals_preserve_group_text_without_shell_evaluation() {
        assert_eq!(
            literal_assignment("TZ_TARGET='America/Los_Angeles' # saved", "TZ_TARGET"),
            Some("America/Los_Angeles".into())
        );
        assert_eq!(
            literal_assignment("PROXY_GROUP='Bob'\\''s $Group'", "PROXY_GROUP"),
            Some("Bob's $Group".into())
        );
        assert_eq!(
            literal_assignment("PROXY_GROUP=\"$Group\"", "PROXY_GROUP"),
            Some("$Group".into())
        );
        assert_eq!(
            literal_assignment("COUNTRY_TARGET='US'; evil", "COUNTRY_TARGET"),
            None
        );
        assert_eq!(
            literal_assignment("COUNTRY_TARGET='US'$(evil)", "COUNTRY_TARGET"),
            None
        );
    }
}
