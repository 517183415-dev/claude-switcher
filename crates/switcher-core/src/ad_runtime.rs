//! Anonymous, fixed-source advertisement reads and shared local clock state.
pub mod local;
use crate::{
    ads::{self, AdCache, ContentSource, RefreshSchedule},
    signed, storage,
    time::{self, ClockSample, ClockState},
};
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};
use switcher_domain::{
    AdSlot, Edition,
    ads::{AdChannel, AdManifest, AdPreferences, ImageAsset, MerchantConsent},
};
use url::Url;

type Result<T> = std::result::Result<T, &'static str>;
const STATE_LIMIT: usize = 2 * 1024 * 1024;

/// These fields remain private. Source and verification key enter production
/// only through a reviewed software release, never IPC, preferences or a URL.
struct ContentPins {
    source: ContentSource,
    key: VerifyingKey,
    key_id: String,
    channel: AdChannel,
    local: bool,
}
impl ContentPins {
    fn production() -> Result<Option<Self>> {
        Self::configured_release([
            option_env!("SWITCHER_AD_SOURCE"),
            option_env!("SWITCHER_AD_PUBLIC_KEY_HEX"),
            option_env!("SWITCHER_AD_KEY_ID"),
            option_env!("SWITCHER_AD_CHANNEL"),
        ])
    }

    fn configured_release(materials: [Option<&str>; 4]) -> Result<Option<Self>> {
        if materials.iter().all(Option::is_none) {
            return Ok(None);
        }
        Self::release_material(materials[0], materials[1], materials[2], materials[3])
            .map(Some)
            .ok_or("ad_release_materials")
    }

    /// Values are embedded by the release build, never read from process state.
    fn release_material(
        source: Option<&str>,
        public_key_hex: Option<&str>,
        key_id: Option<&str>,
        channel: Option<&str>,
    ) -> Option<Self> {
        let (source, public_key_hex, key_id, channel) =
            (source?, public_key_hex?, key_id?, channel?);
        if key_id.is_empty()
            || key_id.len() > 128
            || !key_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        {
            return None;
        }
        let channel = match channel {
            "production" => AdChannel::Production,
            "preview" => AdChannel::Preview,
            _ => return None,
        };
        let bytes = public_key_hex.as_bytes();
        if bytes.len() != 64 {
            return None;
        }
        let mut key_bytes = [0u8; 32];
        for (out, pair) in key_bytes.iter_mut().zip(bytes.chunks_exact(2)) {
            let digit = |b: u8| match b {
                b'0'..=b'9' => Some(b - b'0'),
                b'a'..=b'f' => Some(b - b'a' + 10),
                _ => None,
            };
            *out = digit(pair[0])? * 16 + digit(pair[1])?;
        }
        let key = VerifyingKey::from_bytes(&key_bytes).ok()?;
        if key.is_weak() {
            return None;
        }
        Some(Self {
            source: ContentSource::new(source).ok()?,
            key,
            key_id: key_id.to_owned(),
            channel,
            local: false,
        })
    }
}

/// Validate the trust material embedded in an explicitly formal Public build.
pub fn validate_public_release_materials() -> Result<()> {
    validate_public_release_pins(ContentPins::production()?)
}

fn validate_public_release_pins(pins: Option<ContentPins>) -> Result<()> {
    match pins {
        Some(pins) if pins.channel == AdChannel::Production => {
            let preferences = AdPreferences {
                slots_enabled: [true; 3],
                request_purpose_acknowledged: true,
            };
            let url = pins
                .source
                .manifest_request(&preferences, pins.channel)
                .ok_or("ad_release_materials")?;
            let host = url
                .host_str()
                .ok_or("ad_release_materials")?
                .trim_end_matches('.')
                .to_ascii_lowercase();
            if host == "r2.dev" || host.ends_with(".r2.dev") {
                return Err("ad_release_materials");
            }
            Ok(())
        }
        _ => Err("ad_release_materials"),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Confirmation {
    slot: AdSlot,
    merchant_id: String,
    normalized_host: String,
    campaign_id: String,
    release_id: String,
    requested_at: u64,
}
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Persistent {
    schema: u32,
    cache: AdCache,
    sequence: signed::Sequence,
    schedule: RefreshSchedule,
    consents: Vec<MerchantConsent>,
    confirmations: Vec<Confirmation>,
    #[serde(default)]
    local_wire_hash: Option<String>,
    #[serde(default)]
    local_attempt_hash: Option<String>,
}
impl Persistent {
    fn new() -> Self {
        Self {
            schema: 1,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct RefreshResult {
    pub refreshed: bool,
    pub reason: &'static str,
    pub next_check_at: Option<u64>,
}

/// Content and refresh health are read under the same lifecycle lock.
pub struct AdSnapshot {
    pub manifest: Option<AdManifest>,
    pub refresh_failed: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClickDecision {
    NeedsConfirmation {
        merchant_name: String,
        normalized_host: String,
    },
    Ready {
        url: String,
    },
}

pub struct AdRuntime {
    root: PathBuf,
    edition: Edition,
    pins: Option<ContentPins>,
    local_error: Option<&'static str>,
    sample_clock: fn() -> Result<ClockSample>,
}
struct Download {
    bytes: Vec<u8>,
    mime: Option<String>,
    final_url: Url,
}
trait Fetcher {
    fn get(&mut self, url: &Url, limit: usize) -> Result<Download>;
}
struct HttpsFetcher {
    client: reqwest::blocking::Client,
}
impl HttpsFetcher {
    fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .pool_max_idle_per_host(0)
            .build()
            .map_err(|_| "ad_transport")?;
        Ok(Self { client })
    }
}
impl Fetcher for HttpsFetcher {
    fn get(&mut self, url: &Url, limit: usize) -> Result<Download> {
        let response = self
            .client
            .get(url.clone())
            .header(
                reqwest::header::ACCEPT,
                "application/json,image/png,image/jpeg",
            )
            .send()
            .map_err(|_| "ad_network")?;
        if response.status() != reqwest::StatusCode::OK || response.url() != url {
            return Err("ad_http_status_or_redirect");
        }
        if response
            .content_length()
            .is_some_and(|length| length > limit as u64)
        {
            return Err("ad_download_size");
        }
        let mime = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let final_url = response.url().clone();
        let mut bytes = Vec::new();
        response
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "ad_download")?;
        if bytes.len() > limit {
            return Err("ad_download_size");
        }
        Ok(Download {
            bytes,
            mime,
            final_url,
        })
    }
}

fn enabled(preferences: &AdPreferences) -> bool {
    preferences.request_purpose_acknowledged
        && preferences.slots_enabled.iter().any(|enabled| *enabled)
}
fn utc(sample: &ClockSample) -> Result<u64> {
    sample.utc_secs.try_into().map_err(|_| "ad_clock")
}
fn jitter() -> Result<u8> {
    for _ in 0..16 {
        let mut byte = 0u8;
        // SAFETY: getentropy writes exactly the single-byte buffer and uses the OS CSPRNG.
        if unsafe { libc::getentropy((&mut byte as *mut u8).cast(), 1) } != 0 {
            return Err("ad_random");
        }
        if byte < 244 {
            return Ok(byte % 61);
        }
    }
    Err("ad_random")
}
fn write_changed(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    match storage::read_private_bytes(path, STATE_LIMIT) {
        Ok(previous) if previous == bytes => return Ok(()),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("ad_state_read"),
    }
    storage::atomic_write(path, bytes).map_err(|_| "ad_state_write")
}

impl AdRuntime {
    pub fn new(root: PathBuf, edition: Edition) -> Self {
        let (pins, local_error) = match Self::select_pins(&root, edition, ContentPins::production())
        {
            Ok(pins) => (pins, None),
            Err(error) => (None, Some(error)),
        };
        Self {
            root,
            edition,
            pins,
            local_error,
            sample_clock: time::sample,
        }
    }

    /// Reports only whether a source was selected, not whether content is available or verified.
    pub fn content_source_configured(&self) -> Result<bool> {
        match self.local_error {
            Some(error) => Err(error),
            None => Ok(self.pins.is_some()),
        }
    }

    fn select_pins(
        root: &Path,
        edition: Edition,
        release: Result<Option<ContentPins>>,
    ) -> Result<Option<ContentPins>> {
        if edition == Edition::Personal {
            if let Some(local) = local::pins(root)? {
                return Ok(Some(local));
            }
            return release;
        }
        let release = release?;
        if edition == Edition::Public && release.is_some() {
            return Ok(release);
        }
        Ok(local::pins(root)?.or(release))
    }

    /// Supplies the clock at the Rust composition boundary. Production uses `time::sample`.
    pub fn with_clock(mut self, sample_clock: fn() -> Result<ClockSample>) -> Self {
        self.sample_clock = sample_clock;
        self
    }

    fn v2(&self) -> PathBuf {
        self.root.join("v2")
    }
    fn directory(&self) -> PathBuf {
        self.v2().join(if self.edition == Edition::Personal {
            "personal"
        } else {
            "public"
        })
    }
    fn ads_directory(&self) -> PathBuf {
        self.directory().join("ads")
    }
    fn state_path(&self) -> Result<PathBuf> {
        let Some(pins) = &self.pins else {
            return Ok(self.ads_directory().join("state.json"));
        };
        let preferences = AdPreferences {
            slots_enabled: [true; 3],
            request_purpose_acknowledged: true,
        };
        let url = pins
            .source
            .manifest_request(&preferences, pins.channel)
            .ok_or("ad_source_identity")?;
        let identity = serde_json::to_vec(&(
            url.as_str(),
            pins.key.to_bytes(),
            &pins.key_id,
            pins.channel,
            pins.local,
        ))
        .map_err(|_| "ad_source_identity")?;
        Ok(self
            .ads_directory()
            .join(format!("state-{}.json", signed::digest(&identity))))
    }

    fn legacy_local_state(&self) -> Result<Option<Persistent>> {
        let Some(pins) = self.pins.as_ref().filter(|pins| pins.local) else {
            return Ok(None);
        };
        let bytes = match storage::read_private_bytes(
            &self.ads_directory().join("state.json"),
            STATE_LIMIT,
        ) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err("ad_legacy_state_read"),
        };
        let mut state: Persistent = signed::strict_json(&bytes, STATE_LIMIT)?;
        if state.schema != 1 {
            return Err("ad_legacy_state_shape");
        }
        if state.cache.highest_revision > state.sequence.highest {
            return Err("ad_legacy_state_unverified");
        }
        if state.sequence.highest != 0 {
            let hash = state
                .sequence
                .payload_hash
                .as_deref()
                .ok_or("ad_legacy_state_unverified")?;
            if hash.len() != 64
                || !hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err("ad_legacy_state_unverified");
            }
            // The old watermark may outlive its signed cache, for example when
            // an asset download failed or the local signing key changed. Keep it
            // only as a conservative floor; never treat the old cache as trusted
            // under the new key or lower the floor to make a refresh succeed.
            let cache_matches = state.cache.cached_wire.as_deref().is_some_and(|wire| {
                signed::verify::<AdManifest>(wire, &pins.key, ads::MAX_MANIFEST_BYTES).is_ok_and(
                    |(manifest, digest)| {
                        manifest.channel == pins.channel
                            && manifest.key_id == pins.key_id
                            && manifest.content_revision == state.sequence.highest
                            && digest == hash
                            && state.cache.highest_revision == state.sequence.highest
                            && state.cache.highest_payload_hash.as_deref() == Some(hash)
                    },
                )
            });
            if !cache_matches {
                state.cache = AdCache::default();
                state.schedule = RefreshSchedule::default();
                state.confirmations.clear();
                state.local_wire_hash = None;
                state.local_attempt_hash = None;
            }
        } else if state.sequence.payload_hash.is_some()
            || state.cache.highest_revision != 0
            || state.cache.highest_payload_hash.is_some()
            || state.cache.cached_wire.is_some()
        {
            return Err("ad_legacy_state_unverified");
        }
        Ok(Some(state))
    }
    fn lock(&self) -> Result<storage::LifecycleLock> {
        if let Some(error) = self.local_error {
            return Err(error);
        }
        storage::prepare_data_root(&self.root).map_err(|_| "ad_directory")?;
        storage::create_private_directory(&self.v2()).map_err(|_| "ad_directory")?;
        let lock = storage::LifecycleLock::try_acquire(&self.v2()).map_err(|_| "lifecycle_busy")?;
        for directory in [
            self.directory(),
            self.ads_directory(),
            self.ads_directory().join("assets"),
        ] {
            storage::create_private_directory(&directory).map_err(|_| "ad_directory")?;
        }
        Ok(lock)
    }
    fn load(&self, sample: &ClockSample) -> Result<(Persistent, ClockState)> {
        let path = self.state_path()?;
        let mut state: Persistent = match storage::read_private_bytes(&path, STATE_LIMIT) {
            Ok(bytes) => signed::strict_json(&bytes, STATE_LIMIT)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.legacy_local_state()?.unwrap_or_else(Persistent::new)
            }
            Err(_) => return Err("ad_state_read"),
        };
        if state.schema != 1
            || state.consents.len() > 3
            || state.confirmations.len() > 3
            || state.cache.highest_revision > state.sequence.highest
            || (state.cache.highest_revision == state.sequence.highest
                && state.cache.highest_payload_hash != state.sequence.payload_hash)
            || state
                .cache
                .cached_wire
                .as_ref()
                .is_some_and(|wire| wire.len() > ads::MAX_MANIFEST_BYTES)
        {
            return Err("ad_state_shape");
        }
        let mut clock: ClockState = match storage::read_json(&self.directory().join("clock.json")) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => ClockState::default(),
            Err(_) => return Err("ad_clock_read"),
        };
        clock.observe_passive(sample.clone());
        if let Some(pins) = &self.pins {
            // Expiry or an untrusted clock hides content without deleting either watermark.
            let _ = state.cache.restore_verified(
                &pins.key,
                pins.channel,
                &pins.key_id,
                utc(sample)?,
                clock.trusted(),
            );
        }
        Ok((state, clock))
    }
    fn save(&self, state: &Persistent, clock: &ClockState) -> Result<()> {
        let bytes = serde_json::to_vec(state).map_err(|_| "ad_state_json")?;
        if bytes.len() > STATE_LIMIT {
            return Err("ad_state_size");
        }
        write_changed(&self.state_path()?, &bytes)?;
        write_changed(
            &self.directory().join("clock.json"),
            &serde_json::to_vec(clock).map_err(|_| "ad_clock_json")?,
        )
    }
    fn verified_image(&self, asset: &ImageAsset) -> Result<Vec<u8>> {
        ads::validate_asset(asset)?;
        let bytes = storage::read_private_bytes(
            &self.ads_directory().join("assets").join(&asset.sha256),
            ads::MAX_IMAGE_BYTES,
        )
        .map_err(|_| "ad_image_read")?;
        ads::validate_image(&bytes, asset)?;
        Ok(bytes)
    }
    fn visible(
        &self,
        state: &Persistent,
        clock: &ClockState,
        preferences: &AdPreferences,
        now: u64,
    ) -> Option<AdManifest> {
        if self.pins.is_none()
            || !clock.trusted()
            || state.cache.highest_revision != state.sequence.highest
        {
            return None;
        }
        if self.pins.as_ref().is_some_and(|pins| pins.local)
            && local::current_digest(&self.root).ok().flatten().as_ref()
                != state.local_wire_hash.as_ref()
        {
            return None;
        }
        let mut visible = state.cache.manifest()?.clone();
        for slot in &mut visible.slots {
            let current = state
                .cache
                .current_campaign(slot.slot, now, preferences, true)
                .filter(|campaign| self.verified_image(&campaign.image).is_ok())
                .cloned();
            slot.enabled = current.is_some();
            slot.campaigns = current.into_iter().collect();
        }
        if visible.slots.iter().all(|slot| slot.campaigns.is_empty()) {
            None
        } else {
            Some(visible)
        }
    }
    fn local_changed(&self, state: &Persistent) -> Result<bool> {
        if !self.pins.as_ref().is_some_and(|pins| pins.local) {
            return Ok(false);
        }
        Ok(local::current_digest(&self.root)?
            .is_some_and(|hash| state.local_attempt_hash.as_ref() != Some(&hash)))
    }
    pub fn snapshot(&self, preferences: &AdPreferences) -> Result<Option<AdManifest>> {
        Ok(self.snapshot_with_status(preferences)?.manifest)
    }
    pub fn snapshot_with_status(&self, preferences: &AdPreferences) -> Result<AdSnapshot> {
        let _lock = self.lock()?;
        let mut sample = (self.sample_clock)()?;
        if self.pins.as_ref().is_some_and(|pins| pins.local) && enabled(preferences) {
            self.refresh_locked(preferences, false, sample.clone(), None, None)?;
            sample = (self.sample_clock)()?;
        }
        let (state, clock) = self.load(&sample)?;
        self.save(&state, &clock)?;
        Ok(AdSnapshot {
            manifest: self.visible(&state, &clock, preferences, utc(&sample)?),
            refresh_failed: self.pins.is_some()
                && enabled(preferences)
                && state.schedule.failures > 0,
        })
    }
    pub fn refresh_if_due(
        &self,
        preferences: &AdPreferences,
        manual: bool,
    ) -> Result<RefreshResult> {
        let _lock = self.lock()?;
        let sample = (self.sample_clock)()?;
        self.refresh_locked(preferences, manual, sample, None, None)
    }
    fn refresh_locked(
        &self,
        preferences: &AdPreferences,
        manual: bool,
        sample: ClockSample,
        supplied: Option<&mut dyn Fetcher>,
        fixed_jitter: Option<u8>,
    ) -> Result<RefreshResult> {
        let (mut state, mut clock) = self.load(&sample)?;
        let now = utc(&sample)?;
        let local_empty = enabled(preferences)
            && self.pins.as_ref().is_some_and(|pins| pins.local)
            && local::current_digest(&self.root)?.is_none();
        let reason = if local_empty {
            Some("local_ad_not_published")
        } else if !enabled(preferences) {
            Some("ads_disabled")
        } else if self.pins.is_none() {
            Some("wait_ad_content_materials")
        } else if !clock.allows_verified_refresh() {
            Some("clock_recovery_required")
        } else if !self.local_changed(&state)?
            && !state.schedule.begin(now, preferences, true, manual)
        {
            Some("ad_refresh_not_due_or_merged")
        } else {
            None
        };
        if let Some(reason) = reason {
            self.save(&state, &clock)?;
            // Keep a failed local release visible as a diagnostic during backoff,
            // including when the background tick performed the first attempt.
            if reason == "ad_refresh_not_due_or_merged"
                && self.pins.as_ref().is_some_and(|pins| pins.local)
                && state.local_attempt_hash.is_some()
                && state.local_attempt_hash != state.local_wire_hash
            {
                return Err("local_ad_previous_content_verification_failed");
            }
            return Ok(RefreshResult {
                refreshed: false,
                reason,
                next_check_at: state.schedule.next_check_at,
            });
        }
        // Only an actual refresh advances the normal observation baseline.
        clock.observe(sample.clone());
        if self.pins.as_ref().is_some_and(|pins| pins.local) {
            state.local_attempt_hash = local::current_digest(&self.root)?;
        }
        self.save(&state, &clock)?;
        let pins = self.pins.as_ref().ok_or("wait_ad_content_materials")?;
        let supplied_clock = supplied.is_some();
        let started = std::time::Instant::now();
        let result = (|| -> Result<ClockSample> {
            let mut owned;
            let mut local_fetcher = local::LocalFetcher {
                root: local::directory(&self.root),
            };
            let fetcher: &mut dyn Fetcher = if let Some(fetcher) = supplied {
                fetcher
            } else if pins.local {
                &mut local_fetcher
            } else {
                owned = HttpsFetcher::new()?;
                &mut owned
            };
            let refresh_jitter = match fixed_jitter {
                Some(value) => value,
                None => jitter()?,
            };
            let request = pins
                .source
                .manifest_request(preferences, pins.channel)
                .ok_or("ads_disabled")?;
            let response = fetcher.get(&request, ads::MAX_MANIFEST_BYTES)?;
            if !pins
                .source
                .permits_response_url(&request, &response.final_url)
            {
                return Err("ad_redirect");
            }
            if response
                .mime
                .as_deref()
                .and_then(|value| value.split(';').next())
                .is_none_or(|mime| mime.trim() != "application/json")
            {
                return Err("ad_manifest_mime");
            }
            let (manifest, hash) = ads::verify_manifest(
                &response.bytes,
                &pins.key,
                pins.channel,
                &pins.key_id,
                now,
                true,
            )?;
            state.sequence.remember(manifest.content_revision, hash)?;
            // Preserve authenticated revision even if an asset subsequently fails.
            self.save(&state, &clock)?;
            for slot in &manifest.slots {
                if !slot.enabled || !preferences.slots_enabled[ads::slot_index(slot.slot)] {
                    continue;
                }
                for campaign in slot
                    .campaigns
                    .iter()
                    .filter(|campaign| campaign.enabled && campaign.ends_at > now)
                {
                    if self.verified_image(&campaign.image).is_ok() {
                        continue;
                    }
                    let request = pins
                        .source
                        .asset_request(slot.slot, &campaign.image, preferences, pins.channel)?
                        .ok_or("ads_disabled")?;
                    let response = fetcher.get(&request, ads::MAX_IMAGE_BYTES)?;
                    if !pins
                        .source
                        .permits_response_url(&request, &response.final_url)
                    {
                        return Err("ad_redirect");
                    }
                    if response.mime.as_deref() != Some(&campaign.image.mime) {
                        return Err("ad_image_mime");
                    }
                    ads::validate_image(&response.bytes, &campaign.image)?;
                    storage::atomic_write(
                        &self
                            .ads_directory()
                            .join("assets")
                            .join(&campaign.image.sha256),
                        &response.bytes,
                    )
                    .map_err(|_| "ad_image_write")?;
                }
            }
            let final_sample = if supplied_clock {
                sample.clone()
            } else {
                (self.sample_clock)()?
            };
            clock.observe(final_sample.clone());
            if !clock.allows_verified_refresh() {
                return Err("clock_recovery_required");
            }
            state.cache.accept(
                &response.bytes,
                &pins.key,
                pins.channel,
                &pins.key_id,
                utc(&final_sample)?,
                true,
            )?;
            if pins.local {
                state.local_wire_hash = Some(signed::digest(&response.bytes));
            }
            clock.record_verified_success(final_sample.clone())?;
            state
                .schedule
                .complete_success(utc(&final_sample)?, refresh_jitter)?;
            Ok(final_sample)
        })();
        let reason = match result {
            Ok(_) => "ad_refreshed",
            Err(reason) => {
                // Backoff starts when the attempt ends, not when a potentially
                // slow network request began. Synthetic fetchers supply their
                // own fixed sample; production samples the clock again.
                let finished = if supplied_clock {
                    Some(sample.clone())
                } else {
                    (self.sample_clock)().ok()
                };
                let failed_at = finished
                    .and_then(|finished| {
                        clock.observe(finished.clone());
                        utc(&finished).ok()
                    })
                    .unwrap_or_else(|| {
                        let elapsed = started.elapsed();
                        now.saturating_add(elapsed.as_secs())
                            .saturating_add(u64::from(elapsed.subsec_nanos() != 0))
                    });
                state.schedule.complete_failure(failed_at)?;
                self.save(&state, &clock)?;
                return Err(reason);
            }
        };
        self.save(&state, &clock)?;
        Ok(RefreshResult {
            refreshed: true,
            reason,
            next_check_at: state.schedule.next_check_at,
        })
    }
    pub fn click_target(
        &self,
        slot: AdSlot,
        confirmed: bool,
        preferences: &AdPreferences,
    ) -> Result<ClickDecision> {
        if self.pins.as_ref().is_some_and(|pins| pins.local) && enabled(preferences) {
            self.refresh_if_due(preferences, false)?;
        }
        let _lock = self.lock()?;
        let sample = (self.sample_clock)()?;
        self.click_locked(slot, confirmed, preferences, &sample)
    }
    fn click_locked(
        &self,
        slot: AdSlot,
        confirmed: bool,
        preferences: &AdPreferences,
        sample: &ClockSample,
    ) -> Result<ClickDecision> {
        let (mut state, mut clock) = self.load(sample)?;
        clock.observe(sample.clone());
        let now = utc(sample)?;
        // A rejected click must still persist a detected clock anomaly.
        self.save(&state, &clock)?;
        let visible = self
            .visible(&state, &clock, preferences, now)
            .ok_or("ad_not_current")?;
        let campaign = visible
            .slots
            .iter()
            .find(|content| content.slot == slot)
            .and_then(|content| content.campaigns.first())
            .ok_or("ad_not_current")?;
        let (url, host) = ads::normalize_merchant_url(&campaign.target_url)?;
        if ads::click_requires_confirmation(&state.consents, slot, campaign)? {
            let matches = state.confirmations.iter().any(|confirmation| {
                confirmation.slot == slot
                    && confirmation.merchant_id == campaign.merchant_id
                    && confirmation.normalized_host == host
                    && confirmation.campaign_id == campaign.campaign_id
                    && confirmation.release_id == visible.release_id
                    && confirmation.requested_at <= now
                    && now < confirmation.requested_at.saturating_add(300)
            });
            if !confirmed || !matches {
                state
                    .confirmations
                    .retain(|confirmation| confirmation.slot != slot);
                state.confirmations.push(Confirmation {
                    slot,
                    merchant_id: campaign.merchant_id.clone(),
                    normalized_host: host.clone(),
                    campaign_id: campaign.campaign_id.clone(),
                    release_id: visible.release_id,
                    requested_at: now,
                });
                self.save(&state, &clock)?;
                return Ok(ClickDecision::NeedsConfirmation {
                    merchant_name: campaign.merchant_name.clone(),
                    normalized_host: host,
                });
            }
            ads::record_click_consent(&mut state.consents, slot, campaign)?;
        }
        state
            .confirmations
            .retain(|confirmation| confirmation.slot != slot);
        self.save(&state, &clock)?;
        Ok(ClickDecision::Ready { url })
    }
    pub fn image_bytes(
        &self,
        slot: AdSlot,
        preferences: &AdPreferences,
    ) -> Result<Option<(ImageAsset, Vec<u8>)>> {
        if self.pins.as_ref().is_some_and(|pins| pins.local) && enabled(preferences) {
            self.refresh_if_due(preferences, false)?;
        }
        let _lock = self.lock()?;
        let sample = (self.sample_clock)()?;
        let (state, clock) = self.load(&sample)?;
        self.save(&state, &clock)?;
        let Some(visible) = self.visible(&state, &clock, preferences, utc(&sample)?) else {
            return Ok(None);
        };
        let Some(campaign) = visible
            .slots
            .iter()
            .find(|content| content.slot == slot)
            .and_then(|content| content.campaigns.first())
        else {
            return Ok(None);
        };
        Ok(Some((
            campaign.image.clone(),
            self.verified_image(&campaign.image)?,
        )))
    }
}

#[cfg(test)]
mod tests;
