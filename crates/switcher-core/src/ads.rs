//! Signed advertisement content, local scheduling and bounded static images.
use crate::signed;
use ed25519_dalek::VerifyingKey;
use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader, Limits};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io::Cursor, sync::Mutex};
use switcher_domain::AdSlot;
pub use switcher_domain::ads::*;
use url::{Host, Url};

pub const MAX_MANIFEST_BYTES: usize = 256 * 1024;
pub const MAX_IMAGE_BYTES: usize = 1024 * 1024;
pub const MAX_IMAGE_PIXELS: u64 = 2_000_000;
pub const MAX_MANIFEST_LIFETIME: u64 = 90 * 24 * 60 * 60;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
// ponytail: serialize six-or-fewer decodes; add a bounded pool only if measured latency needs it.
static IMAGE_DECODE: Mutex<()> = Mutex::new(());

pub fn slot_index(slot: AdSlot) -> usize {
    match slot {
        AdSlot::ResidentialBroadband => 0,
        AdSlot::AccountService => 1,
        AdSlot::KycService => 2,
    }
}

fn bounded_text(value: &str, limit: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= limit
        && !value.chars().any(char::is_control)
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn valid_dimensions(width: u32, height: u32) -> bool {
    width > 0 && height > 0 && u64::from(width) * u64::from(height) <= MAX_IMAGE_PIXELS
}

pub fn validate_asset(asset: &ImageAsset) -> Result<(), &'static str> {
    if !valid_hash(&asset.sha256) {
        return Err("image_hash");
    }
    if !matches!(asset.mime.as_str(), "image/png" | "image/jpeg") {
        return Err("image_mime");
    }
    if asset.bytes == 0 || asset.bytes as usize > MAX_IMAGE_BYTES {
        return Err("image_size");
    }
    if !valid_dimensions(asset.width, asset.height) {
        return Err("image_dimensions");
    }
    Ok(())
}

/// Canonical browser target and the exact host shown before the first click.
pub fn normalize_merchant_url(raw: &str) -> Result<(String, String), &'static str> {
    if raw.len() > 2048 || raw.trim() != raw || raw.chars().any(char::is_control) {
        return Err("merchant_url");
    }
    let authority = raw
        .split_once("://")
        .map(|(_, tail)| tail.split(['/', '?', '#']).next().unwrap_or_default())
        .ok_or("merchant_url")?;
    if authority.contains('@') {
        return Err("merchant_userinfo");
    }
    let mut url = Url::parse(raw).map_err(|_| "merchant_url")?;
    if url.scheme() != "https" {
        return Err("merchant_scheme");
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("merchant_userinfo");
    }
    if url.port_or_known_default() != Some(443) {
        return Err("merchant_port");
    }
    let host = match url.host() {
        Some(Host::Domain(host)) => host.trim_end_matches('.').to_ascii_lowercase(),
        _ => return Err("merchant_host"),
    };
    let labels: Vec<_> = host.split('.').collect();
    if host.len() > 253
        || labels.len() < 2
        || labels.iter().any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        return Err("merchant_host");
    }
    let tld = *labels.last().ok_or("merchant_host")?;
    if !tld.bytes().any(|b| b.is_ascii_alphabetic())
        || [
            "localhost",
            "local",
            "localdomain",
            "internal",
            "home",
            "lan",
            "test",
            "invalid",
            "example",
            "onion",
            "alt",
            "arpa",
        ]
        .contains(&tld)
    {
        return Err("merchant_host");
    }
    url.set_host(Some(&host)).map_err(|_| "merchant_host")?;
    Ok((url.into(), host))
}

pub fn click_requires_confirmation(
    consents: &[MerchantConsent],
    slot: AdSlot,
    campaign: &Campaign,
) -> Result<bool, &'static str> {
    let (_, host) = normalize_merchant_url(&campaign.target_url)?;
    Ok(!consents.iter().any(|consent| {
        consent.slot == slot
            && consent.merchant_id == campaign.merchant_id
            && consent.normalized_host == host
    }))
}

pub fn record_click_consent(
    consents: &mut Vec<MerchantConsent>,
    slot: AdSlot,
    campaign: &Campaign,
) -> Result<(), &'static str> {
    let (_, normalized_host) = normalize_merchant_url(&campaign.target_url)?;
    consents.retain(|consent| consent.slot != slot);
    consents.push(MerchantConsent {
        slot,
        merchant_id: campaign.merchant_id.clone(),
        normalized_host,
    });
    Ok(())
}

pub fn validate_manifest(
    manifest: &AdManifest,
    channel: AdChannel,
    key_id: &str,
    now: u64,
) -> Result<(), &'static str> {
    if manifest.schema_version != 1 {
        return Err("manifest_schema");
    }
    if manifest.channel != channel || manifest.key_id != key_id {
        return Err("manifest_scope");
    }
    if !valid_id(&manifest.key_id) || !valid_id(&manifest.release_id) {
        return Err("manifest_id");
    }
    if manifest.content_revision == 0 || manifest.content_revision > MAX_SAFE_INTEGER {
        return Err("manifest_revision");
    }
    if manifest.issued_at > now
        || manifest.expires_at <= now
        || manifest.expires_at <= manifest.issued_at
        || manifest.expires_at - manifest.issued_at > MAX_MANIFEST_LIFETIME
        || manifest.expires_at > MAX_SAFE_INTEGER
    {
        return Err("manifest_time");
    }
    if manifest.slots.len() != 3 {
        return Err("manifest_slots");
    }
    let mut seen_slots = [false; 3];
    let mut ids = HashSet::new();
    for slot in &manifest.slots {
        let index = slot_index(slot.slot);
        if seen_slots[index] {
            return Err("manifest_slots");
        }
        seen_slots[index] = true;
        if slot.campaigns.len() > 2 {
            return Err("campaign_count");
        }
        for campaign in &slot.campaigns {
            if !valid_id(&campaign.campaign_id)
                || !valid_id(&campaign.merchant_id)
                || !ids.insert(&campaign.campaign_id)
            {
                return Err("campaign_id");
            }
            if !bounded_text(&campaign.merchant_name, 128)
                || !bounded_text(&campaign.title, 80)
                || !bounded_text(&campaign.description, 500)
                || !bounded_text(&campaign.button_text, 40)
            {
                return Err("campaign_text");
            }
            if campaign.starts_at >= campaign.ends_at || campaign.ends_at > MAX_SAFE_INTEGER {
                return Err("campaign_time");
            }
            normalize_merchant_url(&campaign.target_url)?;
            validate_asset(&campaign.image)?;
        }
        if let [first, second] = slot.campaigns.as_slice()
            && first.starts_at < second.ends_at
            && second.starts_at < first.ends_at
        {
            return Err("campaign_overlap");
        }
    }
    Ok(())
}

pub fn verify_manifest(
    wire: &[u8],
    key: &VerifyingKey,
    channel: AdChannel,
    key_id: &str,
    now: u64,
    time_trusted: bool,
) -> Result<(AdManifest, String), &'static str> {
    if !time_trusted {
        return Err("clock_untrusted");
    }
    let (manifest, hash) = signed::verify(wire, key, MAX_MANIFEST_BYTES)?;
    validate_manifest(&manifest, channel, key_id, now)?;
    Ok((manifest, hash))
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdCache {
    pub highest_revision: u64,
    pub highest_payload_hash: Option<String>,
    pub last_success: Option<u64>,
    pub cached_wire: Option<Vec<u8>>,
    #[serde(skip)]
    manifest: Option<AdManifest>,
}

impl AdCache {
    pub fn accept(
        &mut self,
        wire: &[u8],
        key: &VerifyingKey,
        channel: AdChannel,
        key_id: &str,
        now: u64,
        time_trusted: bool,
    ) -> Result<(), &'static str> {
        let (manifest, hash) = verify_manifest(wire, key, channel, key_id, now, time_trusted)?;
        if manifest.content_revision < self.highest_revision {
            return Err("old_revision");
        }
        if manifest.content_revision == self.highest_revision
            && self.highest_payload_hash.as_deref() != Some(&hash)
        {
            return Err("revision_conflict");
        }
        self.highest_revision = manifest.content_revision;
        self.highest_payload_hash = Some(hash);
        self.last_success = Some(now);
        self.cached_wire = Some(wire.to_vec());
        self.manifest = Some(manifest);
        Ok(())
    }

    /// Restoring disk cache never extends the offline deadline.
    pub fn restore_verified(
        &mut self,
        key: &VerifyingKey,
        channel: AdChannel,
        key_id: &str,
        now: u64,
        time_trusted: bool,
    ) -> Result<(), &'static str> {
        self.manifest = None;
        if self.last_success.is_none_or(|last| last > now) {
            return Err("cache_time");
        }
        let wire = self.cached_wire.as_deref().ok_or("cache_empty")?;
        let (manifest, hash) = verify_manifest(wire, key, channel, key_id, now, time_trusted)?;
        if manifest.content_revision != self.highest_revision
            || self.highest_payload_hash.as_deref() != Some(&hash)
        {
            return Err("cache_revision");
        }
        self.manifest = Some(manifest);
        Ok(())
    }

    pub fn manifest(&self) -> Option<&AdManifest> {
        self.manifest.as_ref()
    }

    pub fn current_campaign(
        &self,
        slot: AdSlot,
        now: u64,
        preferences: &AdPreferences,
        time_trusted: bool,
    ) -> Option<&Campaign> {
        if !time_trusted
            || !preferences.request_purpose_acknowledged
            || !preferences.slots_enabled[slot_index(slot)]
        {
            return None;
        }
        let manifest = self.manifest.as_ref()?;
        let last_success = self.last_success?;
        if last_success > now {
            return None;
        }
        let offline_deadline = last_success
            .checked_add(24 * 60 * 60)?
            .min(manifest.expires_at);
        if now >= offline_deadline {
            return None;
        }
        let slot = manifest
            .slots
            .iter()
            .find(|content| content.slot == slot && content.enabled)?;
        slot.campaigns.iter().find(|campaign| {
            campaign.enabled && campaign.starts_at <= now && now < campaign.ends_at
        })
    }

    pub fn next_campaign(&self, slot: AdSlot, now: u64) -> Option<&Campaign> {
        self.manifest
            .as_ref()?
            .slots
            .iter()
            .find(|content| content.slot == slot && content.enabled)?
            .campaigns
            .iter()
            .filter(|campaign| campaign.enabled && campaign.starts_at > now)
            .min_by_key(|campaign| campaign.starts_at)
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshSchedule {
    pub next_check_at: Option<u64>,
    pub last_manual_at: Option<u64>,
    pub failures: u8,
    #[serde(skip)]
    in_flight: bool,
}

impl RefreshSchedule {
    /// Returns one request permit. All-disabled and concurrent calls get no permit.
    pub fn begin(
        &mut self,
        now: u64,
        preferences: &AdPreferences,
        time_trusted: bool,
        manual: bool,
    ) -> bool {
        if self.in_flight
            || !time_trusted
            || !preferences.request_purpose_acknowledged
            || !preferences.slots_enabled.iter().any(|enabled| *enabled)
        {
            return false;
        }
        if manual {
            if self
                .last_manual_at
                .is_some_and(|last| now < last.saturating_add(30))
            {
                return false;
            }
            self.last_manual_at = Some(now);
        } else if self.next_check_at.is_some_and(|next| now < next) {
            return false;
        }
        self.in_flight = true;
        true
    }

    pub fn complete_success(&mut self, now: u64, jitter_seconds: u8) -> Result<(), &'static str> {
        if jitter_seconds > 60 {
            return Err("refresh_jitter");
        }
        let next = now
            .checked_add(15 * 60 + u64::from(jitter_seconds))
            .ok_or("refresh_time")?;
        self.next_check_at = Some(next);
        self.failures = 0;
        self.in_flight = false;
        Ok(())
    }

    pub fn complete_failure(&mut self, now: u64) -> Result<(), &'static str> {
        const BACKOFF: [u64; 5] = [60, 300, 900, 1800, 3600];
        let delay = BACKOFF[usize::from(self.failures).min(BACKOFF.len() - 1)];
        self.next_check_at = Some(now.checked_add(delay).ok_or("refresh_time")?);
        self.failures = self.failures.saturating_add(1);
        self.in_flight = false;
        Ok(())
    }
}

/// Only a source fixed by product configuration can construct content requests.
#[derive(Debug, Clone)]
pub struct ContentSource {
    base: Url,
}

impl ContentSource {
    #[cfg(test)]
    pub(crate) fn fixture(base: Url) -> Self {
        Self { base }
    }

    pub fn new(fixed_base: &str) -> Result<Self, &'static str> {
        let (normalized, _) = normalize_merchant_url(fixed_base)?;
        let base = Url::parse(&normalized).map_err(|_| "content_source")?;
        if base.query().is_some() || base.fragment().is_some() || !base.path().ends_with('/') {
            return Err("content_source");
        }
        Ok(Self { base })
    }

    pub fn manifest_request(&self, preferences: &AdPreferences, channel: AdChannel) -> Option<Url> {
        if !preferences.request_purpose_acknowledged
            || !preferences.slots_enabled.iter().any(|enabled| *enabled)
        {
            return None;
        }
        self.base
            .join(match channel {
                AdChannel::Preview => "preview/current.json",
                AdChannel::Production => "production/current.json",
            })
            .ok()
    }

    pub fn asset_request(
        &self,
        slot: AdSlot,
        asset: &ImageAsset,
        preferences: &AdPreferences,
        channel: AdChannel,
    ) -> Result<Option<Url>, &'static str> {
        if !preferences.request_purpose_acknowledged || !preferences.slots_enabled[slot_index(slot)]
        {
            return Ok(None);
        }
        validate_asset(asset)?;
        let extension = if asset.mime == "image/png" {
            "png"
        } else {
            "jpg"
        };
        let channel = match channel {
            AdChannel::Preview => "preview",
            AdChannel::Production => "production",
        };
        self.base
            .join(&format!("{channel}/assets/{}.{}", asset.sha256, extension))
            .map(Some)
            .map_err(|_| "content_source")
    }

    pub fn permits_response_url(&self, requested: &Url, response: &Url) -> bool {
        if requested != response
            || requested.origin() != self.base.origin()
            || requested.query().is_some()
            || requested.fragment().is_some()
        {
            return false;
        }
        let Some(relative) = requested.as_str().strip_prefix(self.base.as_str()) else {
            return false;
        };
        let Some((channel, object)) = relative.split_once('/') else {
            return false;
        };
        if !matches!(channel, "preview" | "production") {
            return false;
        }
        if object == "current.json" {
            return true;
        }
        let Some(asset) = object.strip_prefix("assets/") else {
            return false;
        };
        let Some((hash, extension)) = asset.rsplit_once('.') else {
            return false;
        };
        valid_hash(hash) && matches!(extension, "png" | "jpg")
    }
}

fn png_is_static(bytes: &[u8]) -> Result<(), &'static str> {
    let mut offset = 8usize;
    let mut first = true;
    while offset < bytes.len() {
        let header = bytes
            .get(offset..offset.checked_add(8).ok_or("image_chunk")?)
            .ok_or("image_chunk")?;
        let length =
            u32::from_be_bytes(header[..4].try_into().map_err(|_| "image_chunk")?) as usize;
        let kind = &header[4..8];
        if first && (kind != b"IHDR" || length != 13) {
            return Err("image_chunk");
        }
        first = false;
        if matches!(kind, b"acTL" | b"fcTL" | b"fdAT") {
            return Err("image_animation");
        }
        offset = offset
            .checked_add(12)
            .and_then(|n| n.checked_add(length))
            .ok_or("image_chunk")?;
        if offset > bytes.len() {
            return Err("image_chunk");
        }
        if kind == b"IEND" {
            return if length == 0 && offset == bytes.len() {
                Ok(())
            } else {
                Err("image_chunk")
            };
        }
    }
    Err("image_chunk")
}

fn decode_image(bytes: &[u8], mime: &str) -> Result<DynamicImage, &'static str> {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err("image_size");
    }
    let format = match mime {
        "image/png" if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => {
            png_is_static(bytes)?;
            ImageFormat::Png
        }
        "image/jpeg" if bytes.starts_with(b"\xff\xd8\xff") => ImageFormat::Jpeg,
        _ => return Err("image_mime"),
    };
    let mut header = ImageReader::with_format(Cursor::new(bytes), format);
    let mut header_limits = Limits::default();
    header_limits.max_image_width = Some(MAX_IMAGE_PIXELS as u32);
    header_limits.max_image_height = Some(MAX_IMAGE_PIXELS as u32);
    header_limits.max_alloc = Some(32 * 1024 * 1024);
    header.limits(header_limits);
    let (width, height) = header.into_dimensions().map_err(|_| "image_header")?;
    if !valid_dimensions(width, height) {
        return Err("image_dimensions");
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(width);
    limits.max_image_height = Some(height);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(|_| "image_decode")
}

pub fn validate_image(bytes: &[u8], asset: &ImageAsset) -> Result<(), &'static str> {
    validate_asset(asset)?;
    if bytes.len() != asset.bytes as usize || bytes.len() > MAX_IMAGE_BYTES {
        return Err("image_size");
    }
    if signed::digest(bytes) != asset.sha256 {
        return Err("image_hash");
    }
    let _guard = IMAGE_DECODE
        .lock()
        .map_err(|_| "image_decoder_unavailable")?;
    let decoded = decode_image(bytes, &asset.mime)?;
    if decoded.dimensions() != (asset.width, asset.height) {
        return Err("image_dimensions");
    }
    Ok(())
}

/// Decode and re-encode pixels, so input EXIF/text/profile metadata is not copied.
pub fn normalize_image(bytes: &[u8], mime: &str) -> Result<(Vec<u8>, ImageAsset), &'static str> {
    let _guard = IMAGE_DECODE
        .lock()
        .map_err(|_| "image_decoder_unavailable")?;
    let decoded = decode_image(bytes, mime)?;
    let (width, height) = decoded.dimensions();
    let format = if mime == "image/png" {
        ImageFormat::Png
    } else {
        ImageFormat::Jpeg
    };
    let pixels = if format == ImageFormat::Png {
        DynamicImage::ImageRgba8(decoded.into_rgba8())
    } else {
        DynamicImage::ImageRgb8(decoded.into_rgb8())
    };
    let mut normalized = Vec::new();
    pixels
        .write_to(&mut Cursor::new(&mut normalized), format)
        .map_err(|_| "image_encode")?;
    if normalized.len() > MAX_IMAGE_BYTES {
        return Err("image_size");
    }
    let asset = ImageAsset {
        sha256: signed::digest(&normalized),
        mime: mime.into(),
        bytes: normalized.len() as u32,
        width,
        height,
    };
    Ok((normalized, asset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn asset() -> ImageAsset {
        ImageAsset {
            sha256: "a".repeat(64),
            mime: "image/png".into(),
            bytes: 128,
            width: 100,
            height: 100,
        }
    }
    fn campaign(id: &str, start: u64, end: u64) -> Campaign {
        Campaign {
            campaign_id: id.into(),
            merchant_id: "merchant-one".into(),
            merchant_name: "本地合成商家".into(),
            title: "合成标题".into(),
            description: "合成内容，无账号资料。".into(),
            image: asset(),
            button_text: "了解详情".into(),
            target_url: "https://merchant.example.com/offer?campaign=public-one".into(),
            starts_at: start,
            ends_at: end,
            enabled: true,
        }
    }
    fn manifest(revision: u64) -> AdManifest {
        AdManifest {
            schema_version: 1,
            channel: AdChannel::Production,
            content_revision: revision,
            key_id: "ad-key-test".into(),
            issued_at: 1000,
            expires_at: 200_000,
            release_id: format!("release-{revision}"),
            slots: AdSlot::ALL
                .into_iter()
                .map(|slot| SlotContent {
                    slot,
                    enabled: true,
                    campaigns: vec![],
                })
                .collect(),
        }
    }
    fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[0x22; 32])
    }
    fn wire(manifest: &AdManifest) -> Vec<u8> {
        sign_raw(&serde_json::to_string(manifest).unwrap())
    }
    fn sign_raw(payload: &str) -> Vec<u8> {
        serde_json::to_vec(&signed::Envelope {
            payload: payload.into(),
            signature: signing_key().sign(payload.as_bytes()).to_bytes().to_vec(),
        })
        .unwrap()
    }
    fn enabled() -> AdPreferences {
        AdPreferences {
            slots_enabled: [true; 3],
            request_purpose_acknowledged: true,
        }
    }

    #[test]
    fn three_unique_slots_two_campaigns_and_global_ids() {
        let mut value = manifest(1);
        value.slots[0].campaigns = vec![campaign("one", 1000, 2000), campaign("two", 2000, 3000)];
        assert!(validate_manifest(&value, AdChannel::Production, "ad-key-test", 1000).is_ok());
        value.slots[1].campaigns = vec![campaign("one", 1000, 2000)];
        assert_eq!(
            validate_manifest(&value, AdChannel::Production, "ad-key-test", 1000),
            Err("campaign_id")
        );
        value.slots[1].campaigns.clear();
        value.slots[0].campaigns.push(campaign("three", 3000, 4000));
        assert_eq!(
            validate_manifest(&value, AdChannel::Production, "ad-key-test", 1000),
            Err("campaign_count")
        );
        value.slots[0].campaigns.clear();
        value.slots[2].slot = AdSlot::AccountService;
        assert_eq!(
            validate_manifest(&value, AdChannel::Production, "ad-key-test", 1000),
            Err("manifest_slots")
        );
        value.slots.pop();
        assert_eq!(
            validate_manifest(&value, AdChannel::Production, "ad-key-test", 1000),
            Err("manifest_slots")
        );
    }

    #[test]
    fn overlapping_disabled_campaign_still_requires_resolution() {
        let mut value = manifest(1);
        value.slots[0].campaigns = vec![campaign("one", 1000, 2001), campaign("two", 2000, 3000)];
        value.slots[0].campaigns[1].enabled = false;
        assert_eq!(
            validate_manifest(&value, AdChannel::Production, "ad-key-test", 1000),
            Err("campaign_overlap")
        );
    }

    #[test]
    fn manifest_scope_validity_and_ninety_day_limit() {
        let mut value = manifest(1);
        assert_eq!(
            validate_manifest(&value, AdChannel::Preview, "ad-key-test", 1000),
            Err("manifest_scope")
        );
        assert_eq!(
            validate_manifest(&value, AdChannel::Production, "another-key", 1000),
            Err("manifest_scope")
        );
        value.expires_at = value.issued_at + MAX_MANIFEST_LIFETIME + 1;
        assert_eq!(
            validate_manifest(&value, AdChannel::Production, "ad-key-test", 1000),
            Err("manifest_time")
        );
        value.expires_at = value.issued_at;
        assert_eq!(
            validate_manifest(&value, AdChannel::Production, "ad-key-test", 1000),
            Err("manifest_time")
        );
        value = manifest(MAX_SAFE_INTEGER + 1);
        assert_eq!(
            validate_manifest(&value, AdChannel::Production, "ad-key-test", 1000),
            Err("manifest_revision")
        );
    }

    #[test]
    fn nested_unknown_and_duplicate_fields_are_rejected_after_signature() {
        let mut value = manifest(1);
        value.slots[0].campaigns.push(campaign("one", 1000, 2000));
        let payload = serde_json::to_string(&value).unwrap();
        let duplicate = payload.replace("\"width\":100", "\"width\":100,\"width\":100");
        let unknown = payload.replace("\"width\":100", "\"width\":100,\"remote_command\":\"no\"");
        for payload in [duplicate, unknown] {
            assert!(
                verify_manifest(
                    &sign_raw(&payload),
                    &signing_key().verifying_key(),
                    AdChannel::Production,
                    "ad-key-test",
                    1000,
                    true
                )
                .is_err()
            );
        }
    }

    #[test]
    fn authentication_failures_never_refresh_cache_or_highest_revision() {
        let key = signing_key().verifying_key();
        let mut cache = AdCache::default();
        cache
            .accept(
                &wire(&manifest(2)),
                &key,
                AdChannel::Production,
                "ad-key-test",
                1000,
                true,
            )
            .unwrap();
        assert_eq!(
            cache.accept(
                &wire(&manifest(1)),
                &key,
                AdChannel::Production,
                "ad-key-test",
                1100,
                true
            ),
            Err("old_revision")
        );
        let mut conflicting = manifest(2);
        conflicting.release_id = "different".into();
        assert_eq!(
            cache.accept(
                &wire(&conflicting),
                &key,
                AdChannel::Production,
                "ad-key-test",
                1100,
                true
            ),
            Err("revision_conflict")
        );
        let mut bad: signed::Envelope = serde_json::from_slice(&wire(&manifest(3))).unwrap();
        bad.payload.push(' ');
        assert_eq!(
            cache.accept(
                &serde_json::to_vec(&bad).unwrap(),
                &key,
                AdChannel::Production,
                "ad-key-test",
                1100,
                true
            ),
            Err("signature")
        );
        assert_eq!(cache.highest_revision, 2);
        assert_eq!(cache.last_success, Some(1000));
        assert_eq!(
            cache.accept(
                &wire(&manifest(3)),
                &key,
                AdChannel::Production,
                "ad-key-test",
                1100,
                false
            ),
            Err("clock_untrusted")
        );
    }

    #[test]
    fn utc_boundaries_current_next_pause_and_slot_preferences() {
        let mut value = manifest(1);
        value.slots[0].campaigns = vec![campaign("one", 1000, 2000), campaign("two", 2000, 3000)];
        let mut cache = AdCache::default();
        cache
            .accept(
                &wire(&value),
                &signing_key().verifying_key(),
                AdChannel::Production,
                "ad-key-test",
                1000,
                true,
            )
            .unwrap();
        assert_eq!(
            cache
                .current_campaign(AdSlot::ResidentialBroadband, 1999, &enabled(), true)
                .unwrap()
                .campaign_id,
            "one"
        );
        assert_eq!(
            cache
                .next_campaign(AdSlot::ResidentialBroadband, 1999)
                .unwrap()
                .campaign_id,
            "two"
        );
        assert_eq!(
            cache
                .current_campaign(AdSlot::ResidentialBroadband, 2000, &enabled(), true)
                .unwrap()
                .campaign_id,
            "two"
        );
        assert!(
            cache
                .current_campaign(AdSlot::ResidentialBroadband, 3000, &enabled(), true)
                .is_none()
        );
        assert!(
            cache
                .current_campaign(AdSlot::ResidentialBroadband, 1500, &enabled(), false)
                .is_none()
        );
        assert!(
            cache
                .current_campaign(
                    AdSlot::ResidentialBroadband,
                    1500,
                    &AdPreferences::default(),
                    true
                )
                .is_none()
        );
        value.slots[0].enabled = false;
        value.content_revision = 2;
        cache
            .accept(
                &wire(&value),
                &signing_key().verifying_key(),
                AdChannel::Production,
                "ad-key-test",
                1600,
                true,
            )
            .unwrap();
        assert!(
            cache
                .current_campaign(AdSlot::ResidentialBroadband, 1600, &enabled(), true)
                .is_none()
        );
    }

    #[test]
    fn earliest_offline_expiry_and_disk_restore_do_not_extend_success_time() {
        let mut value = manifest(1);
        value.slots[0].campaigns = vec![campaign("one", 1000, 190_000)];
        let key = signing_key().verifying_key();
        let mut cache = AdCache::default();
        cache
            .accept(
                &wire(&value),
                &key,
                AdChannel::Production,
                "ad-key-test",
                1000,
                true,
            )
            .unwrap();
        assert!(
            cache
                .current_campaign(AdSlot::ResidentialBroadband, 87_399, &enabled(), true)
                .is_some()
        );
        assert!(
            cache
                .current_campaign(AdSlot::ResidentialBroadband, 87_400, &enabled(), true)
                .is_none()
        );
        let mut restored: AdCache =
            serde_json::from_slice(&serde_json::to_vec(&cache).unwrap()).unwrap();
        assert!(restored.manifest().is_none());
        restored
            .restore_verified(&key, AdChannel::Production, "ad-key-test", 2000, true)
            .unwrap();
        assert_eq!(restored.last_success, Some(1000));
        assert!(
            restored
                .current_campaign(AdSlot::ResidentialBroadband, 87_400, &enabled(), true)
                .is_none()
        );
        value.content_revision = 2;
        value.expires_at = 3000;
        cache
            .accept(
                &wire(&value),
                &key,
                AdChannel::Production,
                "ad-key-test",
                2000,
                true,
            )
            .unwrap();
        assert!(
            cache
                .current_campaign(AdSlot::ResidentialBroadband, 3000, &enabled(), true)
                .is_none()
        );
    }

    #[test]
    fn future_cache_success_time_is_hidden_and_cannot_be_restored() {
        let key = signing_key().verifying_key();
        let mut value = manifest(1);
        value.slots[0].campaigns.push(campaign("one", 1000, 3000));
        let mut cache = AdCache::default();
        cache
            .accept(
                &wire(&value),
                &key,
                AdChannel::Production,
                "ad-key-test",
                2000,
                true,
            )
            .unwrap();
        assert!(
            cache
                .current_campaign(AdSlot::ResidentialBroadband, 1500, &enabled(), true)
                .is_none()
        );
        assert_eq!(
            cache.restore_verified(&key, AdChannel::Production, "ad-key-test", 1500, true),
            Err("cache_time")
        );
        assert!(cache.manifest().is_none());
    }

    #[test]
    fn all_disabled_has_zero_request_permits_and_single_disabled_has_no_asset() {
        let source = ContentSource::new("https://content.example.com/ads/").unwrap();
        let mut schedule = RefreshSchedule::default();
        let mut preferences = AdPreferences::default();
        assert!(!schedule.begin(1000, &preferences, true, true));
        assert!(
            source
                .manifest_request(&preferences, AdChannel::Production)
                .is_none()
        );
        assert!(
            source
                .asset_request(
                    AdSlot::AccountService,
                    &asset(),
                    &preferences,
                    AdChannel::Production
                )
                .unwrap()
                .is_none()
        );
        preferences.request_purpose_acknowledged = true;
        preferences.slots_enabled[0] = true;
        let current = source
            .manifest_request(&preferences, AdChannel::Production)
            .unwrap();
        assert_eq!(
            current.as_str(),
            "https://content.example.com/ads/production/current.json"
        );
        assert!(source.permits_response_url(&current, &current));
        assert!(!source.permits_response_url(
            &current,
            &Url::parse("https://elsewhere.example.com/ads/production/current.json").unwrap()
        ));
        assert!(
            source
                .asset_request(
                    AdSlot::AccountService,
                    &asset(),
                    &preferences,
                    AdChannel::Production
                )
                .unwrap()
                .is_none()
        );
        let image = source
            .asset_request(
                AdSlot::ResidentialBroadband,
                &asset(),
                &preferences,
                AdChannel::Preview,
            )
            .unwrap()
            .unwrap();
        assert!(image.path().contains("/preview/assets/"));
        assert!(source.permits_response_url(&image, &image));
        let arbitrary = Url::parse("https://content.example.com/ads/private.json").unwrap();
        assert!(!source.permits_response_url(&arbitrary, &arbitrary));
    }

    #[test]
    fn schedule_persists_jitter_remaining_time_backoff_and_merges_manual_requests() {
        let mut schedule = RefreshSchedule::default();
        assert!(schedule.begin(1000, &enabled(), true, false));
        assert!(!schedule.begin(1000, &enabled(), true, true));
        schedule.complete_success(1000, 60).unwrap();
        assert_eq!(schedule.next_check_at, Some(1960));
        let mut restored: RefreshSchedule =
            serde_json::from_slice(&serde_json::to_vec(&schedule).unwrap()).unwrap();
        assert!(!restored.begin(1959, &enabled(), true, false));
        assert!(restored.begin(1960, &enabled(), true, false));
        restored.complete_success(1960, 0).unwrap();
        assert!(restored.begin(1961, &enabled(), true, true));
        restored.complete_success(1961, 0).unwrap();
        assert!(!restored.begin(1990, &enabled(), true, true));
        assert!(restored.begin(1991, &enabled(), true, true));
        for delay in [60, 300, 900, 1800, 3600, 3600] {
            restored.complete_failure(2000).unwrap();
            assert_eq!(restored.next_check_at, Some(2000 + delay));
        }
        assert_eq!(restored.complete_success(2000, 61), Err("refresh_jitter"));
    }

    #[test]
    fn merchant_urls_reject_private_forms_and_normalize_idn() {
        for raw in [
            "http://example.com",
            "https://127.0.0.1",
            "https://[::1]",
            "https://localhost",
            "https://machine",
            "https://machine.local",
            "https://machine.internal",
            "https://user:pass@example.com",
            "https://@example.com",
            "https://example.com:444",
            "https://example.com\n",
            "https://example.com.evil.invalid",
            "https://2130706433",
            "https://0x7f000001",
        ] {
            assert!(normalize_merchant_url(raw).is_err(), "accepted {raw}");
        }
        let (url, host) =
            normalize_merchant_url("https://BÜCHER.de.:443/path?campaign=public").unwrap();
        assert_eq!(host, "xn--bcher-kva.de");
        assert_eq!(url, "https://xn--bcher-kva.de/path?campaign=public");
    }

    #[test]
    fn changing_merchant_or_normalized_host_requires_new_consent() {
        let mut value = campaign("one", 1000, 2000);
        let mut consents = vec![];
        assert!(click_requires_confirmation(&consents, AdSlot::AccountService, &value).unwrap());
        record_click_consent(&mut consents, AdSlot::AccountService, &value).unwrap();
        assert!(!click_requires_confirmation(&consents, AdSlot::AccountService, &value).unwrap());
        value.target_url = "https://merchant.example.com/another-path".into();
        assert!(!click_requires_confirmation(&consents, AdSlot::AccountService, &value).unwrap());
        value.merchant_id = "other-merchant".into();
        assert!(click_requires_confirmation(&consents, AdSlot::AccountService, &value).unwrap());
        record_click_consent(&mut consents, AdSlot::AccountService, &value).unwrap();
        value.target_url = "https://new.example.com".into();
        assert!(click_requires_confirmation(&consents, AdSlot::AccountService, &value).unwrap());
        assert_eq!(consents.len(), 1);
    }

    fn tiny_image(format: ImageFormat) -> Vec<u8> {
        let image =
            DynamicImage::ImageRgb8(image::RgbImage::from_pixel(2, 3, image::Rgb([10, 20, 30])));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), format)
            .unwrap();
        bytes
    }

    #[test]
    fn png_jpeg_normalization_and_hash_mime_dimensions_are_enforced() {
        for (format, mime) in [
            (ImageFormat::Png, "image/png"),
            (ImageFormat::Jpeg, "image/jpeg"),
        ] {
            let bytes = tiny_image(format);
            let (normalized, mut asset) = normalize_image(&bytes, mime).unwrap();
            assert_eq!((asset.width, asset.height), (2, 3));
            validate_image(&normalized, &asset).unwrap();
            asset.sha256 = "0".repeat(64);
            assert_eq!(validate_image(&normalized, &asset), Err("image_hash"));
            asset.sha256 = signed::digest(&normalized);
            asset.width = 3;
            assert_eq!(validate_image(&normalized, &asset), Err("image_dimensions"));
            asset.width = 2;
            asset.mime = if mime == "image/png" {
                "image/jpeg".into()
            } else {
                "image/png".into()
            };
            assert_eq!(validate_image(&normalized, &asset), Err("image_mime"));
        }
    }

    #[test]
    fn animated_malformed_oversize_and_dimension_bombs_are_rejected() {
        let png = tiny_image(ImageFormat::Png);
        let mut animated = png.clone();
        animated.splice(33..33, [0, 0, 0, 0, b'a', b'c', b'T', b'L', 0, 0, 0, 0]);
        assert_eq!(
            normalize_image(&animated, "image/png").unwrap_err(),
            "image_animation"
        );
        assert!(normalize_image(&png[..20], "image/png").is_err());
        assert_eq!(
            normalize_image(&vec![0; MAX_IMAGE_BYTES + 1], "image/png").unwrap_err(),
            "image_size"
        );
        assert_eq!(
            normalize_image(b"GIF89a", "image/gif").unwrap_err(),
            "image_mime"
        );
        let mut asset = asset();
        asset.width = u32::MAX;
        asset.height = u32::MAX;
        assert_eq!(validate_asset(&asset), Err("image_dimensions"));
        asset.width = 2000;
        asset.height = 1001;
        assert_eq!(validate_asset(&asset), Err("image_dimensions"));
        let oversized_pixels = DynamicImage::ImageRgb8(image::RgbImage::new(2000, 1001));
        let mut oversized_png = Vec::new();
        oversized_pixels
            .write_to(&mut Cursor::new(&mut oversized_png), ImageFormat::Png)
            .unwrap();
        assert!(oversized_png.len() < MAX_IMAGE_BYTES);
        assert_eq!(
            normalize_image(&oversized_png, "image/png").unwrap_err(),
            "image_dimensions"
        );
        assert!(ContentSource::new("https://content.example.com/ads/?token=secret").is_err());
    }
}
