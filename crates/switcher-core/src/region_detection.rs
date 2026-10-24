//! Detect the legacy country/timezone target; Cloudflare is an optional cross-check.
use crate::{config, network};
use reqwest::Client;
use switcher_domain::desktop::RegionView;

const TRACE_URL: &str = "https://www.cloudflare.com/cdn-cgi/trace";

pub struct DetectedRegion(RegionView);

impl DetectedRegion {
    pub(crate) fn into_view(self) -> RegionView {
        self.0
    }
}

fn trace_country(bytes: &[u8]) -> Option<&str> {
    let trace = std::str::from_utf8(bytes).ok()?;
    let country = trace.lines().find_map(|line| line.strip_prefix("loc="))?;
    valid_country(country).then_some(country)
}

fn valid_country(country: &str) -> bool {
    country.len() == 2 && country.bytes().all(|byte| byte.is_ascii_uppercase())
}

fn locale_for(country: &str) -> &'static str {
    match country {
        "JP" => "ja_JP",
        "TW" => "zh_TW",
        "HK" => "zh_HK",
        "SG" => "en_SG",
        "GB" => "en_GB",
        "DE" => "de_DE",
        "FR" => "fr_FR",
        "KR" => "ko_KR",
        "AU" => "en_AU",
        "CA" => "en_CA",
        _ => "en_US",
    }
}

fn checked_target(
    trace: Option<&str>,
    geo: &network::GeoInfo,
) -> Result<Option<DetectedRegion>, &'static str> {
    let Some(country) = geo
        .country
        .as_deref()
        .filter(|country| valid_country(country))
    else {
        return Ok(None);
    };
    if country == "CN" {
        return Err("出口国家为 CN；请先切换代理节点");
    }
    if trace.is_some_and(|observed| observed != country) {
        return Err("出口国家来源不一致；地区目标未更改");
    }
    let Some(timezone) = geo.timezone.as_deref() else {
        return Ok(None);
    };
    if !config::valid_timezone(timezone) {
        return Err("出口时区无效；地区目标未更改");
    }
    Ok(Some(DetectedRegion(RegionView {
        country: country.into(),
        timezone: timezone.into(),
        locale: locale_for(country).into(),
    })))
}

pub async fn detect(client: &Client) -> Result<DetectedRegion, &'static str> {
    let trace = network::limited_get(client, TRACE_URL, 8192).await.ok();
    let country = trace.as_deref().and_then(trace_country);
    if country == Some("CN") {
        return Err("出口国家为 CN；请先切换代理节点");
    }
    for endpoint in network::GEO_ENDPOINTS {
        let Ok(bytes) = network::limited_get(client, endpoint, network::MAX_RESPONSE_BYTES).await
        else {
            continue;
        };
        let Some(geo) = network::parse_geo(&bytes) else {
            continue;
        };
        if let Some(target) = checked_target(country, &geo)? {
            return Ok(target);
        }
    }
    Err("无法确认出口国家和时区；地区目标未更改")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn geo(country: &str, timezone: Option<&str>) -> network::GeoInfo {
        network::GeoInfo {
            ip: Some("203.0.113.9".parse().unwrap()),
            country: Some(country.into()),
            timezone: timezone.map(str::to_owned),
            organization: None,
        }
    }
    #[test]
    fn legacy_target_accepts_geo_without_cloudflare_or_matching_ip() {
        for trace in [None, Some("US")] {
            let view = checked_target(trace, &geo("US", Some("America/New_York")))
                .unwrap()
                .unwrap()
                .into_view();
            assert_eq!(view.timezone, "America/New_York");
            assert_eq!(view.locale, "en_US");
        }
        assert_eq!(trace_country(b"loc=US\n"), Some("US"));
    }
    #[test]
    fn conflicting_country_cn_and_missing_timezone_fail() {
        assert!(checked_target(Some("US"), &geo("JP", Some("Asia/Tokyo"))).is_err());
        assert!(checked_target(None, &geo("CN", Some("Asia/Shanghai"))).is_err());
        assert!(checked_target(None, &geo("US", None)).unwrap().is_none());
    }
    #[tokio::test]
    async fn missing_cloudflare_and_different_ip_use_geo_fallback() {
        use crate::network::test_support::{Fixture, SETTINGS};
        for trace in [None, Some("loc=US\nip=203.0.113.8\n")] {
            let body = r#"{"ip":"203.0.113.9","country":"US","timezone":"America/New_York"}"#;
            let fixture = Fixture::new([trace, Some(body), Some(body), Some(body)]).await;
            SETTINGS
                .scope(fixture.settings.clone(), async {
                    assert_eq!(
                        detect(&fixture.client())
                            .await
                            .unwrap()
                            .into_view()
                            .timezone,
                        "America/New_York"
                    );
                })
                .await;
        }
    }
}
