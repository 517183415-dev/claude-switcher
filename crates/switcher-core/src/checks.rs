//! Legacy-compatible check engine. Only hashed egress history is persisted.
use crate::{config::CheckConfig, macos, network};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use switcher_domain::checks::*;
use tokio::sync::OnceCell;

pub const EVIDENCE_TTL_SECONDS: u64 = 30;
const TOTAL_CHECK_TIMEOUT: Duration = Duration::from_secs(30);
const SERVICE_HOSTS: [&str; 3] = ["claude.ai", "api.anthropic.com", "claudeusercontent.com"];
pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn evaluate_launch(report: &CheckReport, now: u64, generation: &str) -> LaunchDecision {
    let mut reasons = Vec::new();
    if report.schema_version != 1 {
        reasons.push("检查报告版本无效".into());
    }
    if report.observed_at > report.completed_at
        || report.completed_at > now
        || now.saturating_sub(report.completed_at) > EVIDENCE_TTL_SECONDS
    {
        reasons.push("检查报告已过期或时间无效，请重新检查".into());
    }
    if generation.trim().is_empty()
        || report.network_generation.trim().is_empty()
        || report.network_generation != generation
    {
        reasons.push("网络代次无法确认或已变化，请重新检查".into());
    }
    let mut seen = HashSet::new();
    for row in &report.checks {
        if !seen.insert(row.id) {
            reasons.push("检查报告包含重复项".into());
        }
        if row.observed_at > now
            || row.valid_until < row.observed_at
            || row.valid_until > row.observed_at.saturating_add(EVIDENCE_TTL_SECONDS)
            || now > row.valid_until
        {
            reasons.push(format!("{}证据已过期或时间无效，请重新检查", row.id.name()));
        }
        if row.network_generation != report.network_generation {
            reasons.push(format!("{}证据网络代次不一致，请重新检查", row.id.name()));
        }
        if row.status != CheckStatus::Pass
            && (row.id.required() || row.status != CheckStatus::Warning)
        {
            reasons.push(format!("{}未通过", row.id.name()));
        }
    }
    if report.checks.len() != CheckId::ALL.len() || CheckId::ALL.iter().any(|id| !seen.contains(id))
    {
        reasons.push("检查报告缺少必需项目".into());
    }
    let expected = summarize(&report.checks);
    if report.summary.passed != expected.passed
        || report.summary.advisory != expected.advisory
        || report.summary.blocking != expected.blocking
    {
        reasons.push("检查摘要与逐项结果不一致".into());
    }
    reasons.sort();
    reasons.dedup();
    LaunchDecision {
        allowed: reasons.is_empty(),
        reasons,
    }
}

struct Context {
    config: CheckConfig,
    client: Option<reqwest::Client>,
    geo: OnceCell<network::GeoInfo>,
    geo_details: OnceCell<network::GeoInfo>,
    dns: OnceCell<Option<Vec<IpAddr>>>,
}
impl Context {
    async fn geo(&self) -> network::GeoInfo {
        self.geo
            .get_or_init(|| async {
                if let Some(client) = &self.client {
                    tokio::time::timeout(Duration::from_secs(21), network::lookup_egress(client))
                        .await
                        .unwrap_or_default()
                } else {
                    network::GeoInfo::default()
                }
            })
            .await
            .clone()
    }
    async fn geo_details(&self) -> network::GeoInfo {
        self.geo_details
            .get_or_init(|| async {
                if let Some(client) = &self.client {
                    tokio::time::timeout(Duration::from_secs(21), async {
                        network::lookup_geo_details(client, self.geo().await).await
                    })
                    .await
                    .unwrap_or_default()
                } else {
                    network::GeoInfo::default()
                }
            })
            .await
            .clone()
    }
    async fn dns(&self) -> Option<Vec<IpAddr>> {
        self.dns
            .get_or_init(|| async {
                let mut all = Vec::new();
                for host in SERVICE_HOSTS {
                    all.push(*macos::dns_addresses(host).await.ok()?.first()?);
                }
                Some(all)
            })
            .await
            .clone()
    }
}

type Outcome = (CheckStatus, &'static str, Vec<String>);
fn result(status: CheckStatus, reason: &'static str) -> Outcome {
    (status, reason, vec![])
}
fn compare(value: Result<bool, &'static str>, yes: &'static str, no: &'static str) -> Outcome {
    match value {
        Ok(true) => result(CheckStatus::Pass, yes),
        Ok(false) => result(CheckStatus::Fail, no),
        Err(_) => result(CheckStatus::Unknown, "无法读取系统状态"),
    }
}
fn fake_ip(ip: &IpAddr) -> bool {
    matches!(ip,IpAddr::V4(v) if v.octets()[0]==198 && v.octets()[1]==18)
}

fn transport_evidence(label: &str, probe: &TransportProbe) -> String {
    let transport = match probe.transport {
        TransportState::Reached => "已到达",
        TransportState::DnsFailure => "DNS 失败",
        TransportState::TlsFailure => "TLS 失败",
        TransportState::ConnectFailure => "连接失败",
        TransportState::Timeout => "超时",
        TransportState::ProxyAuthentication => "代理需要鉴权",
        TransportState::InvalidResponse => "无效响应",
    };
    let access = match probe.access {
        AccessState::Available => "服务已响应",
        AccessState::AuthenticationRequired => "需要认证",
        AccessState::Restricted => "访问限制",
        AccessState::Redirected => "重定向",
        AccessState::ServerError => "服务端错误",
        AccessState::Unknown => "待确认",
    };
    format!(
        "{label}：传输{transport}；访问{access}；HTTP {}",
        probe
            .http_status
            .map(|v| v.to_string())
            .unwrap_or_else(|| "未取得".into())
    )
}

fn legacy_transport_reached(probe: &TransportProbe) -> bool {
    probe.http_status.is_some_and(|code| code != 0)
}

fn language_is_english(app: Option<&str>, system: Option<&str>) -> bool {
    app.is_some_and(|value| value.lines().take(2).any(|line| line.contains("en-US")))
        || system
            .and_then(|value| value.lines().nth(1))
            .is_some_and(|line| {
                line.trim_matches(|c: char| c.is_whitespace() || matches!(c, ',' | '"'))
                    .starts_with("en")
            })
}

fn residential_asn(org: &str) -> bool {
    [
        "Comcast",
        "AT&T",
        "Verizon",
        "Spectrum",
        "Charter",
        "Cox",
        "T-Mobile",
        "Cable",
        "esidential",
        "NTT",
        "KDDI",
        "SoftBank",
        "IIJ",
        "OCN",
        "So-net",
        "HiNet",
        "Chunghwa",
        "CHT",
        "kbro",
        "Taiwan",
        "FarEasTone",
        "Data Communication",
        "Singtel",
        "StarHub",
        "M1",
        "HKT",
        "PCCW",
        "HKBN",
        "HGC",
        "BT",
        "Sky",
        "Virgin",
        "TalkTalk",
        "Vodafone",
        "Deutsche Telekom",
        "1&1",
        "O2",
    ]
    .iter()
    .any(|name| org.contains(name))
}

fn merge_config_ok() -> bool {
    let Ok(home) = crate::config::home_dir() else {
        return false;
    };
    merge_config_ok_at(&home)
}
fn merge_config_ok_at(home: &std::path::Path) -> bool {
    let Some(group) = crate::user_repairs::legacy_proxy_group(home) else {
        return false;
    };
    std::fs::read_to_string(home.join(
        "Library/Application Support/io.github.clash-verge-rev.clash-verge-rev/clash-verge.yaml",
    ))
    .is_ok_and(|text| text.contains(&format!("DOMAIN-SUFFIX,claude.ai,{group}")))
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct EgressHistory {
    previous: Option<String>,
    entries: Vec<(String, u64)>,
}
impl EgressHistory {
    fn record(&mut self, hash: String, now: u64) -> Outcome {
        let changed = self
            .previous
            .as_ref()
            .is_some_and(|previous| previous != &hash);
        self.previous = Some(hash.clone());
        self.entries
            .retain(|(_, time)| *time >= now.saturating_sub(172_800));
        self.entries.push((hash, now));
        let distinct = self
            .entries
            .iter()
            .map(|(hash, _)| hash)
            .collect::<HashSet<_>>()
            .len();
        if distinct >= 3 {
            result(
                CheckStatus::Warning,
                "48 小时内出口地址达到三个，请固定节点",
            )
        } else if changed {
            result(CheckStatus::Warning, "出口地址与上次不同")
        } else {
            result(CheckStatus::Pass, "出口地址一致或首次记录")
        }
    }
}
fn egress_hash(ip: IpAddr) -> String {
    Sha256::digest(ip.to_string().as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn legacy_history(home: &std::path::Path) -> EgressHistory {
    use std::{io::Read, os::unix::fs::OpenOptionsExt};
    let read = |name: &str| -> Option<String> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(home.join(".claude-switcher").join(name))
            .ok()?;
        if !file.metadata().ok()?.is_file() {
            return None;
        }
        let mut text = String::new();
        file.take(1024 * 1024).read_to_string(&mut text).ok()?;
        Some(text)
    };
    EgressHistory {
        previous: read("lastip")
            .and_then(|text| text.trim().parse().ok())
            .map(egress_hash),
        entries: read("iphistory.log")
            .unwrap_or_default()
            .lines()
            .filter_map(|line| {
                let mut fields = line.split_whitespace();
                Some((
                    egress_hash(fields.next()?.parse().ok()?),
                    fields.next()?.parse().ok()?,
                ))
            })
            .collect(),
    }
}
fn record_egress(ip: IpAddr, previous: Option<&str>) -> Outcome {
    let Ok(home) = crate::config::home_dir() else {
        return result(CheckStatus::Warning, "无法保存出口历史");
    };
    record_egress_at(&home, ip, previous)
}
fn record_egress_at(home: &std::path::Path, ip: IpAddr, previous: Option<&str>) -> Outcome {
    let path = home.join(".claude-switcher/v2/egress-history.json");
    let mut history: EgressHistory = crate::storage::read_private_bytes(&path, 1024 * 1024)
        .ok()
        .and_then(|data| serde_json::from_slice(&data).ok())
        .unwrap_or_else(|| legacy_history(home));
    if history.previous.is_none() {
        history.previous = previous.map(str::to_owned);
    }
    let hash = egress_hash(ip);
    let outcome = history.record(hash, unix_now());
    let save = || -> std::io::Result<()> {
        crate::storage::prepare_data_root(&home.join(".claude-switcher"))?;
        crate::storage::create_private_directory(path.parent().unwrap())?;
        crate::storage::atomic_json(&path, &history)
    };
    if save().is_err() {
        return result(CheckStatus::Warning, "无法保存出口历史");
    }
    outcome
}

async fn observe(id: CheckId, context: &Context) -> Outcome {
    use CheckId::*;
    match id {
        ProxyProcess => compare(
            macos::proxy_running(&context.config.proxy_mode).await,
            "代理客户端正在运行",
            "未发现选定代理客户端",
        ),
        TunRoute => compare(
            macos::tunnel_route().await,
            "外网路由经过隧道",
            "外网路由未经过隧道",
        ),
        EgressRegion => {
            let mut geo = context.geo().await;
            if geo.country.is_none() {
                geo = context.geo_details().await;
            }
            match (&context.config.country_target, geo.country) {
                (Some(target), Some(country)) if country == *target && country != "CN" => {
                    result(CheckStatus::Pass, "出口地区符合已保存目标")
                }
                (Some(_), Some(_)) => result(CheckStatus::Fail, "出口地区与已保存目标不一致"),
                _ => result(CheckStatus::Unknown, "出口地区或地区目标无法确认"),
            }
        }
        IpContinuity => match context.geo().await.ip {
            Some(ip) => record_egress(ip, context.config.previous_egress_hash.as_deref()),
            None => result(CheckStatus::Fail, "未获取到出口地址"),
        },
        Timezone => {
            let actual = std::fs::read_link("/etc/localtime")
                .ok()
                .map(|p| p.to_string_lossy().into_owned());
            match (&context.config.timezone_target, actual) {
                (Some(target), Some(path)) => compare(
                    Ok(path.ends_with(&format!("/zoneinfo/{target}"))),
                    "时区符合已保存目标",
                    "时区与已保存目标不一致",
                ),
                _ => result(CheckStatus::Unknown, "时区或时区目标无法确认"),
            }
        }
        Locale => {
            match (
                &context.config.locale_target,
                macos::read_command("/usr/bin/defaults", &["read", "-g", "AppleLocale"]).await,
            ) {
                (Some(target), Ok(actual)) => compare(
                    Ok(actual.trim() == target),
                    "地区格式符合已保存目标",
                    "地区格式与已保存目标不一致",
                ),
                _ => result(CheckStatus::Unknown, "地区格式或目标无法确认"),
            }
        }
        Ipv6Policy => {
            match macos::read_command("/usr/sbin/networksetup", &["-getinfo", "Wi-Fi"]).await {
                Ok(s) => compare(
                    Ok(s.lines().any(|line| {
                        line.trim().to_ascii_lowercase().starts_with("ipv6:")
                            && line.to_ascii_lowercase().contains("off")
                    })),
                    "所选网络服务已关闭 IPv6",
                    "所选网络服务未关闭 IPv6",
                ),
                Err(_) => result(CheckStatus::Unknown, "无法确认 IPv6 策略"),
            }
        }
        DnsRoute => match macos::dns_addresses("claude.ai").await {
            Ok(addresses) => compare(
                Ok(addresses.first().is_some_and(fake_ip)),
                "claude.ai 首个解析结果为 Fake-IP",
                "claude.ai 首个解析结果未走隧道",
            ),
            Err(_) => result(CheckStatus::Fail, "无法取得 claude.ai 首个解析结果"),
        },
        StunPath => {
            // No valid STUN response remains unknown; HTTPS cannot establish UDP path safety.
            let results = network::stun_all().await;
            if results.iter().all(Result::is_err) {
                stun_outcome(&results, None)
            } else {
                // The shared geo lookup keeps its independent 21-second budget.
                stun_outcome(&results, context.geo().await.ip)
            }
        }

        Guardian => {
            if macos::read_command(
                "/bin/launchctl",
                &["print", "system/local.claude-bodyguard"],
            )
            .await
            .is_ok()
            {
                result(CheckStatus::Pass, "旧版保镖服务已注册")
            } else {
                let service = format!(
                    "gui/{}/{}",
                    unsafe { libc::geteuid() },
                    switcher_domain::system::GUARDIAN_LABEL
                );
                compare(
                    macos::read_command("/bin/launchctl", &["print", &service])
                        .await
                        .map(|_| true),
                    "用户保镖服务已注册",
                    "保镖未注册",
                )
            }
        }
        ServiceRoutes => {
            if context.config.proxy_mode == "clash-verge" && merge_config_ok() {
                return result(CheckStatus::Pass, "Clash 配置包含已保存代理组的置顶规则");
            }
            compare(
                Ok(context.dns().await.is_some_and(|addresses| {
                    addresses.len() == 3 && addresses.iter().all(fake_ip)
                })),
                "三个服务域名首个解析结果均为 Fake-IP",
                "服务域名置顶规则未生效",
            )
        }
        ClientLanguage => {
            let (app, system) = tokio::join!(
                macos::read_command(
                    "/usr/bin/defaults",
                    &["read", "com.anthropic.claudefordesktop", "AppleLanguages"]
                ),
                macos::read_command("/usr/bin/defaults", &["read", "-g", "AppleLanguages"]),
            );
            compare(
                Ok(language_is_english(
                    app.as_deref().ok(),
                    system.as_deref().ok(),
                )),
                "客户端或系统首选语言为英语",
                "客户端和系统首选语言未配置英语",
            )
        }
        AsnInfo => match context.geo_details().await.organization {
            Some(org) if residential_asn(&org) => {
                result(CheckStatus::Pass, "出口属于已收录的住宅运营商")
            }
            Some(_) => result(CheckStatus::Warning, "出口疑似机房网络"),
            None => result(CheckStatus::Warning, "未获得网络运营商信息"),
        },
        ServiceTransport => {
            let Some(client) = &context.client else {
                return result(CheckStatus::Unknown, "网络检查上下文不可用");
            };
            let (a, b) = tokio::join!(
                network::transport_probe(
                    client,
                    network::SERVICE_ENDPOINTS[0].0,
                    network::SERVICE_ENDPOINTS[0].1
                ),
                network::transport_probe(
                    client,
                    network::SERVICE_ENDPOINTS[1].0,
                    network::SERVICE_ENDPOINTS[1].1
                )
            );
            let evidence = vec![
                transport_evidence("API", &a),
                transport_evidence("网页", &b),
            ];
            if [&a, &b].iter().all(|p| legacy_transport_reached(p)) {
                (
                    CheckStatus::Pass,
                    "服务已返回 HTTP 状态；认证和访问结果单独列示",
                    evidence,
                )
            } else {
                (CheckStatus::Unknown, "部分服务未返回 HTTP 状态", evidence)
            }
        }
    }
}

pub fn stun_outcome(
    results: &[Result<IpAddr, network::StunError>],
    egress: Option<IpAddr>,
) -> Outcome {
    let successful: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
    let finish = |status, reason| {
        let evidence = vec![
            format!("有效 STUN 响应：{}", successful.len()),
            format!(
                "STUN 超时：{}",
                results
                    .iter()
                    .filter(|r| matches!(r, Err(network::StunError::Timeout)))
                    .count()
            ),
            format!(
                "STUN 网络或 DNS 失败：{}",
                results
                    .iter()
                    .filter(|r| matches!(r, Err(network::StunError::Dns | network::StunError::Io)))
                    .count()
            ),
            format!(
                "STUN 来源、事务或解析失败：{}",
                results
                    .iter()
                    .filter(|r| matches!(
                        r,
                        Err(network::StunError::WrongSource
                            | network::StunError::Header
                            | network::StunError::Transaction
                            | network::StunError::Length
                            | network::StunError::AddressFamily
                            | network::StunError::MissingAddress)
                    ))
                    .count()
            ),
        ];
        (status, reason, evidence)
    };
    if successful.is_empty() {
        return finish(
            CheckStatus::Unknown,
            "STUN 未收到有效响应，无法确认 UDP 路径",
        );
    }
    let Some(egress) = egress else {
        return finish(
            CheckStatus::Unknown,
            "缺少 HTTPS 出口地址，无法比较 STUN 路径",
        );
    };
    if successful.iter().any(|ip| **ip != egress) {
        return finish(CheckStatus::Fail, "STUN 出口与 HTTPS 出口不一致");
    }
    finish(CheckStatus::Pass, "已返回 STUN 出口均与 HTTPS 出口一致")
}

pub fn summarize(checks: &[CheckResult]) -> CheckSummary {
    CheckSummary {
        passed: checks
            .iter()
            .filter(|r| r.status == CheckStatus::Pass)
            .count() as u32,
        advisory: checks
            .iter()
            .filter(|r| !r.id.required() && r.status == CheckStatus::Warning)
            .count() as u32,
        blocking: checks
            .iter()
            .filter(|r| {
                r.status != CheckStatus::Pass
                    && (r.id.required() || r.status != CheckStatus::Warning)
            })
            .count() as u32,
    }
}

async fn collect_reports(
    mut tasks: tokio::task::JoinSet<CheckResult>,
    deadline: tokio::time::Instant,
    observed_at: u64,
    generation: &str,
) -> Vec<CheckResult> {
    let mut checks = Vec::new();
    while !tasks.is_empty() {
        let next = tokio::time::timeout_at(deadline, tasks.join_next()).await;
        match next {
            Ok(Some(Ok(row))) => checks.push(row),
            Ok(Some(Err(_))) => {}
            _ => {
                tasks.abort_all();
                break;
            }
        }
    }
    for id in CheckId::ALL {
        if !checks.iter().any(|r| r.id == id) {
            checks.push(CheckResult {
                id,
                status: CheckStatus::Timeout,
                reason: "检查到达总截止时间或任务未完成".into(),
                observed_at,
                duration_ms: TOTAL_CHECK_TIMEOUT.as_millis() as u64,
                valid_until: observed_at + EVIDENCE_TTL_SECONDS,
                network_generation: generation.into(),
                evidence: vec![],
            });
        }
    }
    checks.sort_by_key(|r| CheckId::ALL.iter().position(|id| *id == r.id));
    checks
}

pub async fn run_checks(config: CheckConfig) -> CheckReport {
    run_with_generation(
        macos::network_generation,
        |generation| {
            let valid = config.validate().is_ok();
            let context = Arc::new(Context {
                config,
                client: network::inherited_client().ok(),
                geo: OnceCell::new(),
                geo_details: OnceCell::new(),
                dns: OnceCell::new(),
            });
            let mut tasks = tokio::task::JoinSet::new();
            for id in CheckId::ALL {
                let context = context.clone();
                let generation = generation.to_owned();
                tasks.spawn(async move {
                    let start = Instant::now();
                    let observed = unix_now();
                    let (status, reason, evidence) = if valid {
                        observe(id, &context).await
                    } else {
                        result(CheckStatus::Unknown, "保存的检查配置无效")
                    };
                    CheckResult {
                        id,
                        status,
                        reason: reason.into(),
                        observed_at: observed,
                        duration_ms: start.elapsed().as_millis() as u64,
                        valid_until: observed.saturating_add(EVIDENCE_TTL_SECONDS),
                        network_generation: generation,
                        evidence,
                    }
                });
            }
            tasks
        },
        TOTAL_CHECK_TIMEOUT,
    )
    .await
}

async fn sample_generation<S, F>(
    sample: &mut S,
    deadline: tokio::time::Instant,
) -> Result<String, &'static str>
where
    S: FnMut() -> F,
    F: std::future::Future<Output = Result<String, &'static str>>,
{
    if tokio::time::Instant::now() >= deadline {
        return Err("network_generation_timeout");
    }
    let generation = tokio::time::timeout_at(deadline, sample())
        .await
        .map_err(|_| "network_generation_timeout")??;
    if tokio::time::Instant::now() > deadline {
        return Err("network_generation_timeout");
    }
    if generation.trim().is_empty() {
        return Err("network_generation_unavailable");
    }
    Ok(generation)
}

async fn run_with_generation<S, F>(
    mut sample: S,
    spawn_checks: impl FnOnce(&str) -> tokio::task::JoinSet<CheckResult>,
    budget: Duration,
) -> CheckReport
where
    S: FnMut() -> F,
    F: std::future::Future<Output = Result<String, &'static str>>,
{
    let start = Instant::now();
    let deadline = tokio::time::Instant::now() + budget;
    let observed_at = unix_now();
    let initial = sample_generation(&mut sample, deadline).await;
    let generation = initial.as_deref().unwrap_or_default().to_owned();
    let tasks = if tokio::time::Instant::now() < deadline {
        spawn_checks(&generation)
    } else {
        tokio::task::JoinSet::new()
    };
    let mut report = finish_check_report(tasks, deadline, start, observed_at, generation).await;
    let current = sample_generation(&mut sample, deadline).await;
    if !matches!((&initial, &current), (Ok(before), Ok(after)) if before == after) {
        // A later consumer must not revive evidence collected across an unverified network epoch.
        report.network_generation.clear();
    }
    report.completed_at = unix_now();
    report.duration_ms = start.elapsed().as_millis() as u64;
    report.launch = evaluate_launch(
        &report,
        report.completed_at,
        current.as_deref().unwrap_or_default(),
    );
    for (stage, sample) in [("检查前", initial), ("检查后", current)] {
        if let Err(error) = sample {
            report.launch.allowed = false;
            report
                .launch
                .reasons
                .push(format!("{stage}无法确认网络代次（{error}），请重新检查"));
        }
    }
    report
}

async fn finish_check_report(
    tasks: tokio::task::JoinSet<CheckResult>,
    deadline: tokio::time::Instant,
    start: Instant,
    observed_at: u64,
    generation: String,
) -> CheckReport {
    let checks = collect_reports(tasks, deadline, observed_at, &generation).await;
    let summary = summarize(&checks);
    let mut report = CheckReport {
        schema_version: 1,
        observed_at,
        completed_at: unix_now(),
        duration_ms: start.elapsed().as_millis() as u64,
        network_generation: generation.clone(),
        checks,
        summary,
        launch: LaunchDecision {
            allowed: false,
            reasons: vec![],
        },
    };
    report.launch = evaluate_launch(&report, unix_now(), &generation);
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn legacy_guardian_registration_passes_without_health_signature() {
        use macos::command_fixture::*;
        let outputs = Outputs::from([(
            key(
                "/bin/launchctl",
                &["print", "system/local.claude-bodyguard"],
            ),
            Ok("registered".into()),
        )]);
        OUTPUTS
            .scope(outputs, async {
                let context = Context {
                    config: CheckConfig::default(),
                    client: None,
                    geo: OnceCell::new(),
                    geo_details: OnceCell::new(),
                    dns: OnceCell::new(),
                };
                assert_eq!(
                    observe(CheckId::Guardian, &context).await.0,
                    CheckStatus::Pass
                );
            })
            .await;
    }
    #[test]
    fn legacy_merge_group_and_hashed_history_persist_in_fixture_home() {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "switcher-checks-{}-{}",
            std::process::id(),
            unix_now()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir_all(root.join(".local/bin")).unwrap();
        std::fs::write(
            root.join(".local/bin/claude-switch.sh"),
            "PROXY_GROUP=\"SavedGroup\" # fixed\n",
        )
        .unwrap();
        let clash =
            root.join("Library/Application Support/io.github.clash-verge-rev.clash-verge-rev");
        std::fs::create_dir_all(&clash).unwrap();
        std::fs::write(
            clash.join("clash-verge.yaml"),
            "rules: [DOMAIN-SUFFIX,claude.ai,SavedGroup]\n",
        )
        .unwrap();
        assert!(merge_config_ok_at(&root));
        std::fs::write(
            clash.join("clash-verge.yaml"),
            "rules: [DOMAIN-SUFFIX,claude.ai,OtherGroup]\n",
        )
        .unwrap();
        assert!(!merge_config_ok_at(&root));
        std::fs::create_dir_all(root.join(".claude-switcher")).unwrap();
        std::fs::write(root.join(".claude-switcher/lastip"), "203.0.113.8\n").unwrap();
        std::fs::write(
            root.join(".claude-switcher/iphistory.log"),
            format!("203.0.113.7 {}\n203.0.113.6 1\n", unix_now()),
        )
        .unwrap();
        let imported = legacy_history(&root);
        assert_eq!(imported.entries.len(), 2);
        assert_eq!(
            imported.previous.as_deref(),
            Some(egress_hash("203.0.113.8".parse().unwrap()).as_str())
        );
        assert_eq!(
            record_egress_at(&root, "203.0.113.8".parse().unwrap(), None).0,
            CheckStatus::Pass
        );
        assert_eq!(
            record_egress_at(&root, "203.0.113.8".parse().unwrap(), None).0,
            CheckStatus::Pass
        );
        assert_eq!(
            record_egress_at(&root, "203.0.113.9".parse().unwrap(), None).0,
            CheckStatus::Warning
        );
        let bytes = std::fs::read(root.join(".claude-switcher/v2/egress-history.json")).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains("203.0.113"));
        let history: EgressHistory = serde_json::from_str(&text).unwrap();
        assert_eq!(history.entries.len(), 4);
        assert!(history.entries.iter().all(|(hash, _)| hash.len() == 64));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn legacy_dns_routes_language_and_wifi_use_original_scope() {
        use macos::command_fixture::*;
        let outputs = Outputs::from([
            (
                key(
                    "/usr/bin/dig",
                    &["+short", "+time=3", "+tries=1", "claude.ai"],
                ),
                Ok("198.18.0.1\n203.0.113.8\n".into()),
            ),
            (
                key(
                    "/usr/bin/dig",
                    &["+short", "+time=3", "+tries=1", "api.anthropic.com"],
                ),
                Ok("198.18.0.2\n".into()),
            ),
            (
                key(
                    "/usr/bin/dig",
                    &["+short", "+time=3", "+tries=1", "claudeusercontent.com"],
                ),
                Ok("198.18.0.3\n".into()),
            ),
            (
                key("/usr/sbin/networksetup", &["-getinfo", "Wi-Fi"]),
                Ok("IPv6: off\n".into()),
            ),
            (
                key(
                    "/usr/bin/defaults",
                    &["read", "com.anthropic.claudefordesktop", "AppleLanguages"],
                ),
                Ok("(\nzh-CN\n)".into()),
            ),
            (
                key("/usr/bin/defaults", &["read", "-g", "AppleLanguages"]),
                Ok("(\nen-GB\n)".into()),
            ),
        ]);
        OUTPUTS
            .scope(outputs, async {
                let context = Context {
                    config: CheckConfig {
                        network_service: "Ethernet".into(),
                        ..Default::default()
                    },
                    client: None,
                    geo: OnceCell::new(),
                    geo_details: OnceCell::new(),
                    dns: OnceCell::new(),
                };
                for id in [
                    CheckId::DnsRoute,
                    CheckId::ServiceRoutes,
                    CheckId::Ipv6Policy,
                    CheckId::ClientLanguage,
                ] {
                    assert_eq!(observe(id, &context).await.0, CheckStatus::Pass, "{id:?}");
                }
            })
            .await;
    }
    #[test]
    fn legacy_https_accepts_any_nonzero_http_status() {
        let mut probe = TransportProbe {
            endpoint: "fixture".into(),
            transport: TransportState::ProxyAuthentication,
            access: AccessState::Restricted,
            http_status: Some(407),
            tls_verified: false,
            duration_ms: 1,
        };
        assert!(legacy_transport_reached(&probe));
        for status in [200, 301, 401, 403, 500] {
            probe.http_status = Some(status);
            assert!(legacy_transport_reached(&probe));
        }
        probe.http_status = None;
        assert!(!legacy_transport_reached(&probe));
    }
    #[test]
    fn legacy_language_and_asn_conditions() {
        assert!(language_is_english(
            Some("(\nzh-CN,\nen-US\n)"),
            Some("(\nen-US,\nzh-CN\n)")
        ));
        assert!(language_is_english(Some("(\nen-US,\nzh-CN\n)"), None));
        assert!(!language_is_english(
            Some("(\nen-GB\n)"),
            Some("(\nzh-CN\n)")
        ));
        assert!(residential_asn("AS123 NTT Communications"));
        assert!(!residential_asn("AS123 Unknown Network"));
    }
    #[test]
    fn legacy_history_prunes_and_counts_distinct_hashes() {
        let mut history = EgressHistory::default();
        assert_eq!(history.record("one".into(), 100).0, CheckStatus::Pass);
        assert_eq!(history.record("one".into(), 101).0, CheckStatus::Pass);
        assert_eq!(history.record("two".into(), 102).0, CheckStatus::Warning);
        assert_eq!(history.record("three".into(), 103).0, CheckStatus::Warning);
        assert_eq!(history.entries.len(), 4);
        assert_eq!(history.record("three".into(), 172_904).0, CheckStatus::Pass);
        assert_eq!(history.entries.len(), 1);
    }
    #[tokio::test]
    async fn stun_no_response_is_unknown_without_waiting_for_slow_geo() {
        let fixture = network::test_support::Fixture::new([None; 4]).await;
        let blackhole = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let mut settings = fixture.settings.clone();
        settings.stun_servers =
            vec![("127.0.0.1".into(), blackhole.local_addr().unwrap().port()); 4];
        network::test_support::SETTINGS.scope(settings, async {
            let context = Context {
                config: CheckConfig::default(), client: None,
                geo: OnceCell::new(), geo_details: OnceCell::new(), dns: OnceCell::new(),
            };
            // Hold the same shared geo cell that the region check initializes in production.
            let slow_geo = context.geo.get_or_init(|| async {
                tokio::time::sleep(Duration::from_secs(11)).await;
                network::GeoInfo { ip: Some("203.0.113.8".parse().unwrap()), ..Default::default() }
            });
            tokio::pin!(slow_geo);
            let start = Instant::now();
            let outcome = tokio::select! {
                biased;
                _ = &mut slow_geo => panic!("STUN must finish before the slow geo lookup"),
                outcome = observe(CheckId::StunPath, &context) => outcome,
                _ = tokio::time::sleep(Duration::from_secs(5)) => panic!("no-response STUN must not wait for geo"),
            };
            assert_eq!(outcome.0, CheckStatus::Unknown);
            assert!(outcome.2.iter().any(|value| value == "有效 STUN 响应：0"));
            assert!(outcome.2.iter().any(|value| value == "STUN 超时：4"));
            let mut tasks = tokio::task::JoinSet::new();
            for mut row in fresh_good().checks {
                if row.id == CheckId::StunPath {
                    row.status = outcome.0;
                    row.reason = outcome.1.into();
                    row.evidence = outcome.2.clone();
                }
                tasks.spawn(async move { row });
            }
            let report = finish_check_report(
                tasks, tokio::time::Instant::now() + Duration::from_secs(1),
                Instant::now(), unix_now(), "generation".into(),
            ).await;
            assert_eq!(report.checks.iter().map(|row| row.id).collect::<Vec<_>>(), CheckId::ALL.to_vec());
            assert_eq!(report.summary.passed, 13);
            assert_eq!(report.summary.blocking, 1);
            assert!(!report.launch.allowed);
            assert!(!evaluate_launch(&report, unix_now(), "generation").allowed);
            println!("no-response STUN remained unknown in {}ms while geo remained pending for 11s", start.elapsed().as_millis());
        }).await;
    }
    #[tokio::test]
    async fn stun_production_path_does_not_wait_for_slow_asn() {
        let fixture = network::test_support::Fixture::new([
            Some("ip=203.0.113.8\nloc=JP\n"),
            None,
            None,
            None,
        ])
        .await;
        network::test_support::SETTINGS.scope(fixture.settings.clone(), async {
            let context = Context {
                config: CheckConfig { country_target: Some("JP".into()), ..Default::default() },
                client: Some(fixture.client()), geo: OnceCell::new(), geo_details: OnceCell::new(), dns: OnceCell::new(),
            };
            let start = Instant::now();
            let (stun, asn, region) = tokio::join!(
                async {
                    let started = Instant::now();
                    let outcome = tokio::time::timeout(Duration::from_millis(150), observe(CheckId::StunPath, &context)).await.expect("STUN must not wait for advisory ASN");
                    println!("controlled STUN completed in {}ms; four local UDP responses, ASN still advisory", started.elapsed().as_millis());
                    outcome
                },
                observe(CheckId::AsnInfo, &context),
                observe(CheckId::EgressRegion, &context),
            );
            assert_eq!(stun.0, CheckStatus::Pass);
            assert!(stun.2.iter().any(|value| value == "有效 STUN 响应：4"));
            assert_eq!(asn.0, CheckStatus::Warning);
            assert_eq!(region.0, CheckStatus::Pass);
            assert!(start.elapsed() < Duration::from_secs(2));
            assert!(fixture.requests.lock().unwrap().contains(&1));
        }).await;
    }
    #[tokio::test]
    async fn required_region_uses_same_ip_fallback_and_blocks_unverified_country() {
        for (fallback, expected) in [
            (r#"{"ip":"203.0.113.8","country":"JP"}"#, CheckStatus::Pass),
            (
                r#"{"ip":"203.0.113.9","country":"JP"}"#,
                CheckStatus::Unknown,
            ),
            (r#"{"success":false}"#, CheckStatus::Unknown),
            (r#"{"ip":"203.0.113.8","country":"CN"}"#, CheckStatus::Fail),
        ] {
            let fixture = network::test_support::Fixture::new([
                Some("ip=203.0.113.8\n"),
                Some(fallback),
                Some(fallback),
                Some(fallback),
            ])
            .await;
            network::test_support::SETTINGS
                .scope(fixture.settings.clone(), async {
                    let context = Context {
                        config: CheckConfig {
                            country_target: Some("JP".into()),
                            ..Default::default()
                        },
                        client: Some(fixture.client()),
                        geo: OnceCell::new(),
                        geo_details: OnceCell::new(),
                        dns: OnceCell::new(),
                    };
                    let outcome = observe(CheckId::EgressRegion, &context).await;
                    assert_eq!(outcome.0, expected);
                    let mut report = good();
                    report
                        .checks
                        .iter_mut()
                        .find(|row| row.id == CheckId::EgressRegion)
                        .unwrap()
                        .status = outcome.0;
                    report.summary = summarize(&report.checks);
                    assert_eq!(
                        evaluate_launch(&report, 101, "generation").allowed,
                        expected == CheckStatus::Pass
                    );
                })
                .await;
        }
    }
    #[tokio::test]
    async fn stun_production_path_blocks_missing_or_different_egress() {
        for (trace, expected) in [
            ("ip=203.0.113.9\nloc=JP\n", CheckStatus::Fail),
            ("loc=JP\n", CheckStatus::Unknown),
        ] {
            let invalid = Some(r#"{"success":false}"#);
            let fixture =
                network::test_support::Fixture::new([Some(trace), invalid, invalid, invalid]).await;
            network::test_support::SETTINGS
                .scope(fixture.settings.clone(), async {
                    let context = Context {
                        config: CheckConfig {
                            country_target: Some("JP".into()),
                            ..Default::default()
                        },
                        client: Some(fixture.client()),
                        geo: OnceCell::new(),
                        geo_details: OnceCell::new(),
                        dns: OnceCell::new(),
                    };
                    let outcome = observe(CheckId::StunPath, &context).await;
                    assert_eq!(outcome.0, expected);
                    assert!(outcome.2.iter().any(|value| value == "有效 STUN 响应：4"));
                    let mut report = good();
                    report
                        .checks
                        .iter_mut()
                        .find(|row| row.id == CheckId::StunPath)
                        .unwrap()
                        .status = outcome.0;
                    report.summary = summarize(&report.checks);
                    assert!(!evaluate_launch(&report, 101, "generation").allowed);
                })
                .await;
        }
    }
    fn good() -> CheckReport {
        CheckReport {
            schema_version: 1,
            observed_at: 100,
            completed_at: 101,
            duration_ms: 1000,
            network_generation: "generation".into(),
            checks: CheckId::ALL
                .into_iter()
                .map(|id| CheckResult {
                    id,
                    status: CheckStatus::Pass,
                    reason: "test".into(),
                    observed_at: 100,
                    duration_ms: 10,
                    valid_until: 130,
                    network_generation: "generation".into(),
                    evidence: vec![],
                })
                .collect(),
            summary: CheckSummary {
                passed: 14,
                advisory: 0,
                blocking: 0,
            },
            launch: LaunchDecision {
                allowed: false,
                reasons: vec![],
            },
        }
    }
    fn fresh_good() -> CheckReport {
        let mut report = good();
        let now = unix_now();
        report.observed_at = now;
        report.completed_at = now;
        for row in &mut report.checks {
            row.observed_at = now;
            row.valid_until = now + EVIDENCE_TTL_SECONDS;
        }
        report
    }

    fn synthetic_tasks(generation: &str) -> tokio::task::JoinSet<CheckResult> {
        let mut tasks = tokio::task::JoinSet::new();
        for mut row in fresh_good().checks {
            row.network_generation = generation.into();
            tasks.spawn(async move { row });
        }
        tasks
    }

    #[test]
    fn gate_rejects_expired_future_and_inconsistent_evidence_times() {
        for now in [99, 131, 1_000_000] {
            assert!(!evaluate_launch(&good(), now, "generation").allowed);
        }
        for change in 0..6 {
            let mut report = good();
            match change {
                0 => report.observed_at = 102,
                1 => report.completed_at = 102,
                2 => report.checks[0].observed_at = 102,
                3 => report.checks[0].valid_until = 100,
                4 => report.checks[0].valid_until = 99,
                _ => report.checks[0].valid_until = 200,
            }
            assert!(!evaluate_launch(&report, 101, "generation").allowed);
        }
        assert!(evaluate_launch(&good(), 130, "generation").allowed);
        let mut boundary = good();
        boundary.completed_at = 100;
        assert!(evaluate_launch(&boundary, 130, "generation").allowed);
        let mut report = good();
        for row in &mut report.checks {
            row.observed_at = 120;
            row.valid_until = 150;
        }
        assert!(!evaluate_launch(&report, 132, "generation").allowed);
    }

    #[test]
    fn gate_rejects_changed_empty_or_inconsistent_network_generation() {
        for current in ["changed", "", " "] {
            assert!(!evaluate_launch(&good(), 101, current).allowed);
        }
        let mut report = good();
        report.network_generation.clear();
        assert!(!evaluate_launch(&report, 101, "").allowed);
        for row_generation in ["old", "", " "] {
            let mut report = good();
            report.checks[0].network_generation = row_generation.into();
            assert!(!evaluate_launch(&report, 101, "generation").allowed);
        }
    }

    #[test]
    fn gate_requires_complete_unique_results_and_fresh_evidence() {
        assert!(evaluate_launch(&good(), 101, "generation").allowed);
        let mut report = good();
        report.summary.passed = 13;
        assert!(!evaluate_launch(&report, 101, "generation").allowed);
        let mut report = good();
        report.checks.pop();
        assert!(!evaluate_launch(&report, 101, "generation").allowed);
        let mut report = good();
        report.checks[1] = report.checks[0].clone();
        assert!(!evaluate_launch(&report, 101, "generation").allowed);
        for now in [101, 129, 130] {
            assert!(evaluate_launch(&good(), now, "generation").allowed);
        }
        assert!(!evaluate_launch(&good(), 101, "changed").allowed);
        let mut report = good();
        report.checks[0].network_generation = "old".into();
        report.checks[0].valid_until = 200;
        assert!(!evaluate_launch(&report, 101, "generation").allowed);
    }
    #[test]
    fn only_two_advisory_warnings_can_pass_the_gate() {
        for id in CheckId::ALL {
            for status in [
                CheckStatus::Warning,
                CheckStatus::Unknown,
                CheckStatus::Fail,
                CheckStatus::Timeout,
            ] {
                let mut report = good();
                report
                    .checks
                    .iter_mut()
                    .find(|r| r.id == id)
                    .unwrap()
                    .status = status;
                report.summary = summarize(&report.checks);
                assert_eq!(
                    evaluate_launch(&report, 101, "generation").allowed,
                    !id.required() && status == CheckStatus::Warning
                );
            }
        }
    }
    #[test]
    fn stun_without_valid_responses_is_unknown_with_or_without_https_egress() {
        use network::StunError;
        let ip = "203.0.113.8".parse().unwrap();
        let mut cases = vec![vec![]];
        for error in [
            StunError::Timeout,
            StunError::Dns,
            StunError::Io,
            StunError::WrongSource,
            StunError::Header,
            StunError::Transaction,
            StunError::Length,
            StunError::AddressFamily,
            StunError::MissingAddress,
        ] {
            cases.push(vec![Err(error); 4]);
        }
        cases.push(vec![
            Err(StunError::Timeout),
            Err(StunError::Dns),
            Err(StunError::Io),
            Err(StunError::Header),
        ]);
        for results in cases {
            for egress in [None, Some(ip)] {
                let outcome = stun_outcome(&results, egress);
                assert_eq!(outcome.0, CheckStatus::Unknown, "{results:?}, {egress:?}");
                assert!(outcome.1.contains("无法确认"));
                assert!(outcome.2.iter().any(|item| item == "有效 STUN 响应：0"));
                let mut report = good();
                let row = report
                    .checks
                    .iter_mut()
                    .find(|row| row.id == CheckId::StunPath)
                    .unwrap();
                row.status = outcome.0;
                row.reason = outcome.1.into();
                row.evidence = outcome.2;
                report.summary = summarize(&report.checks);
                assert_eq!(report.checks.len(), 14);
                assert_eq!(report.summary.passed, 13);
                assert_eq!(report.summary.blocking, 1);
                assert!(!evaluate_launch(&report, 101, "generation").allowed);
            }
        }
    }

    #[test]
    fn stun_partial_response_matches_egress_but_mismatch_blocks() {
        let ip = "203.0.113.8".parse().unwrap();
        // Current PRD rejects no-response evidence; valid partial responses retain comparison rules.
        assert_eq!(
            stun_outcome(
                &std::array::from_fn::<_, 4, _>(|_| Err(network::StunError::Timeout)),
                Some(ip)
            )
            .0,
            CheckStatus::Unknown
        );
        assert_eq!(
            stun_outcome(&[Ok(ip), Err(network::StunError::Timeout)], Some(ip)).0,
            CheckStatus::Pass
        );
        assert_eq!(
            stun_outcome(&[Ok("203.0.113.9".parse().unwrap())], Some(ip)).0,
            CheckStatus::Fail
        );
        assert_eq!(stun_outcome(&[Ok(ip); 4], Some(ip)).0, CheckStatus::Pass);
        assert_eq!(
            stun_outcome(&[Ok(ip), Err(network::StunError::Io)], None).0,
            CheckStatus::Unknown
        );
    }
    #[tokio::test]
    async fn report_deadline_returns_all_ids_for_pending_and_missing_tasks() {
        let mut tasks = tokio::task::JoinSet::new();
        tasks.spawn(std::future::pending::<CheckResult>());
        let start = Instant::now();
        let report = tokio::time::timeout(
            Duration::from_millis(200),
            finish_check_report(
                tasks,
                tokio::time::Instant::now() + Duration::from_millis(10),
                start,
                unix_now(),
                "generation".into(),
            ),
        )
        .await
        .expect("the total deadline must bound unfinished observations");
        assert_eq!(report.checks.len(), CheckId::ALL.len());
        assert_eq!(
            report.checks.iter().map(|row| row.id).collect::<Vec<_>>(),
            CheckId::ALL.to_vec()
        );
        assert!(
            report
                .checks
                .iter()
                .all(|row| row.status == CheckStatus::Timeout)
        );
        assert_eq!(report.summary.blocking, CheckId::ALL.len() as u32);
        assert!(!report.launch.allowed);
    }

    #[tokio::test]
    async fn report_deadline_preserves_completed_results_and_aborts_pending_tasks() {
        struct Dropped(std::sync::Arc<std::sync::atomic::AtomicBool>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }

        let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let mut rows = fresh_good().checks.into_iter();
        let ready = rows.next().unwrap();
        let ready_id = ready.id;
        let mut tasks = tokio::task::JoinSet::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let pending_guard = Dropped(dropped.clone());
        tasks.spawn(async move {
            let _guard = pending_guard;
            started_tx.send(()).unwrap();
            std::future::pending::<CheckResult>().await
        });
        started_rx.await.unwrap();
        tasks.spawn(async move { ready });
        let report = finish_check_report(
            tasks,
            tokio::time::Instant::now() + Duration::from_millis(30),
            Instant::now(),
            unix_now(),
            "generation".into(),
        )
        .await;
        tokio::task::yield_now().await;

        assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(report.checks.len(), CheckId::ALL.len());
        let completed = report.checks.iter().find(|row| row.id == ready_id).unwrap();
        assert_eq!(completed.status, CheckStatus::Pass);
        assert_eq!(completed.reason, "test");
        assert_eq!(report.summary.passed, 1);
        assert_eq!(report.summary.blocking, 13);
        assert!(!report.launch.allowed);
    }

    #[tokio::test]
    async fn report_join_errors_and_cancellation_preserve_other_results_and_block_launch() {
        let mut tasks = tokio::task::JoinSet::new();
        for (index, row) in fresh_good().checks.into_iter().enumerate() {
            match index {
                0 => {
                    tasks.spawn(async { panic!("injected observation task panic") });
                }
                1 => {
                    tasks.spawn(std::future::pending::<CheckResult>()).abort();
                }
                _ => {
                    tasks.spawn(async move { row });
                }
            }
        }
        let report = finish_check_report(
            tasks,
            tokio::time::Instant::now() + Duration::from_secs(1),
            Instant::now(),
            unix_now(),
            "generation".into(),
        )
        .await;

        assert_eq!(
            report.checks.iter().map(|row| row.id).collect::<Vec<_>>(),
            CheckId::ALL.to_vec()
        );
        assert_eq!(report.summary.passed, 12);
        assert_eq!(report.summary.blocking, 2);
        assert!(
            report.checks[..2]
                .iter()
                .all(|row| row.status == CheckStatus::Timeout)
        );
        assert!(
            report.checks[2..]
                .iter()
                .all(|row| row.status == CheckStatus::Pass)
        );
        assert!(!report.launch.allowed);
    }

    #[tokio::test]
    async fn report_completed_results_allow_only_a_complete_passing_report() {
        assert_eq!(TOTAL_CHECK_TIMEOUT, Duration::from_secs(30));
        for status in [
            CheckStatus::Pass,
            CheckStatus::Fail,
            CheckStatus::Unknown,
            CheckStatus::Timeout,
        ] {
            let mut tasks = tokio::task::JoinSet::new();
            for (index, mut row) in fresh_good().checks.into_iter().enumerate() {
                if index == 0 {
                    row.status = status;
                }
                tasks.spawn(async move { row });
            }
            let report = finish_check_report(
                tasks,
                tokio::time::Instant::now() + Duration::from_secs(1),
                Instant::now(),
                unix_now(),
                "generation".into(),
            )
            .await;

            assert_eq!(report.checks.len(), CheckId::ALL.len());
            assert_eq!(report.checks[0].status, status);
            assert_eq!(report.launch.allowed, status == CheckStatus::Pass);
        }
    }

    #[tokio::test]
    async fn network_generation_before_and_after_checks_controls_permission() {
        for (before, after, allowed) in [
            (Ok("generation"), Ok("generation"), true),
            (Ok("generation"), Ok("changed"), false),
            (Err("start_unavailable"), Ok("generation"), false),
            (Ok("generation"), Err("end_unavailable"), false),
            (Ok(""), Ok("generation"), false),
            (Ok("generation"), Ok(" "), false),
        ] {
            let mut samples = [before, after].into_iter();
            let mut calls = 0;
            let report = run_with_generation(
                || {
                    calls += 1;
                    std::future::ready(samples.next().unwrap().map(str::to_owned))
                },
                synthetic_tasks,
                Duration::from_secs(1),
            )
            .await;
            assert_eq!(
                calls, 2,
                "the final generation must be sampled independently"
            );
            assert_eq!(report.launch.allowed, allowed);
            assert_eq!(report.summary.passed, 14);
            assert_eq!(
                report.checks.iter().map(|row| row.id).collect::<Vec<_>>(),
                CheckId::ALL.to_vec()
            );
            if !allowed {
                for later_generation in ["generation", "changed"] {
                    assert!(
                        !evaluate_launch(&report, unix_now(), later_generation).allowed,
                        "later sampling must not revive an invalidated report"
                    );
                }
                assert!(
                    report
                        .launch
                        .reasons
                        .iter()
                        .any(|reason| reason.contains("网络代次"))
                );
            }
        }
    }

    #[tokio::test]
    async fn network_generation_sampling_timeout_keeps_complete_reports() {
        for pending_sample in [0, 1] {
            let mut calls = 0;
            let mut observations_started = false;
            let report = tokio::time::timeout(
                Duration::from_millis(200),
                run_with_generation(
                    || {
                        let pending = calls == pending_sample;
                        calls += 1;
                        async move {
                            if pending {
                                std::future::pending::<()>().await;
                            }
                            Ok("generation".into())
                        }
                    },
                    |generation| {
                        observations_started = true;
                        synthetic_tasks(generation)
                    },
                    Duration::from_millis(20),
                ),
            )
            .await
            .expect("sampling must share the total check deadline");
            assert!(!report.launch.allowed);
            assert!(!evaluate_launch(&report, unix_now(), "generation").allowed);
            assert_eq!(observations_started, pending_sample == 1);
            assert_eq!(report.checks.len(), CheckId::ALL.len());
            assert_eq!(
                report.summary.passed,
                if pending_sample == 1 { 14 } else { 0 }
            );
            if pending_sample == 0 {
                assert!(
                    report
                        .checks
                        .iter()
                        .all(|row| row.status == CheckStatus::Timeout)
                );
            }
            assert!(
                report
                    .launch
                    .reasons
                    .iter()
                    .any(|reason| reason.contains("network_generation_timeout"))
            );
        }
    }

    #[tokio::test]
    async fn network_generation_late_synchronous_sample_cannot_allow_launch() {
        let mut calls = 0;
        let report = run_with_generation(
            || {
                calls += 1;
                let final_sample = calls == 2;
                async move {
                    if final_sample {
                        // Model a native synchronous operation that returns after the deadline.
                        std::thread::sleep(Duration::from_millis(250));
                    }
                    Ok("generation".into())
                }
            },
            synthetic_tasks,
            Duration::from_millis(200),
        )
        .await;
        assert_eq!(calls, 2);
        assert_eq!(report.summary.passed, 14);
        assert!(!report.launch.allowed);
        assert!(!evaluate_launch(&report, unix_now(), "generation").allowed);
    }

    #[tokio::test]
    async fn network_generation_samples_share_one_budget_with_observations() {
        let report = run_with_generation(
            || async {
                tokio::time::sleep(Duration::from_millis(30)).await;
                Ok("generation".into())
            },
            synthetic_tasks,
            Duration::from_millis(45),
        )
        .await;
        assert_eq!(report.summary.passed, 14);
        assert!(!report.launch.allowed);
        assert!(
            report
                .launch
                .reasons
                .iter()
                .any(|reason| reason.contains("检查后"))
        );
    }

    #[tokio::test]
    async fn incomplete_check_tasks_keep_all_ids_and_block() {
        for _ in 0..10 {
            for blocked in [false, true] {
                let mut tasks = tokio::task::JoinSet::new();
                for row in good().checks {
                    tasks.spawn(async move {
                        if blocked {
                            tokio::time::sleep(Duration::from_secs(60)).await;
                        }
                        row
                    });
                }
                let start = Instant::now();
                let checks = collect_reports(
                    tasks,
                    tokio::time::Instant::now() + Duration::from_millis(5),
                    100,
                    "generation",
                )
                .await;
                assert!(start.elapsed() < Duration::from_secs(1));
                assert_eq!(checks.len(), 14);
                let mut report = good();
                report.summary = summarize(&checks);
                report.checks = checks;
                assert_eq!(
                    evaluate_launch(&report, 101, "generation").allowed,
                    !blocked
                );
                assert!(!evaluate_launch(&report, 101, "changed").allowed);
                if blocked {
                    assert!(
                        report
                            .checks
                            .iter()
                            .all(|r| r.status == CheckStatus::Timeout)
                    );
                }
            }
        }
    }
}
