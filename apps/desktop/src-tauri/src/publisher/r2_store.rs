//! R2 storage adapter. Configuration and secrets must come from a trusted Personal source.
use super::protocol::{self, Object, ObjectStore, StoreError};
use aws_sdk_s3::{
    Client,
    config::{
        BehaviorVersion, Credentials, Region, RequestChecksumCalculation, retry::RetryConfig,
        timeout::TimeoutConfig,
    },
    primitives::ByteStream,
};
use std::{io::Read, time::Duration};
use switcher_core::{ads, signed};
use switcher_domain::ads::AdChannel;

// Pin the SDK behavior before automatic environment proxy support.
#[allow(deprecated)]
fn no_proxy_behavior() -> BehaviorVersion {
    BehaviorVersion::v2025_01_17()
}

type StoreResult<T> = std::result::Result<T, StoreError>;

/// Explicit Personal operator configuration, validated before use.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct R2Config {
    pub account_id: String,
    pub bucket: String,
    pub public_origin: String,
    pub channel: AdChannel,
}

pub struct R2Store {
    channel: AdChannel,
    bucket: String,
    origin: reqwest::Url,
    target: String,
    lock_target: String,
    client: Client,
    public_client: reqwest::blocking::Client,
    runtime: tokio::runtime::Runtime,
}

fn valid_bucket(bucket: &str) -> bool {
    (3..=63).contains(&bucket.len())
        && bucket
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && bucket
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && bucket
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
}

impl R2Config {
    pub(crate) fn lock_identity(&self) -> StoreResult<String> {
        self.validate()?;
        let bytes = serde_json::to_vec(&(
            self.account_id.to_ascii_lowercase(),
            &self.bucket,
            self.channel,
        ))
        .map_err(|_| StoreError::Unavailable)?;
        Ok(format!("r2-lock:{}", signed::digest(&bytes)))
    }

    pub(crate) fn identity(&self) -> StoreResult<String> {
        let origin = self.validate()?;
        let identity = serde_json::to_vec(&(
            "r2-v1",
            self.account_id.to_ascii_lowercase(),
            &self.bucket,
            origin
                .host_str()
                .ok_or(StoreError::Permission)?
                .trim_end_matches('.'),
            self.channel,
        ))
        .map_err(|_| StoreError::Unavailable)?;
        Ok(format!("r2:{}", signed::digest(&identity)))
    }
    pub(crate) fn validate(&self) -> StoreResult<reqwest::Url> {
        if self.account_id.len() != 32
            || !self.account_id.bytes().all(|c| c.is_ascii_hexdigit())
            || !valid_bucket(&self.bucket)
        {
            return Err(StoreError::Permission);
        }
        let origin =
            reqwest::Url::parse(&self.public_origin).map_err(|_| StoreError::Permission)?;
        let host = origin.host_str().ok_or(StoreError::Permission)?;
        let canonical_host = host.trim_end_matches('.');
        if origin.scheme() != "https"
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.query().is_some()
            || origin.fragment().is_some()
            || origin.path() != "/"
            || origin.port().is_some()
            || origin.domain().is_none()
            || ["r2.dev", "r2.cloudflarestorage.com"].iter().any(|domain| {
                canonical_host == *domain
                    || canonical_host
                        .strip_suffix(domain)
                        .is_some_and(|p| p.ends_with('.'))
            })
        {
            return Err(StoreError::Permission);
        }
        Ok(origin)
    }
}

impl R2Store {
    /// The credential strings are held by the SDK and never exposed through errors or logs.
    pub fn open(config: R2Config, access_key: &str, secret_key: &str) -> StoreResult<Self> {
        let origin = config.validate()?;
        if access_key.is_empty() || secret_key.is_empty() {
            return Err(StoreError::Permission);
        }
        let target = config.identity()?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| StoreError::Unavailable)?;
        let credentials = Credentials::new(access_key, secret_key, None, None, "r2");
        let sdk_config = aws_sdk_s3::config::Builder::new()
            .behavior_version(no_proxy_behavior())
            .region(Region::new("auto"))
            .endpoint_url(format!(
                "https://{}.r2.cloudflarestorage.com",
                config.account_id
            ))
            .force_path_style(true)
            .credentials_provider(credentials)
            .retry_config(RetryConfig::standard().with_max_attempts(1))
            .timeout_config(
                TimeoutConfig::builder()
                    .operation_timeout(Duration::from_secs(30))
                    .operation_attempt_timeout(Duration::from_secs(30))
                    .build(),
            )
            .request_checksum_calculation(RequestChecksumCalculation::WhenRequired)
            .build();
        let public_client = reqwest::blocking::Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .referer(false)
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .pool_max_idle_per_host(0)
            .build()
            .map_err(|_| StoreError::Unavailable)?;
        Ok(Self {
            channel: config.channel,
            lock_target: config.lock_identity()?,
            bucket: config.bucket,
            origin,
            target,
            client: Client::from_conf(sdk_config),
            public_client,
            runtime,
        })
    }

    fn source_read(&self, key: &str, limit: usize) -> StoreResult<Option<Object>> {
        self.source_read_with_timeout(key, limit, Duration::from_secs(30))
    }

    fn source_read_with_timeout(
        &self,
        key: &str,
        limit: usize,
        timeout: Duration,
    ) -> StoreResult<Option<Object>> {
        protocol::validate_key(self.channel, key)?;
        self.raw_read(key, limit, timeout)
    }

    fn raw_read(&self, key: &str, limit: usize, timeout: Duration) -> StoreResult<Option<Object>> {
        if limit > ads::MAX_IMAGE_BYTES {
            return Err(StoreError::Permission);
        }
        self.runtime.block_on(async {
            tokio::time::timeout(timeout, async {
                let mut response = match self
                    .client
                    .get_object()
                    .bucket(&self.bucket)
                    .key(key)
                    .send()
                    .await
                {
                    Ok(value) => value,
                    Err(err) => {
                        return match err.raw_response().map(|r| r.status().as_u16()) {
                            Some(404) => Ok(None),
                            Some(401 | 403) => Err(StoreError::Permission),
                            _ => Err(StoreError::Unavailable),
                        };
                    }
                };
                let etag = response.e_tag().unwrap_or("").to_owned();
                if etag.is_empty()
                    || etag.len() > 1024
                    || etag.chars().any(char::is_control)
                    || response
                        .content_length()
                        .is_some_and(|n| n < 0 || n as u64 > limit as u64)
                {
                    return Err(StoreError::Unavailable);
                }
                let mut bytes = Vec::new();
                while let Some(chunk) = response.body.next().await {
                    let chunk = chunk.map_err(|_| StoreError::Unavailable)?;
                    if chunk.len() > limit.saturating_sub(bytes.len()) {
                        return Err(StoreError::Unavailable);
                    }
                    bytes.extend_from_slice(&chunk);
                }
                Ok(Some(Object { bytes, etag }))
            })
            .await
            .map_err(|_| StoreError::Unavailable)?
        })
    }
    fn put(&self, key: &str, bytes: &[u8], expected: Option<&str>) -> StoreResult<()> {
        protocol::validate_key(self.channel, key)?;
        self.raw_put(key, bytes, expected)
    }

    fn raw_put(&self, key: &str, bytes: &[u8], expected: Option<&str>) -> StoreResult<()> {
        if bytes.len() > ads::MAX_IMAGE_BYTES {
            return Err(StoreError::Permission);
        }
        let request = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(bytes.to_vec()));
        let request = match expected {
            Some(etag) => request.if_match(etag),
            None => request.if_none_match("*"),
        };
        self.runtime.block_on(async {
            match request.send().await {
                Ok(_) => Ok(()),
                Err(err) => match err.raw_response().map(|r| r.status().as_u16()) {
                    Some(412) => Err(StoreError::Conflict),
                    Some(401 | 403) => Err(StoreError::Permission),
                    _ => Err(StoreError::OutcomeUnknown),
                },
            }
        })
    }
}
impl ObjectStore for R2Store {
    fn lock_identity(&self) -> Option<String> {
        Some(self.lock_target.clone())
    }
    fn read_lock(&mut self) -> StoreResult<Option<Object>> {
        self.raw_read(
            &format!("{}/publisher-lock.json", protocol::prefix(self.channel)),
            super::remote_lock::WIRE_LIMIT,
            Duration::from_secs(30),
        )
    }
    fn commit_lock(&mut self, expected: Option<&str>, bytes: &[u8]) -> StoreResult<()> {
        if bytes.len() > super::remote_lock::WIRE_LIMIT {
            return Err(StoreError::Permission);
        }
        self.raw_put(
            &format!("{}/publisher-lock.json", protocol::prefix(self.channel)),
            bytes,
            expected,
        )
    }

    fn channel(&self) -> AdChannel {
        self.channel
    }
    fn target_identity(&self) -> Option<String> {
        Some(self.target.clone())
    }
    fn current(&mut self) -> StoreResult<Option<Object>> {
        self.source_read(
            &protocol::current_key(self.channel),
            ads::MAX_MANIFEST_BYTES,
        )
    }
    fn public_read(&mut self, key: &str, limit: usize) -> StoreResult<Option<Object>> {
        protocol::validate_key(self.channel, key)?;
        if limit > ads::MAX_IMAGE_BYTES {
            return Err(StoreError::Permission);
        }
        let url = self.origin.join(key).map_err(|_| StoreError::Permission)?;
        let mut response = self
            .public_client
            .get(url)
            .send()
            .map_err(|_| StoreError::Unavailable)?;
        match response.status().as_u16() {
            404 => return Ok(None),
            401 | 403 => return Err(StoreError::Permission),
            200 => (),
            _ => return Err(StoreError::Unavailable),
        }
        if response.content_length().is_some_and(|n| n > limit as u64) {
            return Err(StoreError::Unavailable);
        }
        let mut bytes = Vec::new();
        (&mut response)
            .take((limit as u64) + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| StoreError::Unavailable)?;
        if bytes.len() > limit {
            return Err(StoreError::Unavailable);
        }
        Ok(Some(Object {
            bytes,
            etag: String::new(),
        }))
    }
    fn put_immutable(&mut self, key: &str, bytes: &[u8]) -> StoreResult<()> {
        if key == protocol::current_key(self.channel) {
            return Err(StoreError::Permission);
        }
        match self.put(key, bytes, None) {
            Err(StoreError::Conflict) => match self.source_read(key, ads::MAX_IMAGE_BYTES)? {
                Some(object) if object.bytes == bytes => Ok(()),
                _ => Err(StoreError::Conflict),
            },
            result => result,
        }
    }
    fn commit_current(&mut self, expected_etag: Option<&str>, bytes: &[u8]) -> StoreResult<()> {
        if bytes.len() > ads::MAX_MANIFEST_BYTES
            || expected_etag
                .is_some_and(|e| e.is_empty() || e.len() > 1024 || e.chars().any(char::is_control))
        {
            return Err(StoreError::Permission);
        }
        self.put(&protocol::current_key(self.channel), bytes, expected_etag)
    }
}

#[cfg(test)]
pub(super) mod tests;
