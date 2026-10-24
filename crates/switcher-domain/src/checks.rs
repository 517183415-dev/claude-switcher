use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CheckId {
    ProxyProcess,
    TunRoute,
    EgressRegion,
    IpContinuity,
    Timezone,
    Locale,
    Ipv6Policy,
    DnsRoute,
    StunPath,
    Guardian,
    ServiceRoutes,
    ClientLanguage,
    AsnInfo,
    ServiceTransport,
}

impl CheckId {
    pub const ALL: [Self; 14] = [
        Self::ProxyProcess,
        Self::TunRoute,
        Self::EgressRegion,
        Self::IpContinuity,
        Self::Timezone,
        Self::Locale,
        Self::Ipv6Policy,
        Self::DnsRoute,
        Self::StunPath,
        Self::Guardian,
        Self::ServiceRoutes,
        Self::ClientLanguage,
        Self::AsnInfo,
        Self::ServiceTransport,
    ];
    pub fn required(self) -> bool {
        !matches!(self, Self::IpContinuity | Self::AsnInfo)
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::ProxyProcess => "代理客户端",
            Self::TunRoute => "隧道路由",
            Self::EgressRegion => "出口地区",
            Self::IpContinuity => "出口地址变化",
            Self::Timezone => "时区",
            Self::Locale => "地区格式",
            Self::Ipv6Policy => "IPv6 策略",
            Self::DnsRoute => "DNS 路径",
            Self::StunPath => "STUN 路径",
            Self::Guardian => "后台守护",
            Self::ServiceRoutes => "服务路由",
            Self::ClientLanguage => "客户端语言",
            Self::AsnInfo => "网络运营商信息",
            Self::ServiceTransport => "服务传输与访问结果",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Warning,
    Fail,
    Unknown,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CheckResult {
    pub id: CheckId,
    pub status: CheckStatus,
    pub reason: String,
    pub observed_at: u64,
    pub duration_ms: u64,
    pub valid_until: u64,
    pub network_generation: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CheckSummary {
    pub passed: u32,
    pub advisory: u32,
    pub blocking: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct LaunchDecision {
    pub allowed: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct CheckReport {
    pub schema_version: u32,
    pub observed_at: u64,
    pub completed_at: u64,
    pub duration_ms: u64,
    pub network_generation: String,
    pub checks: Vec<CheckResult>,
    pub summary: CheckSummary,
    pub launch: LaunchDecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum TransportState {
    Reached,
    DnsFailure,
    TlsFailure,
    ConnectFailure,
    Timeout,
    ProxyAuthentication,
    InvalidResponse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum AccessState {
    Available,
    AuthenticationRequired,
    Restricted,
    Redirected,
    ServerError,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct TransportProbe {
    pub endpoint: String,
    pub transport: TransportState,
    pub access: AccessState,
    pub http_status: Option<u16>,
    pub tls_verified: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct ProxyDescription {
    pub scheme: String,
    pub host_scope: String,
    pub dns: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticReport {
    pub schema_version: u32,
    pub proxy: ProxyDescription,
    pub probes: Vec<TransportProbe>,
    pub transport_reached: bool,
    pub launch_allowed: Option<bool>,
    pub system_modified: bool,
}
