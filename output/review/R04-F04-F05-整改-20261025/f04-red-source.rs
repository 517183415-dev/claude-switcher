//! Fixed user-level repairs. Preview is read-only; every write requires the shared v2 lock.
//! A durable pending receipt is rolled back after a crash, never guessed to be committed.
use crate::storage::{self, LifecycleLock};
#[path = "pinned_routes.rs"]
mod pinned_routes;
use serde::{Deserialize, Serialize};
use std::{
    ffi::{CStr, CString, c_char, c_void},
    fs::{self, File, OpenOptions},
    io::{self, Read},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
fn fail(message: &'static str) -> io::Error {
    io::Error::other(message)
}

/// Read the installed legacy literal without executing user shell configuration.
pub(crate) fn legacy_proxy_group(home: &Path) -> Option<String> {
    fs::read_to_string(home.join(".local/bin/claude-switch.sh"))
        .ok()?
        .lines()
        .find_map(|line| crate::config::literal_assignment(line, "PROXY_GROUP"))
}

/// `Some` changes only the region format; `None` repairs only the client language.
/// The returned strings are per-item errors. One failure does not suppress later repairs.
pub(crate) fn repair_legacy_preferences(locale: Option<UserLocale>) -> Vec<String> {
    if cfg!(test) {
        return vec!["native_user_preferences_disabled_in_tests".into()];
    }
    repair_legacy_preferences_with(locale, &mut |args| {
        let output = Command::new("/usr/bin/defaults")
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err("用户偏好命令失败".into())
        }
    })
}
fn repair_legacy_preferences_with(
    locale: Option<UserLocale>,
    run: &mut impl FnMut(&[&str]) -> Result<String, String>,
) -> Vec<String> {
    let result = if let Some(locale) = locale {
        let current = run(&["read", "-g", "AppleLocale"]).unwrap_or_default();
        if current.trim() == locale.value() {
            Ok(String::new())
        } else {
            run(&["write", "-g", "AppleLocale", "-string", locale.value()])
        }
    } else {
        let app =
            run(&["read", "com.anthropic.claudefordesktop", "AppleLanguages"]).unwrap_or_default();
        let system = run(&["read", "-g", "AppleLanguages"]).unwrap_or_default();
        let first = system
            .lines()
            .nth(1)
            .unwrap_or("")
            .trim_matches(|c: char| c.is_whitespace() || matches!(c, ',' | '"'));
        if app.contains("en-US") || first.starts_with("en") {
            Ok(String::new())
        } else {
            run(&[
                "write",
                "com.anthropic.claudefordesktop",
                "AppleLanguages",
                "-array",
                "en-US",
            ])
        }
    };
    result.err().into_iter().collect()
}

/// Restore only the legacy top-level false text, after the normal three-second quit wait.
pub(crate) fn repair_legacy_tun() -> Result<bool, String> {
    if cfg!(test) {
        return Err("native_user_tun_disabled_in_tests".into());
    }
    let home =
        crate::migration::account_home(unsafe { libc::geteuid() }).map_err(|e| e.to_string())?;
    repair_legacy_tun_at(&home, &mut crate::proxy_repair::Native).map_err(|e| e.to_string())
}
pub(crate) fn repair_legacy_tun_at(
    home: &Path,
    runtime: &mut impl crate::proxy_repair::Runtime,
) -> io::Result<bool> {
    let path = home.join(Kind::Proxy.relative());
    let path = path.canonicalize().unwrap_or(path);
    let original = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    let (proposed, changed) = proxy_proposal(&original)?;
    if !changed {
        return Ok(false);
    }
    runtime.stop().map_err(fail)?;
    // Refuse to overwrite an unrelated edit made while the app was quitting.
    if fs::read(&path)? != original {
        return Err(fail("代理配置在退出期间变化"));
    }
    replace_file(&path, &proposed, unsafe { libc::geteuid() })?;
    runtime.start().map_err(fail)?;
    Ok(true)
}

/// Preserve legacy config-OR-route success. `true` means the caller already verified routes.
pub(crate) fn repair_legacy_merge(rules_ok: bool) -> Result<bool, String> {
    if cfg!(test) {
        return Err("native_user_merge_disabled_in_tests".into());
    }
    let home =
        crate::migration::account_home(unsafe { libc::geteuid() }).map_err(|e| e.to_string())?;
    repair_legacy_merge_at(&home, rules_ok).map_err(|e| e.to_string())
}
fn repair_legacy_merge_at(home: &Path, rules_ok: bool) -> io::Result<bool> {
    if rules_ok {
        return Ok(false);
    }
    let config_path = home.join(pinned_routes::GROUP_RELATIVE);
    let config = match fs::read(&config_path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    let group = legacy_proxy_group(home)
        .map(Ok)
        .unwrap_or_else(|| pinned_routes::first_select_group(&config))?;
    if String::from_utf8_lossy(&config).contains(&format!("DOMAIN-SUFFIX,claude.ai,{group}")) {
        return Ok(false);
    }
    let path = home.join(Kind::PinnedRoutes.relative());
    let path = path.canonicalize().unwrap_or(path);
    let before = match fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    let (proposed, changed) = pinned_routes::propose(before.as_deref(), &group)?;
    if !changed {
        return Ok(false);
    }
    fs::create_dir_all(path.parent().ok_or_else(|| fail("配置父目录缺失"))?)?;
    if let Some(original) = before {
        let backup = path.with_file_name(format!(
            "Merge.yaml.bak.{}-{}",
            std::process::id(),
            next_id()
        ));
        copy_file(&path, &backup)?;
        if fs::read(&path)? != original {
            return Err(fail("置顶规则在写入前变化"));
        }
        replace_file(&path, &proposed, unsafe { libc::geteuid() })?;
    } else {
        // Create the new file only after its full staged contents were written.
        let stage = stage_path(&path)?;
        let result = (|| {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&stage)?;
            std::io::Write::write_all(&mut file, &proposed)?;
            file.sync_all()?;
            if path.exists() {
                return Err(fail("置顶规则在创建前出现"));
            }
            fs::rename(&stage, &path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&stage);
        }
        result?;
    }
    Ok(true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserLocale {
    Us,
    Jp,
    Tw,
    Hk,
    Sg,
    Gb,
    De,
    Fr,
    Kr,
    Au,
    Ca,
}
impl UserLocale {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "en_US" => Self::Us,
            "ja_JP" => Self::Jp,
            "zh_TW" => Self::Tw,
            "zh_HK" => Self::Hk,
            "en_SG" => Self::Sg,
            "en_GB" => Self::Gb,
            "de_DE" => Self::De,
            "fr_FR" => Self::Fr,
            "ko_KR" => Self::Kr,
            "en_AU" => Self::Au,
            "en_CA" => Self::Ca,
            _ => return None,
        })
    }
    pub fn value(self) -> &'static str {
        match self {
            Self::Us => "en_US",
            Self::Jp => "ja_JP",
            Self::Tw => "zh_TW",
            Self::Hk => "zh_HK",
            Self::Sg => "en_SG",
            Self::Gb => "en_GB",
            Self::De => "de_DE",
            Self::Fr => "fr_FR",
            Self::Kr => "ko_KR",
            Self::Au => "en_AU",
            Self::Ca => "en_CA",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyRepairMode {
    Auto,
    ClashVerge,
    ZeroDCloud,
    Shadowrocket,
    SingBox,
    ClashX,
    Surge,
}
impl ProxyRepairMode {
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "auto" => Self::Auto,
            "clash-verge" => Self::ClashVerge,
            "0dcloud" => Self::ZeroDCloud,
            "shadowrocket" => Self::Shadowrocket,
            "sing-box" => Self::SingBox,
            "clashx" => Self::ClashX,
            "surge" => Self::Surge,
            _ => return None,
        })
    }
}
#[derive(Debug, Clone, Copy)]
pub struct UserRepairTarget {
    pub locale: UserLocale,
    pub proxy: ProxyRepairMode,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UserRepairState {
    Ready,
    Unchanged,
    Unsupported,
}
#[derive(Debug, Clone, Serialize)]
pub struct UserRepairItem {
    pub id: &'static str,
    pub state: UserRepairState,
    pub original: String,
    pub target: String,
    pub message: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct UserRepairPreview {
    pub items: Vec<UserRepairItem>,
    pub can_execute: bool,
    pub recovery_pending: bool,
}
#[derive(Debug)]
pub struct PreparedUserRepair {
    pub preview: UserRepairPreview,
    changes: Vec<Change>,
    home: PathBuf,
}
impl PreparedUserRepair {
    pub(crate) fn changes_proxy_files(&self) -> bool {
        self.changes
            .iter()
            .any(|change| matches!(change.kind, Kind::Proxy | Kind::PinnedRoutes))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserRepairReceipt {
    id: String,
}
#[derive(Debug)]
pub struct UserRepairCoordinator {
    home: PathBuf,
    owner: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
enum Kind {
    Locale,
    Language,
    Proxy,
    PinnedRoutes,
}
impl Kind {
    fn relative(self) -> &'static str {
        match self {
            Self::Locale => "Library/Preferences/.GlobalPreferences.plist",
            Self::Language => "Library/Preferences/com.anthropic.claudefordesktop.plist",
            Self::Proxy => {
                "Library/Application Support/io.github.clash-verge-rev.clash-verge-rev/verge.yaml"
            }
            Self::PinnedRoutes => {
                "Library/Application Support/io.github.clash-verge-rev.clash-verge-rev/profiles/Merge.yaml"
            }
        }
    }
    fn id(self) -> &'static str {
        match self {
            Self::Locale => "locale",
            Self::Language => "client_language",
            Self::Proxy => "proxy_tun_config",
            Self::PinnedRoutes => "proxy_pinned_routes",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Locale => "AppleLocale",
            Self::Language => "AppleLanguages",
            Self::Proxy => "enable_tun_mode",
            Self::PinnedRoutes => "prepend-rules",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Stamp {
    dev: u64,
    ino: u64,
    len: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    mtime: i64,
    mtime_ns: i64,
    ctime: i64,
    ctime_ns: i64,
    hash: String,
    attributes: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Change {
    kind: Kind,
    before: Option<Stamp>,
    // Exact metadata of the verified copy; absent in older schema-1 journals.
    #[serde(default)]
    backup_stamp: Option<Stamp>,
    #[serde(default)]
    group_source: Option<Stamp>,
    #[serde(skip)]
    target: Vec<u8>,
    target_hash: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Journal {
    schema: u8,
    id: String,
    changes: Vec<Change>,
    attempted: usize,
    after: Vec<Option<Stamp>>,
    committed: bool,
    #[serde(default)]
    rolled_back: bool,
}

impl UserRepairCoordinator {
    pub fn for_current_user(_edition: switcher_domain::Edition) -> io::Result<Self> {
        let owner = unsafe { libc::geteuid() };
        #[cfg(all(test, target_os = "macos"))]
        if let Some(home) = TEST_HOME.with(|value| value.borrow().clone()) {
            return Ok(Self { home, owner });
        }
        let home = crate::migration::account_home(owner)?;
        Ok(Self { home, owner })
    }
    fn v2(&self) -> PathBuf {
        self.home.join(".claude-switcher/v2")
    }
    fn directory(&self) -> PathBuf {
        self.v2().join("user-repairs")
    }
    fn journal(&self) -> PathBuf {
        self.directory().join("journal.json")
    }
    fn path(&self, kind: Kind) -> PathBuf {
        {
            let path = self.home.join(kind.relative());
            path.canonicalize().unwrap_or(path)
        }
    }
    pub fn preview(&self, target: UserRepairTarget) -> io::Result<PreparedUserRepair> {
        self.preview_inner(target, false)
    }
    pub fn preview_with_proxy_rules(
        &self,
        target: UserRepairTarget,
        rules_ok: bool,
    ) -> io::Result<PreparedUserRepair> {
        self.preview_inner(target, rules_ok)
    }
    fn preview_inner(
        &self,
        target: UserRepairTarget,
        rules_ok: bool,
    ) -> io::Result<PreparedUserRepair> {
        check_directory(&self.home, self.owner)?;
        let recovery_pending = self.journal().try_exists()?;
        let mut items = Vec::new();
        let mut changes = Vec::new();
        for (kind, value) in [
            (Kind::Locale, target.locale.value()),
            (Kind::Language, "en-US"),
        ] {
            let path = self.path(kind);
            self.check_user_path(&path)?;
            let (before, bytes) = snapshot(&path, self.owner)?;
            let language_ok = kind == Kind::Language && {
                let (_, global) = snapshot(&self.path(Kind::Locale), self.owner)?;
                preferences::language_acceptable(bytes.as_deref(), global.as_deref())
                    .unwrap_or(false)
            };
            match preferences::propose(bytes.as_deref(), kind, value) {
                Ok((original, proposed, changed)) => {
                    let changed = changed && !language_ok;
                    items.push(item(
                        kind,
                        if changed {
                            UserRepairState::Ready
                        } else {
                            UserRepairState::Unchanged
                        },
                        &original,
                        value,
                        "原生偏好同步后读取同一键验证；保留完整原文件备份",
                    ));
                    if changed {
                        changes.push(Change {
                            kind,
                            before,
                            backup_stamp: None,
                            group_source: None,
                            target_hash: crate::signed::digest(&proposed),
                            target: proposed,
                        });
                    }
                }
                Err(_) => items.push(item(
                    kind,
                    UserRepairState::Unsupported,
                    "无法安全解析",
                    value,
                    "偏好格式或类型不受支持；未生成写入计划",
                )),
            }
        }
        if target.proxy == ProxyRepairMode::ClashVerge {
            // A missing adapter directory is unsupported; an unsafe existing path is an error.
            if self.path(Kind::Proxy).parent().is_some_and(|p| p.exists()) {
                self.check_user_path(&self.path(Kind::Proxy))?;
            }
            match snapshot(&self.path(Kind::Proxy), self.owner) {
                Ok((Some(before), Some(bytes))) => match proxy_proposal(&bytes) {
                    Ok((proposed, changed)) => {
                        items.push(item(
                            Kind::Proxy,
                            if changed {
                                UserRepairState::Ready
                            } else {
                                UserRepairState::Unchanged
                            },
                            if changed { "false" } else { "true" },
                            "true",
                            "仅保存TUN配置；不重启代理，不证明TUN已生效",
                        ));
                        if changed {
                            changes.push(Change {
                                kind: Kind::Proxy,
                                before: Some(before),
                                backup_stamp: None,
                                group_source: None,
                                target_hash: crate::signed::digest(&proposed),
                                target: proposed,
                            });
                        }
                    }
                    Err(_) => items.push(item(
                        Kind::Proxy,
                        UserRepairState::Unsupported,
                        "未确认",
                        "true",
                        "仅支持唯一顶层布尔enable_tun_mode；拒绝未知格式",
                    )),
                },
                Ok(_) => items.push(item(
                    Kind::Proxy,
                    UserRepairState::Unsupported,
                    "配置缺失",
                    "true",
                    "Clash Verge固定配置不存在",
                )),
                Err(error) if error.kind() == io::ErrorKind::NotFound => items.push(item(
                    Kind::Proxy,
                    UserRepairState::Unsupported,
                    "配置缺失",
                    "true",
                    "Clash Verge固定目录不存在",
                )),
                Err(error) => return Err(error),
            }
        } else {
            items.push(item(
                Kind::Proxy,
                UserRepairState::Unsupported,
                "未确认",
                "客户端人工配置",
                "该代理模式没有已验证的安全写入适配器",
            ));
        }
        if rules_ok {
            items.push(item(
                Kind::PinnedRoutes,
                UserRepairState::Unchanged,
                "旧版路由检查通过",
                "Claude域名置顶规则",
                "已有配置或行为路由通过，跳过改写与重启",
            ));
        } else {
            self.preview_pinned_routes(target.proxy, &mut items, &mut changes)?;
        }
        Ok(PreparedUserRepair {
            preview: UserRepairPreview {
                can_execute: !recovery_pending && !changes.is_empty(),
                items,
                recovery_pending,
            },
            changes,
            home: self.home.clone(),
        })
    }
    fn preview_pinned_routes(
        &self,
        mode: ProxyRepairMode,
        items: &mut Vec<UserRepairItem>,
        changes: &mut Vec<Change>,
    ) -> io::Result<()> {
        let kind = Kind::PinnedRoutes;
        let unsupported = |message: &str| {
            item(
                kind,
                UserRepairState::Unsupported,
                "未确认",
                "Claude域名置顶规则",
                message,
            )
        };
        if mode != ProxyRepairMode::ClashVerge {
            items.push(unsupported("该代理模式没有已验证的Merge.yaml写入适配器"));
            return Ok(());
        }
        let group_path = self.home.join(pinned_routes::GROUP_RELATIVE);
        self.check_user_path(&group_path)?;
        let (Some(group_source), Some(group_bytes)) = snapshot(&group_path, self.owner)? else {
            items.push(unsupported("Clash Verge代理组配置缺失"));
            return Ok(());
        };
        let group_result = legacy_proxy_group(&self.home)
            .map(Ok)
            .unwrap_or_else(|| pinned_routes::first_select_group(&group_bytes));
        let Ok(group) = group_result else {
            items.push(unsupported("代理组格式复杂或含歧义；未生成置顶规则计划"));
            return Ok(());
        };
        if String::from_utf8_lossy(&group_bytes)
            .contains(&format!("DOMAIN-SUFFIX,claude.ai,{group}"))
        {
            items.push(item(
                kind,
                UserRepairState::Unchanged,
                "已有置顶配置",
                "Claude域名置顶规则",
                "旧版配置检查通过，跳过改写与重启",
            ));
            return Ok(());
        }
        let path = self.path(kind);
        self.check_user_path(&path)?;
        let (before, bytes) = snapshot(&path, self.owner)?;
        match pinned_routes::propose(bytes.as_deref(), &group) {
            Ok((proposed, changed)) => {
                items.push(item(
                    kind,
                    if changed {
                        UserRepairState::Ready
                    } else {
                        UserRepairState::Unchanged
                    },
                    if before.is_some() {
                        "已有Merge.yaml"
                    } else {
                        "文件缺失"
                    },
                    "Claude域名置顶规则",
                    "仅保存Merge.yaml；代理重载及规则生效尚未验证",
                ));
                if changed {
                    changes.push(Change {
                        kind,
                        before,
                        backup_stamp: None,
                        group_source: Some(group_source),
                        target_hash: crate::signed::digest(&proposed),
                        target: proposed,
                    });
                }
            }
            Err(_) => items.push(unsupported("Merge.yaml格式或标记不受支持；未生成写入计划")),
        }
        Ok(())
    }
    fn check_group_source(&self, change: &Change) -> io::Result<()> {
        if change.kind == Kind::PinnedRoutes {
            let path = self.home.join(pinned_routes::GROUP_RELATIVE);
            self.check_user_path(&path)?;
            if snapshot(&path, self.owner)?.0 != change.group_source {
                return Err(fail("代理组配置已在预览后变化，请重新预览"));
            }
        }
        Ok(())
    }
    fn check_user_path(&self, path: &Path) -> io::Result<()> {
        let mut parent = path.parent().ok_or_else(|| fail("配置父目录缺失"))?;
        while !parent.exists() {
            parent = parent.parent().ok_or_else(|| fail("配置父目录缺失"))?;
        }
        check_directory(parent, self.owner)
    }
    fn check_lock(&self, lock: &LifecycleLock) -> io::Result<()> {
        check_directory(&self.home, self.owner)?;
        if self.owner != unsafe { libc::geteuid() } || !lock.protects(&self.v2()) {
            return Err(fail("用户修复必须持有同一v2生命周期锁"));
        }
        Ok(())
    }
    pub fn execute_with_lock(
        &self,
        prepared: PreparedUserRepair,
        lock: &LifecycleLock,
    ) -> io::Result<UserRepairReceipt> {
        #[cfg(all(test, target_os = "macos"))]
        if let Some(prefs) = test_preferences(self) {
            return self.execute_inner(prepared, lock, &|| Ok(()), &prefs);
        }
        self.execute_inner(
            prepared,
            lock,
            &|| crate::accounts::ensure_claude_stopped(self.owner),
            &NativePreferences,
        )
    }
    fn execute_inner(
        &self,
        prepared: PreparedUserRepair,
        lock: &LifecycleLock,
        stopped: &dyn Fn() -> io::Result<()>,
        prefs: &dyn PreferenceWriter,
    ) -> io::Result<UserRepairReceipt> {
        self.check_lock(lock)?;
        stopped()?;
        if prepared.home != self.home
            || !prepared.preview.can_execute
            || self.journal().try_exists()?
        {
            return Err(fail("修复计划不可执行，或存在待恢复事务"));
        }
        for change in &prepared.changes {
            self.check_user_path(&self.path(change.kind))?;
            self.check_group_source(change)?;
            if snapshot(&self.path(change.kind), self.owner)?.0 != change.before {
                return Err(fail("配置已在预览后变化，请重新预览"));
            }
        }
        storage::create_private_directory(&self.directory())?;
        let id = format!(
            "repair-{}-{}-{}",
            std::process::id(),
            next_id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos()
        );
        let backup = self.directory().join(&id);
        storage::create_private_directory(&backup)?;
        let mut journal = Journal {
            schema: 1,
            id: id.clone(),
            changes: prepared.changes,
            attempted: 0,
            after: Vec::new(),
            committed: false,
            rolled_back: false,
        };
        // Full copies precede the first durable journal and the first user setting write.
        for (index, change) in journal.changes.iter_mut().enumerate() {
            storage::atomic_write(&backup.join(format!("target-{index}")), &change.target)?;
            if let Some(before) = &change.before {
                let source = self.path(change.kind);
                let saved = backup.join(format!("original-{index}"));
                copy_file(&source, &saved)?;
                let copied = snapshot(&saved, self.owner)?
                    .0
                    .ok_or_else(|| fail("备份缺失"))?;
                let matches = same_contents(before, &copied)
                    || (!matches!(change.kind, Kind::Proxy | Kind::PinnedRoutes)
                        && copied_with_added_provenance(
                            before,
                            &copied,
                            &metadata_digest_without_provenance(&saved)?,
                        ));
                if !matches
                    || snapshot(&saved, self.owner)?.0.as_ref() != Some(&copied)
                    || snapshot(&source, self.owner)?.0.as_ref() != Some(before)
                {
                    return Err(fail("配置备份验证失败或源已变化"));
                }
                change.backup_stamp = Some(copied);
            }
        }
        storage::atomic_json(&self.journal(), &journal)?;
        let result = (|| {
            for index in 0..journal.changes.len() {
                self.check_lock(lock)?;
                stopped()?;
                let change = &journal.changes[index];
                self.check_group_source(change)?;
                if snapshot(&self.path(change.kind), self.owner)?.0 != change.before {
                    return Err(fail("配置在执行前变化，请恢复后重新预览"));
                }
                journal.attempted = index + 1;
                storage::atomic_json(&self.journal(), &journal)?;
                let change = &journal.changes[index];
                self.apply(change, prefs)?;
                if let Some(before) = &change.before {
                    let path = self.path(change.kind);
                    let (_, bytes) = snapshot(&path, self.owner)?;
                    if !matches!(change.kind, Kind::Proxy | Kind::PinnedRoutes) {
                        replace_with_template(
                            &path,
                            bytes.as_deref().ok_or_else(|| fail("偏好同步后文件缺失"))?,
                            &backup.join(format!("original-{index}")),
                            self.owner,
                        )?;
                    }
                    let after = snapshot(&path, self.owner)?
                        .0
                        .ok_or_else(|| fail("写入后配置缺失"))?;
                    if before.mode != after.mode
                        || before.uid != after.uid
                        || before.gid != after.gid
                        || change.backup_stamp.as_ref().unwrap_or(before).attributes
                            != after.attributes
                    {
                        return Err(fail("写入后配置元数据未保留"));
                    }
                }
                let (after, bytes) = snapshot(&self.path(change.kind), self.owner)?;
                self.check_group_source(change)?;
                if !matches_target(change, bytes.as_deref())? {
                    return Err(fail("写入后实际值无法确认；需要恢复"));
                }
                journal.after.push(after);
                storage::atomic_json(&self.journal(), &journal)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            return match self.restore_inner(&journal, lock, stopped, prefs) {
                Ok(()) => Err(io::Error::other(format!(
                    "用户配置修复失败，已恢复原值：{error}"
                ))),
                Err(_) => Err(fail(
                    "用户配置修复中断；恢复证据冲突，请保留事务并先执行恢复",
                )),
            };
        }
        Ok(UserRepairReceipt { id })
    }
    fn apply(&self, change: &Change, prefs: &dyn PreferenceWriter) -> io::Result<()> {
        match change.kind {
            Kind::Proxy | Kind::PinnedRoutes if change.before.is_some() => {
                replace_file(&self.path(change.kind), &change.target, self.owner)
            }
            Kind::PinnedRoutes => {
                fs::create_dir_all(
                    self.path(change.kind)
                        .parent()
                        .ok_or_else(|| fail("配置父目录缺失"))?,
                )?;
                if snapshot(&self.path(change.kind), self.owner)?.0.is_some() {
                    return Err(fail("置顶规则文件在创建前出现"));
                }
                storage::atomic_write(&self.path(change.kind), &change.target)
            }
            kind => prefs.set(kind, &change.target),
        }
    }
    fn read_journal(&self) -> io::Result<Journal> {
        self.read_journal_at(&self.journal())
    }
    fn read_journal_at(&self, path: &Path) -> io::Result<Journal> {
        let mut journal: Journal = storage::read_json(path)?;
        if journal.schema != 1
            || journal.changes.len() > 4
            || journal.attempted > journal.changes.len()
            || journal.after.len() > journal.attempted
            || !valid_receipt_id(&journal.id)
            || (journal.committed && journal.rolled_back)
        {
            return Err(fail("用户修复恢复记录无效"));
        }
        let mut seen = Vec::new();
        for (index, change) in journal.changes.iter_mut().enumerate() {
            change.target = fs::read(
                self.directory()
                    .join(&journal.id)
                    .join(format!("target-{index}")),
            )?;
            if crate::signed::digest(&change.target) != change.target_hash {
                return Err(fail("用户修复目标备份校验失败"));
            }
            if seen.contains(&change.kind) {
                return Err(fail("用户修复恢复记录含重复配置"));
            }
            seen.push(change.kind);
        }
        Ok(journal)
    }
    pub(crate) fn pending_receipt_with_lock(
        &self,
        lock: &LifecycleLock,
    ) -> io::Result<Option<UserRepairReceipt>> {
        self.check_lock(lock)?;
        if !self.journal().try_exists()? {
            return Ok(None);
        }
        Ok(Some(UserRepairReceipt {
            id: self.read_journal()?.id,
        }))
    }
    fn receipt_journal(&self, receipt: &UserRepairReceipt) -> io::Result<Journal> {
        if !valid_receipt_id(&receipt.id) {
            return Err(fail("用户修复收据标识无效"));
        }
        if self.journal().try_exists()? {
            let journal = self.read_journal()?;
            if journal.id == receipt.id {
                return Ok(journal);
            }
        }
        let journal =
            self.read_journal_at(&self.directory().join(format!("{}.json", receipt.id)))?;
        if journal.id != receipt.id || (!journal.committed && !journal.rolled_back) {
            return Err(fail("用户修复归档收据无效"));
        }
        Ok(journal)
    }
    pub fn restore_with_lock(
        &self,
        receipt: &UserRepairReceipt,
        lock: &LifecycleLock,
    ) -> io::Result<()> {
        self.check_lock(lock)?;
        let journal = self.receipt_journal(receipt)?;
        #[cfg(all(test, target_os = "macos"))]
        if let Some(prefs) = test_preferences(self) {
            return self.restore_inner(&journal, lock, &|| Ok(()), &prefs);
        }
        self.restore_inner(
            &journal,
            lock,
            &|| crate::accounts::ensure_claude_stopped(self.owner),
            &NativePreferences,
        )
    }
    pub fn recover_with_lock(&self, lock: &LifecycleLock) -> io::Result<bool> {
        self.check_lock(lock)?;
        if !self.journal().try_exists()? {
            return Ok(false);
        }
        let journal = self.read_journal()?;
        if journal.committed || journal.rolled_back {
            self.finalize_record(&journal, lock)?;
            return Ok(true);
        }
        #[cfg(all(test, target_os = "macos"))]
        if let Some(prefs) = test_preferences(self) {
            self.restore_inner(&journal, lock, &|| Ok(()), &prefs)?;
            return Ok(true);
        }
        self.restore_inner(
            &journal,
            lock,
            &|| crate::accounts::ensure_claude_stopped(self.owner),
            &NativePreferences,
        )?;
        Ok(true)
    }
    pub fn commit_with_lock(
        &self,
        receipt: &UserRepairReceipt,
        lock: &LifecycleLock,
    ) -> io::Result<()> {
        #[cfg(all(test, target_os = "macos"))]
        if test_preferences(self).is_some() {
            return self.commit_inner(receipt, lock, &|| Ok(()));
        }
        self.commit_inner(receipt, lock, &|| {
            crate::accounts::ensure_claude_stopped(self.owner)
        })
    }
    /// Read-only receipt validation after an application restart. Does not commit or restore.
    pub(crate) fn verify_receipt_with_lock(
        &self,
        receipt: &UserRepairReceipt,
        lock: &LifecycleLock,
        restored: bool,
    ) -> io::Result<()> {
        self.check_lock(lock)?;
        let journal = self.receipt_journal(receipt)?;
        if restored != journal.rolled_back {
            return Err(fail("用户配置收据状态不一致"));
        }
        self.verify_images(&journal, restored)
    }
    fn verify_images(&self, journal: &Journal, restored: bool) -> io::Result<()> {
        if !restored && journal.after.len() != journal.changes.len() {
            return Err(fail("用户修复事务尚未完整验证"));
        }
        for (index, change) in journal.changes.iter().enumerate() {
            self.check_user_path(&self.path(change.kind))?;
            let current = snapshot(&self.path(change.kind), self.owner)?.0;
            if restored {
                // Unattempted changes keep the exact original image. Restored
                // copies can instead retain the exact recorded copy provenance.
                let saved_image = match (&current, &change.before, &change.backup_stamp) {
                    (Some(current), Some(_), Some(saved)) => same_contents(current, saved),
                    _ => false,
                };
                if !original_image(&current, &change.before) && !saved_image {
                    return Err(fail("恢复后用户配置已改变"));
                }
            } else {
                self.check_group_source(change)?;
                if current != journal.after[index] {
                    return Err(fail("用户配置在提交前变化，保留恢复事务"));
                }
            }
        }
        Ok(())
    }
    fn commit_inner(
        &self,
        receipt: &UserRepairReceipt,
        lock: &LifecycleLock,
        stopped: &dyn Fn() -> io::Result<()>,
    ) -> io::Result<()> {
        self.check_lock(lock)?;
        let mut journal = self.receipt_journal(receipt)?;
        if receipt.id != journal.id || journal.after.len() != journal.changes.len() {
            return Err(fail("用户修复事务尚未完整验证"));
        }
        if journal.rolled_back {
            return Err(fail("用户修复已恢复，不能重新提交"));
        }
        self.verify_images(&journal, false)?;
        if journal.committed {
            return self.finalize_record(&journal, lock);
        }
        stopped()?;
        journal.committed = true;
        // This single atomic durable replacement is the commit decision.
        // A crash after this point may only finalize records, never roll user settings back.
        storage::atomic_json(&self.journal(), &journal)?;
        self.finalize_record(&journal, lock)
    }
    fn finalize_record(&self, journal: &Journal, lock: &LifecycleLock) -> io::Result<()> {
        self.check_lock(lock)?;
        if (!journal.committed && !journal.rolled_back)
            || (journal.committed && journal.after.len() != journal.changes.len())
        {
            return Err(fail("用户修复提交决议不完整"));
        }
        storage::atomic_json(
            &self.directory().join(format!("{}.json", journal.id)),
            journal,
        )?;
        if self.journal().try_exists()? && self.read_journal()?.id == journal.id {
            fs::remove_file(self.journal())?;
        }
        File::open(self.directory())?.sync_all()
    }
    fn restore_inner(
        &self,
        journal: &Journal,
        lock: &LifecycleLock,
        stopped: &dyn Fn() -> io::Result<()>,
        prefs: &dyn PreferenceWriter,
    ) -> io::Result<()> {
        self.check_lock(lock)?;
        if journal.rolled_back {
            return self.finalize_record(journal, lock);
        }
        stopped()?;
        if journal.committed {
            return Err(fail("用户修复已提交，不允许按未提交事务回滚"));
        }
        for change in &journal.changes {
            self.check_user_path(&self.path(change.kind))?;
        }
        let backup = self.directory().join(&journal.id);
        check_directory(&backup, self.owner)?;
        // Validate every source before restoring any source. Unknown changes are never overwritten.
        for (index, change) in journal.changes.iter().take(journal.attempted).enumerate() {
            let (current, bytes) = snapshot(&self.path(change.kind), self.owner)?;
            if original_image(&current, &change.before) {
                continue;
            }
            if let Some(after) = journal.after.get(index) {
                if &current != after {
                    let original = read_original(&backup, index, change, self.owner)?;
                    if matches!(change.kind, Kind::Proxy | Kind::PinnedRoutes)
                        || !preferences::same_key(
                            original.as_deref(),
                            bytes.as_deref(),
                            change.kind,
                        )?
                    {
                        return Err(fail("用户配置已被外部修改，拒绝覆盖恢复"));
                    }
                }
            } else if !matches_target(change, bytes.as_deref())? {
                let original = read_original(&backup, index, change, self.owner)?;
                if matches!(change.kind, Kind::Proxy | Kind::PinnedRoutes)
                    || !preferences::same_key(original.as_deref(), bytes.as_deref(), change.kind)?
                {
                    return Err(fail("中断配置无法与目标关联，拒绝猜测恢复"));
                }
            }
            let original = read_original(&backup, index, change, self.owner)?;
            if !matches!(change.kind, Kind::Proxy | Kind::PinnedRoutes)
                && !preferences::same_except_key(
                    original.as_deref(),
                    bytes.as_deref(),
                    change.kind,
                )?
            {
                return Err(fail("偏好文件含事务以外变化，拒绝覆盖恢复"));
            }
        }
        for (index, change) in journal
            .changes
            .iter()
            .take(journal.attempted)
            .enumerate()
            .rev()
        {
            stopped()?;
            self.check_lock(lock)?;
            if original_image(
                &snapshot(&self.path(change.kind), self.owner)?.0,
                &change.before,
            ) {
                continue;
            }
            let original = read_original(&backup, index, change, self.owner)?;
            if !matches!(change.kind, Kind::Proxy | Kind::PinnedRoutes) {
                prefs.restore(change.kind, original.as_deref())?;
            }
            let path = self.path(change.kind);
            match &change.before {
                Some(before) => {
                    let before = change.backup_stamp.as_ref().unwrap_or(before);
                    let saved = backup.join(format!("original-{index}"));
                    let stage = stage_path(&path)?;
                    copy_file(&saved, &stage)?;
                    if !same_contents(
                        before,
                        &snapshot(&stage, self.owner)?
                            .0
                            .ok_or_else(|| fail("恢复暂存缺失"))?,
                    ) {
                        return Err(fail("恢复暂存元数据校验失败"));
                    }
                    fs::rename(&stage, &path)?;
                    File::open(path.parent().ok_or_else(|| fail("配置父目录缺失"))?)?.sync_all()?;
                    if !same_contents(
                        before,
                        &snapshot(&path, self.owner)?
                            .0
                            .ok_or_else(|| fail("恢复后配置缺失"))?,
                    ) {
                        return Err(fail("恢复后配置校验失败"));
                    }
                }
                None => {
                    if path.try_exists()? {
                        fs::remove_file(&path)?;
                        File::open(path.parent().ok_or_else(|| fail("配置父目录缺失"))?)?
                            .sync_all()?;
                    }
                }
            }
        }
        let mut restored = journal.clone();
        restored.rolled_back = true;
        storage::atomic_json(&self.journal(), &restored)?;
        self.finalize_record(&restored, lock)
    }
}
fn valid_receipt_id(id: &str) -> bool {
    id.len() <= 80
        && id.starts_with("repair-")
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase() || b == b'-')
}
fn item(
    kind: Kind,
    state: UserRepairState,
    original: &str,
    target: &str,
    message: &str,
) -> UserRepairItem {
    UserRepairItem {
        id: kind.id(),
        state,
        original: original.into(),
        target: target.into(),
        message: message.into(),
    }
}
fn original_image(current: &Option<Stamp>, before: &Option<Stamp>) -> bool {
    match (current, before) {
        (None, None) => true,
        (Some(a), Some(b)) => same_contents(a, b),
        _ => false,
    }
}
fn same_contents(a: &Stamp, b: &Stamp) -> bool {
    a.len == b.len
        && a.mode == b.mode
        && a.uid == b.uid
        && a.gid == b.gid
        && a.mtime == b.mtime
        && a.mtime_ns == b.mtime_ns
        && a.hash == b.hash
        && a.attributes == b.attributes
}
// Only an explicit cross-inode preference copy can add this system attribute.
// Comparing the filtered COPY against the FULL source digest also proves that
// the source had no provenance. Existing provenance must still match exactly.
fn copied_with_added_provenance(source: &Stamp, copy: &Stamp, filtered_copy: &str) -> bool {
    let mut normalized = copy.clone();
    normalized.attributes = filtered_copy.to_owned();
    (source.dev, source.ino) != (copy.dev, copy.ino)
        && source.attributes != copy.attributes
        && same_contents(source, &normalized)
}
fn check_directory(path: &Path, _owner: u32) -> io::Result<()> {
    if !fs::metadata(path)?.is_dir() {
        return Err(fail("配置父路径不是目录"));
    }
    Ok(())
}
fn snapshot(path: &Path, owner: u32) -> io::Result<(Option<Stamp>, Option<Vec<u8>>)> {
    let parent = path.parent().ok_or_else(|| fail("配置父目录缺失"))?;
    if parent.exists() {
        check_directory(parent, owner)?;
    }
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok((None, None)),
        Err(error) => return Err(error),
    };
    let m = file.metadata()?;
    if !m.is_file() {
        return Err(fail("配置路径不是普通文件"));
    }
    let mut bytes = Vec::new();
    (&file).read_to_end(&mut bytes)?;
    let attributes = metadata_digest(path)?;
    let after = file.metadata()?;
    let named = fs::metadata(path)?;
    if (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ) != (
        after.dev(),
        after.ino(),
        after.len(),
        after.mtime(),
        after.mtime_nsec(),
        after.ctime(),
        after.ctime_nsec(),
    ) || (m.dev(), m.ino()) != (named.dev(), named.ino())
    {
        return Err(fail("配置在读取期间变化"));
    }
    Ok((
        Some(Stamp {
            dev: m.dev(),
            ino: m.ino(),
            len: m.len(),
            mode: m.mode() & 0o7777,
            uid: m.uid(),
            gid: m.gid(),
            mtime: m.mtime(),
            mtime_ns: m.mtime_nsec(),
            ctime: m.ctime(),
            ctime_ns: m.ctime_nsec(),
            hash: crate::signed::digest(&bytes),
            attributes,
        }),
        Some(bytes),
    ))
}
fn read_original(
    backup: &Path,
    index: usize,
    change: &Change,
    owner: u32,
) -> io::Result<Option<Vec<u8>>> {
    match &change.before {
        None => Ok(None),
        Some(before) => {
            let (stamp, bytes) = snapshot(&backup.join(format!("original-{index}")), owner)?;
            if !stamp.as_ref().is_some_and(|value| {
                same_contents(value, change.backup_stamp.as_ref().unwrap_or(before))
            }) {
                return Err(fail("原配置备份损坏或元数据不匹配"));
            }
            Ok(bytes)
        }
    }
}
fn stage_path(path: &Path) -> io::Result<PathBuf> {
    let id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let stage = path
        .parent()
        .ok_or_else(|| fail("配置父目录缺失"))?
        .join(format!(".claude-switcher-{}-{id}", std::process::id()));
    if stage.try_exists()? {
        return Err(fail("配置暂存路径冲突"));
    }
    Ok(stage)
}
fn copy_file(source: &Path, target: &Path) -> io::Result<()> {
    if target.try_exists()? {
        return Err(fail("备份目标已经存在"));
    }
    let result = Command::new("/usr/bin/ditto")
        .args(["--rsrc", "--extattr", "--acl"])
        .arg(source)
        .arg(target)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .output()?;
    if !result.status.success() {
        return Err(fail("完整配置备份复制失败"));
    }
    File::open(target)?.sync_all()?;
    File::open(target.parent().ok_or_else(|| fail("备份父目录缺失"))?)?.sync_all()
}
fn replace_file(path: &Path, bytes: &[u8], owner: u32) -> io::Result<()> {
    replace_with_template(path, bytes, path, owner)
}
fn replace_with_template(path: &Path, bytes: &[u8], template: &Path, owner: u32) -> io::Result<()> {
    let before = snapshot(path, owner)?
        .0
        .ok_or_else(|| fail("原子替换源不存在"))?;
    let stage = stage_path(path)?;
    copy_file(template, &stage)?;
    let result = (|| {
        // Truncate only our private staged inode; ditto preserved its ACL/xattr/mode.
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&stage)?;
        std::io::Write::write_all(&mut file, bytes)?;
        file.sync_all()?;
        if snapshot(path, owner)?.0.as_ref() != Some(&before) {
            return Err(fail("代理配置在原子替换前变化"));
        }
        fs::rename(&stage, path)?;
        File::open(path.parent().ok_or_else(|| fail("代理目录缺失"))?)?.sync_all()
    })();
    if stage.try_exists().unwrap_or(false) {
        let _ = fs::remove_file(stage);
    }
    result
}
fn proxy_proposal(bytes: &[u8]) -> io::Result<(Vec<u8>, bool)> {
    let text = std::str::from_utf8(bytes).map_err(|_| fail("代理配置不是UTF-8"))?;
    let mut changed = false;
    let output = text
        .split_inclusive('\n')
        .map(|line| {
            if let Some(rest) = line.strip_prefix("enable_tun_mode: false") {
                changed = true;
                format!("enable_tun_mode: true{rest}")
            } else {
                line.to_owned()
            }
        })
        .collect::<String>();
    Ok((output.into_bytes(), changed))
}
fn matches_target(change: &Change, bytes: Option<&[u8]>) -> io::Result<bool> {
    match (change.kind, bytes) {
        (_, None) => Ok(false),
        (Kind::Proxy | Kind::PinnedRoutes, Some(bytes)) => Ok(bytes == change.target),
        (kind, Some(bytes)) => preferences::same_key(Some(bytes), Some(&change.target), kind),
    }
}
trait PreferenceWriter {
    fn set(&self, kind: Kind, target: &[u8]) -> io::Result<()>;
    fn restore(&self, kind: Kind, original: Option<&[u8]>) -> io::Result<()>;
}
struct NativePreferences;
impl PreferenceWriter for NativePreferences {
    fn set(&self, kind: Kind, target: &[u8]) -> io::Result<()> {
        preferences::validate_target(kind, target)?;
        preferences::write(kind, Some(target), true)
    }
    fn restore(&self, kind: Kind, original: Option<&[u8]>) -> io::Result<()> {
        preferences::write(kind, original, false)
    }
}

#[cfg(target_os = "macos")]
fn metadata_digest(path: &Path) -> io::Result<String> {
    metadata_digest_inner(path, false)
}
#[cfg(target_os = "macos")]
fn metadata_digest_without_provenance(path: &Path) -> io::Result<String> {
    metadata_digest_inner(path, true)
}
#[cfg(target_os = "macos")]
fn metadata_digest_inner(path: &Path, omit_provenance: bool) -> io::Result<String> {
    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| fail("配置路径无效"))?;
    let n =
        unsafe { libc::listxattr(path.as_ptr(), std::ptr::null_mut(), 0, libc::XATTR_NOFOLLOW) };
    if !(0..=16 * 1024 * 1024).contains(&n) {
        return Err(fail("配置扩展属性无法验证"));
    }
    let mut names = vec![0u8; n as usize];
    if n > 0
        && unsafe {
            libc::listxattr(
                path.as_ptr(),
                names.as_mut_ptr().cast(),
                names.len(),
                libc::XATTR_NOFOLLOW,
            )
        } != n
    {
        return Err(fail("配置扩展属性在读取期间变化"));
    }
    let mut attrs = std::collections::BTreeMap::new();
    let mut total = 0usize;
    for name in names.split(|b| *b == 0).filter(|name| !name.is_empty()) {
        let key = CString::new(name).map_err(|_| fail("扩展属性名称无效"))?;
        let len = unsafe {
            libc::getxattr(
                path.as_ptr(),
                key.as_ptr(),
                std::ptr::null_mut(),
                0,
                0,
                libc::XATTR_NOFOLLOW,
            )
        };
        if len < 0 || len as usize > 16 * 1024 * 1024 - total {
            return Err(fail("扩展属性过大或无法读取"));
        }
        total += len as usize;
        let mut value = vec![0; len as usize];
        if unsafe {
            libc::getxattr(
                path.as_ptr(),
                key.as_ptr(),
                value.as_mut_ptr().cast(),
                value.len(),
                0,
                libc::XATTR_NOFOLLOW,
            )
        } != len
        {
            return Err(fail("扩展属性读取不稳定"));
        }
        if !omit_provenance || name != b"com.apple.provenance" {
            attrs.insert(name.to_vec(), value);
        }
    }
    unsafe extern "C" {
        fn acl_get_link_np(path: *const c_char, kind: libc::c_int) -> *mut c_void;
        fn acl_free(object: *mut c_void) -> libc::c_int;
        fn acl_valid(acl: *mut c_void) -> libc::c_int;
        fn acl_get_entry(
            acl: *mut c_void,
            index: libc::c_int,
            entry: *mut *mut c_void,
        ) -> libc::c_int;
        fn acl_get_tag_type(entry: *mut c_void, tag: *mut libc::c_int) -> libc::c_int;
        fn acl_get_permset_mask_np(entry: *mut c_void, mask: *mut u64) -> libc::c_int;
        fn acl_to_text(acl: *mut c_void, length: *mut libc::ssize_t) -> *mut c_char;
    }
    let acl = unsafe { acl_get_link_np(path.as_ptr(), 0x100) };
    let mut acl_bytes = Vec::new();
    if acl.is_null() {
        if io::Error::last_os_error().raw_os_error() != Some(libc::ENOENT) {
            return Err(fail("配置ACL无法读取"));
        }
    } else {
        let result = (|| {
            if unsafe { acl_valid(acl) } != 0 {
                return Err(fail("配置ACL无效"));
            }
            let mut index = 0;
            loop {
                let mut entry = std::ptr::null_mut();
                if unsafe { acl_get_entry(acl, index, &mut entry) } != 0 {
                    if io::Error::last_os_error().raw_os_error() == Some(libc::EINVAL) {
                        break;
                    }
                    return Err(fail("配置ACL条目无法验证"));
                }
                index = -1;
                let mut tag = 0;
                let mut mask = 0;
                if unsafe { acl_get_tag_type(entry, &mut tag) } != 0
                    || unsafe { acl_get_permset_mask_np(entry, &mut mask) } != 0
                {
                    return Err(fail("配置ACL权限无法验证"));
                }
            }
            let mut len = 0;
            let text = unsafe { acl_to_text(acl, &mut len) };
            if text.is_null() || !(0..=65536).contains(&len) {
                return Err(fail("配置ACL序列化失败"));
            }
            acl_bytes =
                unsafe { std::slice::from_raw_parts(text.cast::<u8>(), len as usize) }.to_vec();
            unsafe {
                acl_free(text.cast());
            }
            Ok(())
        })();
        unsafe {
            acl_free(acl);
        }
        result?;
    }
    let bytes = serde_json::to_vec(&(attrs.into_iter().collect::<Vec<_>>(), acl_bytes))
        .map_err(io::Error::other)?;
    Ok(crate::signed::digest(&bytes))
}
#[cfg(not(target_os = "macos"))]
fn metadata_digest_without_provenance(_path: &Path) -> io::Result<String> {
    Err(fail("用户配置修复仅支持macOS"))
}
#[cfg(not(target_os = "macos"))]
fn metadata_digest(_path: &Path) -> io::Result<String> {
    Err(fail("用户配置修复仅支持macOS"))
}

#[cfg(target_os = "macos")]
mod preferences {
    use super::*;
    type Cf = *mut c_void;
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        static kCFPreferencesAnyApplication: Cf;
        static kCFPreferencesCurrentUser: Cf;
        static kCFPreferencesAnyHost: Cf;
        fn CFRelease(value: Cf);
        fn CFStringCreateWithCString(allocator: Cf, value: *const c_char, encoding: u32) -> Cf;
        fn CFStringGetCString(value: Cf, buffer: *mut c_char, size: isize, encoding: u32) -> u8;
        fn CFGetTypeID(value: Cf) -> usize;
        fn CFStringGetTypeID() -> usize;
        fn CFArrayGetTypeID() -> usize;
        fn CFArrayGetCount(value: Cf) -> isize;
        fn CFArrayGetValueAtIndex(value: Cf, index: isize) -> Cf;
        fn CFArrayCreate(
            allocator: Cf,
            values: *const Cf,
            count: isize,
            callbacks: *mut c_void,
        ) -> Cf;
        fn CFDataCreate(allocator: Cf, bytes: *const u8, length: isize) -> Cf;
        fn CFDataGetLength(value: Cf) -> isize;
        fn CFDataGetBytePtr(value: Cf) -> *const u8;
        fn CFPropertyListCreateWithData(
            allocator: Cf,
            data: Cf,
            options: usize,
            format: *mut isize,
            error: *mut Cf,
        ) -> Cf;
        fn CFPropertyListCreateData(
            allocator: Cf,
            value: Cf,
            format: isize,
            options: usize,
            error: *mut Cf,
        ) -> Cf;
        fn CFDictionaryGetTypeID() -> usize;
        fn CFDictionaryCreateMutable(
            allocator: Cf,
            capacity: isize,
            keys: *const c_void,
            values: *const c_void,
        ) -> Cf;
        fn CFDictionaryCreateMutableCopy(allocator: Cf, capacity: isize, dictionary: Cf) -> Cf;
        fn CFDictionaryGetValue(dictionary: Cf, key: Cf) -> Cf;
        fn CFDictionarySetValue(dictionary: Cf, key: Cf, value: Cf);
        fn CFDictionaryRemoveValue(dictionary: Cf, key: Cf);
        fn CFEqual(a: Cf, b: Cf) -> u8;
        fn CFPreferencesSetValue(key: Cf, value: Cf, application: Cf, user: Cf, host: Cf);
        fn CFPreferencesCopyValue(key: Cf, application: Cf, user: Cf, host: Cf) -> Cf;
        fn CFPreferencesCopyAppValue(key: Cf, application: Cf) -> Cf;
        fn CFPreferencesAppValueIsForced(key: Cf, application: Cf) -> u8;
        fn CFPreferencesSynchronize(application: Cf, user: Cf, host: Cf) -> u8;
    }
    struct Object(Cf);
    impl Drop for Object {
        fn drop(&mut self) {
            unsafe {
                CFRelease(self.0);
            }
        }
    }
    fn object(value: Cf) -> io::Result<Object> {
        if value.is_null() {
            Err(fail("原生偏好对象创建失败"))
        } else {
            Ok(Object(value))
        }
    }
    fn string(value: &str) -> io::Result<Object> {
        let bytes = CString::new(value).map_err(|_| fail("偏好字符串无效"))?;
        object(unsafe {
            CFStringCreateWithCString(std::ptr::null_mut(), bytes.as_ptr(), 0x08000100)
        })
    }
    fn dictionary(bytes: Option<&[u8]>) -> io::Result<Object> {
        let value = if let Some(bytes) = bytes {
            let data = object(unsafe {
                CFDataCreate(std::ptr::null_mut(), bytes.as_ptr(), bytes.len() as isize)
            })?;
            object(unsafe {
                CFPropertyListCreateWithData(
                    std::ptr::null_mut(),
                    data.0,
                    0,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            })?
        } else {
            object(unsafe {
                CFDictionaryCreateMutable(
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null(),
                    std::ptr::null(),
                )
            })?
        };
        if unsafe { CFGetTypeID(value.0) } != unsafe { CFDictionaryGetTypeID() } {
            return Err(fail("偏好根不是字典"));
        }
        Ok(value)
    }
    fn serialize(value: Cf) -> io::Result<Vec<u8>> {
        let data = object(unsafe {
            CFPropertyListCreateData(std::ptr::null_mut(), value, 200, 0, std::ptr::null_mut())
        })?;
        let len = unsafe { CFDataGetLength(data.0) };
        if len < 0 {
            return Err(fail("原生偏好序列化失败"));
        }
        Ok(unsafe { std::slice::from_raw_parts(CFDataGetBytePtr(data.0), len as usize) }.to_vec())
    }
    fn text(value: Cf) -> io::Result<String> {
        if value.is_null() || unsafe { CFGetTypeID(value) } != unsafe { CFStringGetTypeID() } {
            return Err(fail("偏好值不是字符串"));
        }
        let mut buffer = vec![0u8; 1024];
        if unsafe {
            CFStringGetCString(
                value,
                buffer.as_mut_ptr().cast(),
                buffer.len() as isize,
                0x08000100,
            )
        } == 0
        {
            return Err(fail("偏好字符串过长"));
        }
        Ok(unsafe { CStr::from_ptr(buffer.as_ptr().cast()) }
            .to_str()
            .map_err(|_| fail("偏好字符串不是UTF-8"))?
            .into())
    }
    fn describe(value: Cf, kind: Kind) -> io::Result<String> {
        if value.is_null() {
            return Ok("未设置".into());
        }
        match kind {
            Kind::Locale => {
                let value = text(value)?;
                if UserLocale::parse(&value).is_some() {
                    Ok(value)
                } else {
                    Ok("其它地区格式".into())
                }
            }
            Kind::Language => {
                if unsafe { CFGetTypeID(value) } != unsafe { CFArrayGetTypeID() } {
                    return Err(fail("客户端语言不是数组"));
                }
                if unsafe { CFArrayGetCount(value) } == 0 {
                    return Ok("未设置".into());
                }
                let first = text(unsafe { CFArrayGetValueAtIndex(value, 0) })?;
                // Never expose arbitrary application preference strings to UI or logs.
                if matches!(
                    first.as_str(),
                    "en" | "en-US"
                        | "en-GB"
                        | "ja"
                        | "ja-JP"
                        | "zh"
                        | "zh-CN"
                        | "zh-TW"
                        | "zh-HK"
                        | "ko"
                        | "ko-KR"
                        | "de"
                        | "de-DE"
                        | "fr"
                        | "fr-FR"
                        | "es"
                        | "es-ES"
                        | "pt"
                        | "pt-BR"
                        | "it"
                        | "it-IT"
                        | "ru"
                        | "ru-RU"
                ) {
                    Ok(first)
                } else {
                    Ok("其它语言".into())
                }
            }
            Kind::Proxy | Kind::PinnedRoutes => Err(fail("代理没有原生偏好键")),
        }
    }
    pub(super) fn language_acceptable(
        app: Option<&[u8]>,
        global: Option<&[u8]>,
    ) -> io::Result<bool> {
        let key = string("AppleLanguages")?;
        for (bytes, system) in [(app, false), (global, true)] {
            let dict = dictionary(bytes)?;
            let value = unsafe { CFDictionaryGetValue(dict.0, key.0) };
            if value.is_null() || unsafe { CFGetTypeID(value) } != unsafe { CFArrayGetTypeID() } {
                continue;
            }
            let count = unsafe { CFArrayGetCount(value) };
            for index in 0..count {
                let value = text(unsafe { CFArrayGetValueAtIndex(value, index) })?;
                if (!system && value.contains("en-US"))
                    || (system && index == 0 && value.starts_with("en"))
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    pub(super) fn propose(
        bytes: Option<&[u8]>,
        kind: Kind,
        target: &str,
    ) -> io::Result<(String, Vec<u8>, bool)> {
        let original = dictionary(bytes)?;
        let key = string(kind.key())?;
        let current = unsafe { CFDictionaryGetValue(original.0, key.0) };
        let label = describe(current, kind)?;
        let value = string(target)?;
        let array = if kind == Kind::Language {
            Some(object(unsafe {
                CFArrayCreate(std::ptr::null_mut(), &value.0, 1, std::ptr::null_mut())
            })?)
        } else {
            None
        };
        let desired = array.as_ref().map_or(value.0, |array| array.0);
        let changed = current.is_null() || unsafe { CFEqual(current, desired) } == 0;
        let mutable =
            object(unsafe { CFDictionaryCreateMutableCopy(std::ptr::null_mut(), 0, original.0) })?;
        unsafe {
            CFDictionarySetValue(mutable.0, key.0, desired);
        }
        Ok((label, serialize(mutable.0)?, changed))
    }
    pub(super) fn same_key(a: Option<&[u8]>, b: Option<&[u8]>, kind: Kind) -> io::Result<bool> {
        let a = dictionary(a)?;
        let b = dictionary(b)?;
        let key = string(kind.key())?;
        let av = unsafe { CFDictionaryGetValue(a.0, key.0) };
        let bv = unsafe { CFDictionaryGetValue(b.0, key.0) };
        Ok(if av.is_null() || bv.is_null() {
            av == bv
        } else {
            (unsafe { CFEqual(av, bv) }) != 0
        })
    }
    pub(super) fn same_except_key(
        a: Option<&[u8]>,
        b: Option<&[u8]>,
        kind: Kind,
    ) -> io::Result<bool> {
        let a = dictionary(a)?;
        let b = dictionary(b)?;
        let key = string(kind.key())?;
        let ac = object(unsafe { CFDictionaryCreateMutableCopy(std::ptr::null_mut(), 0, a.0) })?;
        let bc = object(unsafe { CFDictionaryCreateMutableCopy(std::ptr::null_mut(), 0, b.0) })?;
        unsafe {
            CFDictionaryRemoveValue(ac.0, key.0);
            CFDictionaryRemoveValue(bc.0, key.0);
        }
        Ok(unsafe { CFEqual(ac.0, bc.0) } != 0)
    }
    pub(super) fn validate_target(kind: Kind, bytes: &[u8]) -> io::Result<()> {
        let dictionary = dictionary(Some(bytes))?;
        let key = string(kind.key())?;
        let value = unsafe { CFDictionaryGetValue(dictionary.0, key.0) };
        let valid = match kind {
            Kind::Locale => UserLocale::parse(&text(value)?).is_some(),
            Kind::Language => {
                !value.is_null()
                    && unsafe { CFGetTypeID(value) } == unsafe { CFArrayGetTypeID() }
                    && unsafe { CFArrayGetCount(value) } == 1
                    && text(unsafe { CFArrayGetValueAtIndex(value, 0) })? == "en-US"
            }
            Kind::Proxy | Kind::PinnedRoutes => false,
        };
        if !valid {
            return Err(fail("修复目标不在固定枚举内"));
        }
        Ok(())
    }
    pub(super) fn write(kind: Kind, bytes: Option<&[u8]>, effective: bool) -> io::Result<()> {
        let source = dictionary(bytes)?;
        let key = string(kind.key())?;
        let value = unsafe { CFDictionaryGetValue(source.0, key.0) };
        let app = string("com.anthropic.claudefordesktop")?;
        let application = if kind == Kind::Locale {
            unsafe { kCFPreferencesAnyApplication }
        } else {
            app.0
        };
        // Global API uses exact AnyApplication / CurrentUser / AnyHost scope.
        // Client language uses exactly the domain read by the existing 14 checks.
        if effective
            && kind == Kind::Language
            && unsafe { CFPreferencesAppValueIsForced(key.0, app.0) } != 0
        {
            return Err(fail("客户端语言受管理策略控制，无法自动修复"));
        }
        unsafe {
            CFPreferencesSetValue(
                key.0,
                value,
                application,
                kCFPreferencesCurrentUser,
                kCFPreferencesAnyHost,
            );
        }
        if unsafe {
            CFPreferencesSynchronize(
                application,
                kCFPreferencesCurrentUser,
                kCFPreferencesAnyHost,
            )
        } == 0
        {
            return Err(fail("原生偏好同步失败；实际值未确认"));
        }
        let actual = unsafe {
            if effective && kind == Kind::Language && !value.is_null() {
                CFPreferencesCopyAppValue(key.0, app.0)
            } else {
                CFPreferencesCopyValue(
                    key.0,
                    application,
                    kCFPreferencesCurrentUser,
                    kCFPreferencesAnyHost,
                )
            }
        };
        let equal = if value.is_null() || actual.is_null() {
            value == actual
        } else {
            (unsafe { CFEqual(value, actual) }) != 0
        };
        if !actual.is_null() {
            unsafe {
                CFRelease(actual);
            }
        }
        if !equal {
            return Err(fail("原生偏好实际值未达到目标，保持等待并恢复"));
        }
        Ok(())
    }
}
#[cfg(not(target_os = "macos"))]
mod preferences {
    use super::*;
    pub(super) fn language_acceptable(_: Option<&[u8]>, _: Option<&[u8]>) -> io::Result<bool> {
        Err(fail("仅支持macOS"))
    }
    pub(super) fn propose(
        _: Option<&[u8]>,
        _: Kind,
        _: &str,
    ) -> io::Result<(String, Vec<u8>, bool)> {
        Err(fail("仅支持macOS"))
    }
    pub(super) fn same_key(_: Option<&[u8]>, _: Option<&[u8]>, _: Kind) -> io::Result<bool> {
        Err(fail("仅支持macOS"))
    }
    pub(super) fn same_except_key(_: Option<&[u8]>, _: Option<&[u8]>, _: Kind) -> io::Result<bool> {
        Err(fail("仅支持macOS"))
    }
    pub(super) fn validate_target(_: Kind, _: &[u8]) -> io::Result<()> {
        Err(fail("仅支持macOS"))
    }
    pub(super) fn write(_: Kind, _: Option<&[u8]>, _: bool) -> io::Result<()> {
        Err(fail("仅支持macOS"))
    }
}

#[cfg(all(test, target_os = "macos"))]
thread_local! {
    static TEST_HOME: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
    static TEST_FAIL_AT: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}
#[cfg(all(test, target_os = "macos"))]
fn test_preferences(coordinator: &UserRepairCoordinator) -> Option<tests::SyntheticPreferences> {
    TEST_HOME.with(|home| {
        (home.borrow().as_ref() == Some(&coordinator.home)).then(|| tests::SyntheticPreferences {
            home: coordinator.home.clone(),
            writes: std::cell::Cell::new(0),
            fail_at: TEST_FAIL_AT.with(std::cell::Cell::get),
        })
    })
}
#[cfg(all(test, target_os = "macos"))]
pub(crate) fn with_test_home<T>(home: &Path, fail_at: Option<usize>, run: impl FnOnce() -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            TEST_HOME.with(|value| *value.borrow_mut() = None);
            TEST_FAIL_AT.with(|value| value.set(None));
        }
    }
    TEST_HOME.with(|value| {
        assert!(
            value.borrow().is_none(),
            "test preference injection must not nest"
        );
        *value.borrow_mut() = Some(home.to_owned());
    });
    TEST_FAIL_AT.with(|value| value.set(fail_at));
    let _reset = Reset;
    run()
}
#[cfg(all(test, target_os = "macos"))]
pub(crate) mod tests {
    use super::*;
    use std::{cell::Cell, os::unix::fs::PermissionsExt};
    pub(crate) struct Fixture {
        pub(crate) coordinator: UserRepairCoordinator,
        pub(crate) root: PathBuf,
    }
    impl Fixture {
        pub(crate) fn new() -> Self {
            let root = PathBuf::from("/private/tmp").join(format!(
                "cs-user-repair-{}-{}-{}",
                std::process::id(),
                next_id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&root).unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            for directory in [
                "Library",
                "Library/Preferences",
                "Library/Application Support",
                "Library/Application Support/io.github.clash-verge-rev.clash-verge-rev",
                ".claude-switcher",
                ".claude-switcher/v2",
            ] {
                fs::create_dir(root.join(directory)).unwrap();
            }
            let coordinator = UserRepairCoordinator {
                home: root.clone(),
                owner: unsafe { libc::geteuid() },
            };
            fs::write(
                coordinator.path(Kind::Locale),
                plist(
                    "<key>AppleLocale</key><string>ja_JP</string><key>keep</key><data>AQID</data>",
                ),
            )
            .unwrap();
            fs::write(coordinator.path(Kind::Language),plist("<key>AppleLanguages</key><array><string>ja-JP</string></array><key>keep</key><date>2026-10-22T00:00:00Z</date>")).unwrap();
            fs::write(
                coordinator.path(Kind::Proxy),
                b"other: retained\nenable_tun_mode: false\n",
            )
            .unwrap();
            for kind in [Kind::Locale, Kind::Language, Kind::Proxy] {
                fs::set_permissions(coordinator.path(kind), fs::Permissions::from_mode(0o640))
                    .unwrap();
            }
            Self { coordinator, root }
        }
        pub(crate) fn target() -> UserRepairTarget {
            UserRepairTarget {
                locale: UserLocale::Us,
                proxy: ProxyRepairMode::ClashVerge,
            }
        }
        fn lock(&self) -> LifecycleLock {
            LifecycleLock::try_acquire(&self.coordinator.v2()).unwrap()
        }
        fn writer(&self) -> SyntheticPreferences {
            SyntheticPreferences {
                home: self.root.clone(),
                writes: Cell::new(0),
                fail_at: None,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn plist(body: &str) -> Vec<u8> {
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\"><plist version=\"1.0\"><dict>{body}</dict></plist>").into_bytes()
    }
    pub(super) struct SyntheticPreferences {
        pub(super) home: PathBuf,
        pub(super) writes: Cell<usize>,
        pub(super) fail_at: Option<usize>,
    }
    impl PreferenceWriter for SyntheticPreferences {
        fn set(&self, kind: Kind, target: &[u8]) -> io::Result<()> {
            self.writes.set(self.writes.get() + 1);
            if self.fail_at == Some(self.writes.get()) {
                return Err(fail("合成写入中断"));
            }
            let path = self.home.join(kind.relative());
            if path.try_exists()? {
                replace_file(&path, target, unsafe { libc::geteuid() })
            } else {
                storage::atomic_write(&path, target)
            }
        }
        fn restore(&self, kind: Kind, original: Option<&[u8]>) -> io::Result<()> {
            let path = self.home.join(kind.relative());
            let bytes = original.map_or_else(|| plist(""), Vec::from);
            if path.try_exists()? {
                replace_file(&path, &bytes, unsafe { libc::geteuid() })
            } else {
                storage::atomic_write(&path, &bytes)
            }
        }
    }
    #[test]
    fn preview_is_read_only_redacted_and_fixed_enums() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let before = snapshot(&c.path(Kind::Locale), c.owner).unwrap().0;
        let prepared = c.preview(Fixture::target()).unwrap();
        assert_eq!(prepared.changes.len(), 3);
        assert!(prepared.preview.can_execute);
        assert_eq!(prepared.preview.items[0].original, "ja_JP");
        assert_eq!(prepared.preview.items[0].target, "en_US");
        assert_eq!(prepared.preview.items[1].target, "en-US");
        assert!(!c.directory().exists());
        assert_eq!(before, snapshot(&c.path(Kind::Locale), c.owner).unwrap().0);
        assert!(UserLocale::parse("$(whoami)").is_none());
        assert!(ProxyRepairMode::parse("clash-verge;open").is_none());
        assert_eq!(
            prepared.preview.items.last().unwrap().state,
            UserRepairState::Unsupported
        );
    }
    #[test]
    fn unsupported_proxy_is_explicit_without_suppressing_safe_locale_items() {
        let fixture = Fixture::new();
        let prepared = fixture
            .coordinator
            .preview(UserRepairTarget {
                locale: UserLocale::Us,
                proxy: ProxyRepairMode::Surge,
            })
            .unwrap();
        assert_eq!(prepared.changes.len(), 2);
        assert_eq!(
            prepared.preview.items[2].state,
            UserRepairState::Unsupported
        );
        assert!(prepared.preview.items[2].message.contains("没有已验证"));
    }
    #[test]
    fn legacy_proxy_text_replaces_false_even_in_complex_documents() {
        let source = b"x: &alias\nenable_tun_mode: false # comment\n---\nenable_tun_mode: false\n  enable_tun_mode: false\n";
        let (output, changed) = proxy_proposal(source).unwrap();
        assert!(changed);
        assert_eq!(output, b"x: &alias\nenable_tun_mode: true # comment\n---\nenable_tun_mode: true\n  enable_tun_mode: false\n");
        assert!(!proxy_proposal(b"enable_tun_mode: true\n").unwrap().1);
        assert!(!proxy_proposal(b"enable_tun_mode: yes\n").unwrap().1);
    }
    #[test]
    fn legacy_language_skips_app_fallback_or_system_english_and_locale_is_independent() {
        for (app, system) in [
            ("(\n ja-JP,\n en-US\n)", "(\n ja-JP\n)"),
            ("(\n ja-JP\n)", "(\n en-GB,\n ja-JP\n)"),
        ] {
            let mut writes = Vec::new();
            let errors = repair_legacy_preferences_with(None, &mut |args| {
                if args[0] == "write" {
                    writes.push(args.join(" "));
                    return Ok(String::new());
                }
                Ok(if args[1] == "-g" { system } else { app }.into())
            });
            assert!(errors.is_empty());
            assert!(writes.is_empty());
        }
        let mut writes = Vec::new();
        assert!(
            repair_legacy_preferences_with(Some(UserLocale::Us), &mut |args| {
                if args[0] == "write" {
                    writes.push(args.join(" "));
                }
                Ok("ja_JP".into())
            })
            .is_empty()
        );
        assert_eq!(writes, ["write -g AppleLocale -string en_US"]);
    }
    #[test]
    fn f04_current_merge_with_missing_live_rules_requires_reload_without_writing() {
        let fixture = Fixture::new();
        let merge = enable_routes(&fixture);
        let current = pinned_routes::propose(None, "香港 节点").unwrap().0;
        fs::write(&merge, &current).unwrap();
        let entries = || {
            let mut names = fs::read_dir(merge.parent().unwrap())
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect::<Vec<_>>();
            names.sort();
            names
        };
        let before = entries();
        assert!(!repair_legacy_merge_at(&fixture.root, true).unwrap());
        assert_eq!(fs::read(&merge).unwrap(), current);
        assert_eq!(entries(), before);
        let needs_reload = repair_legacy_merge_at(&fixture.root, false).unwrap();
        assert_eq!(fs::read(&merge).unwrap(), current);
        assert_eq!(entries(), before, "an unchanged Merge must not gain a backup");
        assert!(needs_reload, "missing live rules require a reload even when Merge is current");
    }

    #[test]
    fn f04_current_legacy_config_with_missing_live_rules_requires_reload_without_writing() {
        let fixture = Fixture::new();
        let merge = enable_routes(&fixture);
        let current = b"# preserve user merge\nmixed-port: 7890\n";
        fs::write(&merge, current).unwrap();
        let config = fixture.root.join(pinned_routes::GROUP_RELATIVE);
        let mut bytes = fs::read(&config).unwrap();
        bytes.extend_from_slice("rules:\n  - DOMAIN-SUFFIX,claude.ai,香港 节点\n".as_bytes());
        fs::write(&config, &bytes).unwrap();
        let needs_reload = repair_legacy_merge_at(&fixture.root, false).unwrap();
        assert_eq!(fs::read(&config).unwrap(), bytes);
        assert_eq!(fs::read(&merge).unwrap(), current);
        assert_eq!(fs::read_dir(merge.parent().unwrap()).unwrap().count(), 1);
        assert!(needs_reload, "the legacy config shortcut still requires a live reload");
    }

    #[test]
    fn legacy_merge_uses_saved_group_creates_directory_and_preserves_complex_yaml() {
        let fixture = Fixture::new();
        let home = &fixture.root;
        fs::create_dir_all(home.join(".local/bin")).unwrap();
        fs::write(
            home.join(".local/bin/claude-switch.sh"),
            "PROXY_GROUP=\"Saved $Group\" # untouched literal\n",
        )
        .unwrap();
        assert!(repair_legacy_merge_at(home, false).unwrap());
        let merge = home.join(Kind::PinnedRoutes.relative());
        assert!(
            fs::read_to_string(&merge)
                .unwrap()
                .contains("claude.ai,Saved $Group")
        );
        let before = fs::read(&merge).unwrap();
        assert!(!repair_legacy_merge_at(home, true).unwrap());
        assert_eq!(fs::read(&merge).unwrap(), before);
        fs::write(
            home.join(pinned_routes::GROUP_RELATIVE),
            "rules:\n  - DOMAIN-SUFFIX,claude.ai,Saved $Group # old config\n",
        )
        .unwrap();
        assert!(!repair_legacy_merge_at(home, false).unwrap());
        assert_eq!(fs::read(&merge).unwrap(), before);
    }
    #[test]
    fn legacy_tun_quits_before_edit_and_restarts_without_touching_real_apps() {
        struct Runtime {
            path: PathBuf,
            events: Vec<&'static str>,
        }
        impl crate::proxy_repair::Runtime for Runtime {
            fn running(&mut self) -> Result<bool, &'static str> {
                Ok(true)
            }
            fn stop(&mut self) -> Result<(), &'static str> {
                assert!(fs::read_to_string(&self.path).unwrap().contains("false"));
                self.events.push("stop");
                Ok(())
            }
            fn start(&mut self) -> Result<(), &'static str> {
                assert!(fs::read_to_string(&self.path).unwrap().contains("true"));
                self.events.push("start");
                Ok(())
            }
            fn verify(&mut self, _: &str) -> Result<(), &'static str> {
                Ok(())
            }
        }
        let fixture = Fixture::new();
        let mut runtime = Runtime {
            path: fixture.coordinator.path(Kind::Proxy),
            events: vec![],
        };
        assert!(repair_legacy_tun_at(&fixture.root, &mut runtime).unwrap());
        assert_eq!(runtime.events, ["stop", "start"]);
        assert!(!repair_legacy_tun_at(&fixture.root, &mut runtime).unwrap());
    }
    fn enable_routes(fixture: &Fixture) -> PathBuf {
        let parent = fixture
            .coordinator
            .path(Kind::Proxy)
            .parent()
            .unwrap()
            .to_path_buf();
        fs::write(parent.join("clash-verge.yaml"),
            "proxy-groups:\n  - name: '香港 节点'\n    type: select\n    proxies:\n      - DIRECT\n").unwrap();
        let profiles = parent.join("profiles");
        fs::create_dir(&profiles).unwrap();
        profiles.join("Merge.yaml")
    }
    #[test]
    fn routes_create_preview_execute_restore_and_source_drift() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let merge = enable_routes(&fixture);
        let prepared = c.preview(Fixture::target()).unwrap();
        assert_eq!(prepared.changes.len(), 4);
        assert_eq!(
            prepared.preview.items.last().unwrap().state,
            UserRepairState::Ready
        );
        assert!(!merge.exists());
        let group = c
            .path(Kind::Proxy)
            .parent()
            .unwrap()
            .join("clash-verge.yaml");
        fs::write(
            &group,
            "proxy-groups:\n  - name: Changed\n    type: select\n",
        )
        .unwrap();
        assert!(
            c.execute_inner(prepared, &fixture.lock(), &|| Ok(()), &fixture.writer())
                .unwrap_err()
                .to_string()
                .contains("代理组配置已在预览后变化")
        );
        assert!(!c.journal().exists());
        fs::write(
            &group,
            "proxy-groups:\n  - name: '香港 节点'\n    type: select\n",
        )
        .unwrap();
        let prepared = c.preview(Fixture::target()).unwrap();
        let lock = fixture.lock();
        c.execute_inner(prepared, &lock, &|| Ok(()), &fixture.writer())
            .unwrap();
        let written = fs::read_to_string(&merge).unwrap();
        assert_eq!(written.matches("DOMAIN-SUFFIX,").count(), 13);
        assert!(written.contains("香港 节点"));
        assert_eq!(c.read_journal().unwrap().after.len(), 4);
        let journal = c.read_journal().unwrap();
        c.restore_inner(&journal, &lock, &|| Ok(()), &fixture.writer())
            .unwrap();
        assert!(!merge.exists());
    }
    #[test]
    fn routes_replacement_preserves_user_rules_and_recovers_full_original() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let merge = enable_routes(&fixture);
        let original = b"profile:\n  store-selected: true\nprepend-rules:\n  - DOMAIN-SUFFIX,user.example,DIRECT\n";
        fs::write(&merge, original).unwrap();
        let prepared = c.preview(Fixture::target()).unwrap();
        let lock = fixture.lock();
        c.execute_inner(prepared, &lock, &|| Ok(()), &fixture.writer())
            .unwrap();
        assert!(
            fs::read_to_string(&merge)
                .unwrap()
                .contains("DOMAIN-SUFFIX,user.example,DIRECT")
        );
        let journal = c.read_journal().unwrap();
        c.restore_inner(&journal, &lock, &|| Ok(()), &fixture.writer())
            .unwrap();
        assert_eq!(fs::read(&merge).unwrap(), original);
    }
    #[test]
    fn stale_preview_and_wrong_lock_never_write() {
        let fixture = Fixture::new();
        let other = Fixture::new();
        let c = &fixture.coordinator;
        let prepared = c.preview(Fixture::target()).unwrap();
        assert!(
            c.execute_inner(prepared, &other.lock(), &|| Ok(()), &fixture.writer())
                .is_err()
        );
        let prepared = c.preview(Fixture::target()).unwrap();
        fs::write(c.path(Kind::Proxy), b"enable_tun_mode: true\n").unwrap();
        assert!(
            c.execute_inner(prepared, &fixture.lock(), &|| Ok(()), &fixture.writer())
                .unwrap_err()
                .to_string()
                .contains("预览后变化")
        );
        assert!(!c.journal().exists());
    }
    #[test]
    fn stopped_gate_runs_before_backups_or_writes() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let prepared = c.preview(Fixture::target()).unwrap();
        let writer = fixture.writer();
        assert!(
            c.execute_inner(
                prepared,
                &fixture.lock(),
                &|| Err(fail("Claude仍在运行")),
                &writer
            )
            .is_err()
        );
        assert_eq!(writer.writes.get(), 0);
        assert!(!c.directory().exists());
    }
    #[test]
    fn provenance_copy_exception_is_directional_and_keeps_other_stamp_fields_strict() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let source = snapshot(&c.path(Kind::Language), c.owner)
            .unwrap()
            .0
            .unwrap();
        let mut copy = source.clone();
        copy.ino += 1;
        copy.attributes = "full-copy-with-added-provenance".into();
        assert!(copied_with_added_provenance(
            &source,
            &copy,
            &source.attributes
        ));
        assert!(!same_contents(&source, &copy));
        let mut changed_existing_provenance = copy.clone();
        changed_existing_provenance.ino += 1;
        changed_existing_provenance.attributes = "different-existing-provenance".into();
        assert!(!copied_with_added_provenance(
            &copy,
            &changed_existing_provenance,
            &source.attributes
        ));
        assert!(!copied_with_added_provenance(
            &copy,
            &source,
            &source.attributes
        ));
        assert!(!copied_with_added_provenance(
            &source,
            &copy,
            "changed-xattr-or-acl"
        ));
        for changed in [
            Stamp {
                len: copy.len + 1,
                ..copy.clone()
            },
            Stamp {
                mode: copy.mode ^ 0o100,
                ..copy.clone()
            },
            Stamp {
                uid: copy.uid + 1,
                ..copy.clone()
            },
            Stamp {
                gid: copy.gid + 1,
                ..copy.clone()
            },
            Stamp {
                mtime: copy.mtime + 1,
                ..copy.clone()
            },
            Stamp {
                mtime_ns: copy.mtime_ns + 1,
                ..copy.clone()
            },
            Stamp {
                hash: "changed-content".into(),
                ..copy.clone()
            },
            Stamp {
                ino: source.ino,
                ..copy.clone()
            },
        ] {
            assert!(!copied_with_added_provenance(
                &source,
                &changed,
                &source.attributes
            ));
        }
    }
    #[test]
    fn filtered_copy_digest_retains_quarantine_custom_xattrs_and_acl() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let path = c.path(Kind::Language);
        let before = snapshot(&path, c.owner).unwrap().0.unwrap();
        for name in ["com.apple.quarantine", "user.cs-copy-test"] {
            let raw_path = CString::new(path.as_os_str().as_bytes()).unwrap();
            let name = CString::new(name).unwrap();
            assert_eq!(
                unsafe {
                    libc::setxattr(
                        raw_path.as_ptr(),
                        name.as_ptr(),
                        b"test".as_ptr().cast(),
                        4,
                        0,
                        libc::XATTR_NOFOLLOW,
                    )
                },
                0
            );
            assert_ne!(
                metadata_digest_without_provenance(&path).unwrap(),
                before.attributes
            );
        }
        let attrs = metadata_digest_without_provenance(&path).unwrap();
        assert!(
            Command::new("/bin/chmod")
                .args(["+a", "everyone allow read"])
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        assert_ne!(metadata_digest_without_provenance(&path).unwrap(), attrs);
    }
    #[test]
    fn old_journal_missing_backup_stamp_keeps_strict_original_validation() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let prepared = c.preview(Fixture::target()).unwrap();
        let lock = fixture.lock();
        c.execute_inner(prepared, &lock, &|| Ok(()), &fixture.writer())
            .unwrap();
        let journal = c.read_journal().unwrap();
        let mut encoded = serde_json::to_value(&journal).unwrap();
        for change in encoded["changes"].as_array_mut().unwrap() {
            change.as_object_mut().unwrap().remove("backup_stamp");
        }
        let legacy: Journal = serde_json::from_value(encoded).unwrap();
        let backup = c.directory().join(&legacy.id);
        let mut change = legacy.changes[0].clone();
        assert!(change.backup_stamp.is_none());
        read_original(&backup, 0, &change, c.owner).unwrap();
        change.before.as_mut().unwrap().attributes = "different-original-metadata".into();
        assert!(read_original(&backup, 0, &change, c.owner).is_err());
    }
    #[test]
    fn restored_receipt_verifies_unattempted_original_and_legacy_journals_strictly() {
        for legacy in [false, true] {
            let fixture = Fixture::new();
            let c = &fixture.coordinator;
            let prepared = c.preview(Fixture::target()).unwrap();
            let lock = fixture.lock();
            let mut writer = fixture.writer();
            writer.fail_at = Some(1);
            assert!(
                c.execute_inner(prepared, &lock, &|| Ok(()), &writer)
                    .is_err()
            );
            let archive = fs::read_dir(c.directory())
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .find(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "json")
                })
                .unwrap();
            let mut journal: Journal = storage::read_json(&archive).unwrap();
            let receipt = UserRepairReceipt {
                id: journal.id.clone(),
            };
            assert!(journal.rolled_back);
            // A first-write failure can leave every original untouched, even
            // when its successful backup has different recorded provenance.
            for change in &mut journal.changes {
                if legacy {
                    change.backup_stamp = None;
                } else {
                    change.backup_stamp.as_mut().unwrap().attributes =
                        "copy-with-new-provenance".into();
                }
            }
            let archive = c.directory().join(format!("{}.json", receipt.id));
            storage::atomic_json(&archive, &journal).unwrap();
            c.verify_receipt_with_lock(&receipt, &lock, true).unwrap();
            journal.changes[0].before.as_mut().unwrap().attributes = "different-original".into();
            storage::atomic_json(&archive, &journal).unwrap();
            assert!(c.verify_receipt_with_lock(&receipt, &lock, true).is_err());
        }
    }
    #[test]
    fn restored_receipt_accepts_saved_real_copy_with_added_provenance() {
        let Some(source) = std::env::var_os("CS_REPAIR_READONLY_COPY_SOURCE") else {
            return;
        };
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let source = PathBuf::from(source);
        let (before, original) = snapshot(&source, c.owner).unwrap();
        let before = before.unwrap();
        let (_, target, _) =
            preferences::propose(original.as_deref(), Kind::Language, "en-US").unwrap();
        let id = "repair-provenance-receipt".to_string();
        storage::create_private_directory(&c.directory()).unwrap();
        let backup = c.directory().join(&id);
        storage::create_private_directory(&backup).unwrap();
        for directory in [&backup, c.path(Kind::Language).parent().unwrap()] {
            let path = CString::new(directory.as_os_str().as_bytes()).unwrap();
            assert_eq!(
                unsafe { libc::chown(path.as_ptr(), u32::MAX, before.gid) },
                0
            );
        }
        let saved = backup.join("original-0");
        copy_file(&source, &saved).unwrap();
        let copied = snapshot(&saved, c.owner).unwrap().0.unwrap();
        assert!(
            !same_contents(&before, &copied),
            "probe requires a real newly added provenance attribute"
        );
        assert!(copied_with_added_provenance(
            &before,
            &copied,
            &metadata_digest_without_provenance(&saved).unwrap()
        ));
        storage::atomic_write(&backup.join("target-0"), &target).unwrap();
        storage::atomic_write(&c.path(Kind::Language), &target).unwrap();
        let after = snapshot(&c.path(Kind::Language), c.owner).unwrap().0;
        let journal = Journal {
            schema: 1,
            id: id.clone(),
            changes: vec![Change {
                kind: Kind::Language,
                before: Some(before.clone()),
                backup_stamp: Some(copied),
                group_source: None,
                target_hash: crate::signed::digest(&target),
                target,
            }],
            attempted: 1,
            after: vec![after],
            committed: false,
            rolled_back: false,
        };
        storage::atomic_json(&c.journal(), &journal).unwrap();
        let lock = fixture.lock();
        c.restore_inner(&journal, &lock, &|| Ok(()), &fixture.writer())
            .unwrap();
        let receipt = UserRepairReceipt { id };
        // This is the exact predicate used by the old restored receipt branch.
        assert!(!original_image(
            &snapshot(&c.path(Kind::Language), c.owner).unwrap().0,
            &journal.changes[0].before
        ));
        c.verify_receipt_with_lock(&receipt, &lock, true).unwrap();
        let mut restored = c.receipt_journal(&receipt).unwrap();
        // The old schema cannot infer a new exception from the current file.
        restored.changes[0].backup_stamp = None;
        storage::atomic_json(
            &c.directory().join(format!("{}.json", receipt.id)),
            &restored,
        )
        .unwrap();
        assert!(c.verify_receipt_with_lock(&receipt, &lock, true).is_err());
        assert_eq!(snapshot(&source, c.owner).unwrap().0, Some(before));
    }
    // Opt-in evidence probe: copies and reads only; never modifies the supplied source.
    #[test]
    fn read_only_external_preference_copy_probe() {
        let Some(source) = std::env::var_os("CS_REPAIR_READONLY_COPY_SOURCE") else {
            return;
        };
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let source = PathBuf::from(source);
        let before = snapshot(&source, c.owner).unwrap().0.unwrap();
        // ditto inherits the destination directory group. Match only our private
        // probe directory to the source group, as the real home backup does.
        let probe_root = CString::new(fixture.root.as_os_str().as_bytes()).unwrap();
        assert_eq!(
            unsafe { libc::chown(probe_root.as_ptr(), u32::MAX, before.gid) },
            0
        );
        let saved = fixture.root.join("external-original");
        copy_file(&source, &saved).unwrap();
        let copied = snapshot(&saved, c.owner).unwrap().0.unwrap();
        assert!(
            same_contents(&before, &copied)
                || copied_with_added_provenance(
                    &before,
                    &copied,
                    &metadata_digest_without_provenance(&saved).unwrap()
                )
        );
        let change = Change {
            kind: Kind::Language,
            before: Some(before.clone()),
            backup_stamp: Some(copied.clone()),
            group_source: None,
            target: Vec::new(),
            target_hash: String::new(),
        };
        fs::rename(&saved, fixture.root.join("original-0")).unwrap();
        read_original(&fixture.root, 0, &change, c.owner).unwrap();
        let destination = fixture.root.join("isolated-preference");
        storage::atomic_write(&destination, b"isolated original").unwrap();
        replace_with_template(
            &destination,
            b"isolated replacement",
            &fixture.root.join("original-0"),
            c.owner,
        )
        .unwrap();
        let written = snapshot(&destination, c.owner).unwrap().0.unwrap();
        assert_eq!(written.attributes, copied.attributes);
        assert_eq!(
            (written.mode, written.uid, written.gid),
            (copied.mode, copied.uid, copied.gid)
        );
        let mut corrupted_record = change.clone();
        corrupted_record.backup_stamp.as_mut().unwrap().attributes =
            "changed-provenance-or-other-metadata".into();
        assert!(read_original(&fixture.root, 0, &corrupted_record, c.owner).is_err());
        let stage = fixture.root.join("restore-stage");
        copy_file(&fixture.root.join("original-0"), &stage).unwrap();
        assert!(same_contents(
            &copied,
            &snapshot(&stage, c.owner).unwrap().0.unwrap()
        ));
        assert_eq!(snapshot(&source, c.owner).unwrap().0, Some(before));
    }
    #[test]
    fn full_apply_and_restore_preserves_bytes_modes_xattrs_and_unrelated_plist_types() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let name = CString::new("user.cs-repair-test").unwrap();
        let path = CString::new(c.path(Kind::Locale).as_os_str().as_bytes()).unwrap();
        assert_eq!(
            unsafe {
                libc::setxattr(
                    path.as_ptr(),
                    name.as_ptr(),
                    b"meta".as_ptr().cast(),
                    4,
                    0,
                    libc::XATTR_NOFOLLOW,
                )
            },
            0
        );
        // A real isolated read-only extended ACL must survive ditto, atomic writes and rollback.
        assert!(
            Command::new("/bin/chmod")
                .args(["+a", "everyone allow read"])
                .arg(c.path(Kind::Locale))
                .status()
                .unwrap()
                .success()
        );
        let originals: [_; 3] = [Kind::Locale, Kind::Language, Kind::Proxy]
            .map(|kind| snapshot(&c.path(kind), c.owner).unwrap());
        let prepared = c.preview(Fixture::target()).unwrap();
        let lock = fixture.lock();
        let receipt = c
            .execute_inner(prepared, &lock, &|| Ok(()), &fixture.writer())
            .unwrap();
        let journal = c.read_journal().unwrap();
        assert_eq!(journal.after.len(), 3);
        assert_eq!(journal.id, receipt.id);
        for (index, change) in journal.changes.iter().enumerate() {
            let (after, bytes) = snapshot(&c.path(change.kind), c.owner).unwrap();
            assert!(matches_target(change, bytes.as_deref()).unwrap());
            assert_eq!(
                after.unwrap().attributes,
                originals[index].0.as_ref().unwrap().attributes
            );
            assert!(
                change.kind == Kind::Proxy
                    || preferences::same_except_key(
                        originals[index].1.as_deref(),
                        bytes.as_deref(),
                        change.kind
                    )
                    .unwrap()
            );
        }
        assert!(
            c.preview(Fixture::target())
                .unwrap()
                .preview
                .recovery_pending
        );
        c.restore_inner(&journal, &lock, &|| Ok(()), &fixture.writer())
            .unwrap();
        for (index, kind) in [Kind::Locale, Kind::Language, Kind::Proxy]
            .into_iter()
            .enumerate()
        {
            let restored = snapshot(&c.path(kind), c.owner).unwrap();
            assert_eq!(restored.1, originals[index].1);
            assert!(same_contents(
                restored.0.as_ref().unwrap(),
                originals[index].0.as_ref().unwrap()
            ));
        }
        assert!(!c.journal().exists());
    }
    #[test]
    fn failed_second_write_rolls_back_first_write() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let original = fs::read(c.path(Kind::Locale)).unwrap();
        let prepared = c.preview(Fixture::target()).unwrap();
        let mut writer = fixture.writer();
        writer.fail_at = Some(2);
        let error = c
            .execute_inner(prepared, &fixture.lock(), &|| Ok(()), &writer)
            .unwrap_err();
        assert!(error.to_string().contains("已恢复原值"));
        assert_eq!(fs::read(c.path(Kind::Locale)).unwrap(), original);
        assert!(!c.journal().exists());
    }
    #[test]
    fn crash_at_each_journal_checkpoint_restores_without_receipt_memory() {
        for attempted in 0..=3 {
            for saved_after in [false, true] {
                let fixture = Fixture::new();
                let c = &fixture.coordinator;
                let lock = fixture.lock();
                let originals: [_; 3] = [Kind::Locale, Kind::Language, Kind::Proxy]
                    .map(|kind| fs::read(c.path(kind)).unwrap());
                let prepared = c.preview(Fixture::target()).unwrap();
                let _receipt = c
                    .execute_inner(prepared, &lock, &|| Ok(()), &fixture.writer())
                    .unwrap();
                let mut journal = c.read_journal().unwrap();
                // Recreate a crash before/after the post-image receipt for the last attempted write.
                for (index, original) in originals.iter().enumerate().skip(attempted) {
                    let kind = journal.changes[index].kind;
                    replace_file(&c.path(kind), original, c.owner).unwrap();
                }
                journal.attempted = attempted;
                journal.after.truncate(if saved_after {
                    attempted
                } else {
                    attempted.saturating_sub(1)
                });
                storage::atomic_json(&c.journal(), &journal).unwrap();
                let reloaded = c.read_journal().unwrap();
                c.restore_inner(&reloaded, &lock, &|| Ok(()), &fixture.writer())
                    .unwrap();
                for (index, kind) in [Kind::Locale, Kind::Language, Kind::Proxy]
                    .into_iter()
                    .enumerate()
                {
                    assert_eq!(fs::read(c.path(kind)).unwrap(), originals[index]);
                }
            }
        }
    }
    #[test]
    fn recovery_rejects_unrelated_external_changes_and_corrupt_backup() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let lock = fixture.lock();
        c.execute_inner(
            c.preview(Fixture::target()).unwrap(),
            &lock,
            &|| Ok(()),
            &fixture.writer(),
        )
        .unwrap();
        let journal = c.read_journal().unwrap();
        fs::write(c.path(Kind::Language),plist("<key>AppleLanguages</key><array><string>en-US</string></array><key>new_external</key><true/>")).unwrap();
        let untouched = fs::read(c.path(Kind::Locale)).unwrap();
        assert!(
            c.restore_inner(&journal, &lock, &|| Ok(()), &fixture.writer())
                .is_err()
        );
        assert_eq!(fs::read(c.path(Kind::Locale)).unwrap(), untouched);
        assert!(c.journal().exists());
        let backup = c.directory().join(&journal.id).join("original-0");
        fs::write(backup, b"damaged").unwrap();
        assert!(
            read_original(
                &c.directory().join(&journal.id),
                0,
                &journal.changes[0],
                c.owner
            )
            .is_err()
        );
    }
    #[test]
    fn missing_client_preferences_restore_to_absent() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        fs::remove_file(c.path(Kind::Language)).unwrap();
        let lock = fixture.lock();
        c.execute_inner(
            c.preview(Fixture::target()).unwrap(),
            &lock,
            &|| Ok(()),
            &fixture.writer(),
        )
        .unwrap();
        let journal = c.read_journal().unwrap();
        c.restore_inner(&journal, &lock, &|| Ok(()), &fixture.writer())
            .unwrap();
        assert!(!c.path(Kind::Language).exists());
    }
    #[test]
    fn ordinary_large_user_file_with_write_acl_is_accepted() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let path = c.path(Kind::Proxy);
        let mut bytes = vec![b'#'; 2 * 1024 * 1024 + 1];
        bytes.extend_from_slice(b"\nenable_tun_mode: false\n");
        fs::write(&path, &bytes).unwrap();
        assert!(
            Command::new("/bin/chmod")
                .args(["+a", "everyone allow write"])
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(snapshot(&path, c.owner).unwrap().1.unwrap(), bytes);
        assert!(
            c.preview(Fixture::target())
                .unwrap()
                .changes
                .iter()
                .any(|c| c.kind == Kind::Proxy)
        );
    }
    #[test]
    fn ordinary_symlink_hardlink_and_world_writable_source_are_accepted() {
        for mode in 0..3 {
            let fixture = Fixture::new();
            let c = &fixture.coordinator;
            let path = c.path(Kind::Locale);
            match mode {
                0 => {
                    let other = c.home.join("outside");
                    fs::rename(&path, &other).unwrap();
                    std::os::unix::fs::symlink(other, &path).unwrap();
                }
                1 => fs::hard_link(&path, c.home.join("alias")).unwrap(),
                _ => fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap(),
            }
            assert!(c.preview(Fixture::target()).is_ok());
        }
    }
    #[test]
    fn recovery_is_idempotent_if_rollback_was_interrupted() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let lock = fixture.lock();
        c.execute_inner(
            c.preview(Fixture::target()).unwrap(),
            &lock,
            &|| Ok(()),
            &fixture.writer(),
        )
        .unwrap();
        let journal = c.read_journal().unwrap();
        let source = c.directory().join(&journal.id).join("original-2");
        let target = c.path(Kind::Proxy);
        let stage = stage_path(&target).unwrap();
        copy_file(&source, &stage).unwrap();
        fs::rename(stage, target).unwrap();
        c.restore_inner(&journal, &lock, &|| Ok(()), &fixture.writer())
            .unwrap();
        assert!(!c.journal().exists());
    }
    #[test]
    fn commit_retains_full_backup_receipt_and_rejects_post_apply_changes() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let lock = fixture.lock();
        let receipt = c
            .execute_inner(
                c.preview(Fixture::target()).unwrap(),
                &lock,
                &|| Ok(()),
                &fixture.writer(),
            )
            .unwrap();
        fs::write(c.path(Kind::Proxy), b"enable_tun_mode: false\n").unwrap();
        assert!(c.commit_inner(&receipt, &lock, &|| Ok(())).is_err());
        assert!(c.journal().exists());
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let lock = fixture.lock();
        let receipt = c
            .execute_inner(
                c.preview(Fixture::target()).unwrap(),
                &lock,
                &|| Ok(()),
                &fixture.writer(),
            )
            .unwrap();
        c.commit_inner(&receipt, &lock, &|| Ok(())).unwrap();
        assert!(!c.journal().exists());
        assert!(c.directory().join(format!("{}.json", receipt.id)).exists());
        assert!(c.directory().join(&receipt.id).join("original-0").exists());
    }
    #[test]
    fn writable_ancestor_is_accepted() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        fs::set_permissions(c.home.join("Library"), fs::Permissions::from_mode(0o777)).unwrap();
        assert!(c.preview(Fixture::target()).is_ok());
    }
    #[test]
    fn durable_commit_decision_survives_archive_cleanup_crash_without_rollback() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let lock = fixture.lock();
        c.execute_inner(
            c.preview(Fixture::target()).unwrap(),
            &lock,
            &|| Ok(()),
            &fixture.writer(),
        )
        .unwrap();
        let mut journal = c.read_journal().unwrap();
        journal.committed = true;
        storage::atomic_json(&c.journal(), &journal).unwrap();
        // A legitimate later change must survive finalization of an already committed transaction.
        fs::write(
            c.path(Kind::Proxy),
            b"enable_tun_mode: false\n# later user edit\n",
        )
        .unwrap();
        assert!(
            c.restore_inner(&journal, &lock, &|| Ok(()), &fixture.writer())
                .is_err()
        );
        assert!(c.recover_with_lock(&lock).unwrap());
        assert_eq!(
            fs::read(c.path(Kind::Proxy)).unwrap(),
            b"enable_tun_mode: false\n# later user edit\n"
        );
        assert!(!c.journal().exists());
        let archived: Journal =
            storage::read_json(&c.directory().join(format!("{}.json", journal.id))).unwrap();
        assert!(archived.committed);
    }
    #[test]
    fn large_preferences_keep_raw_targets_out_of_the_bounded_journal() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let large = plist(&format!(
            "<key>AppleLocale</key><string>ja_JP</string><key>unrelated</key><string>{}</string>",
            "x".repeat(100_000)
        ));
        fs::write(c.path(Kind::Locale), &large).unwrap();
        let lock = fixture.lock();
        c.execute_inner(
            c.preview(Fixture::target()).unwrap(),
            &lock,
            &|| Ok(()),
            &fixture.writer(),
        )
        .unwrap();
        assert!(fs::metadata(c.journal()).unwrap().len() < 65536);
        let journal = c.read_journal().unwrap();
        assert!(journal.changes[0].target.len() > 65536);
        c.restore_inner(&journal, &lock, &|| Ok(()), &fixture.writer())
            .unwrap();
        assert_eq!(fs::read(c.path(Kind::Locale)).unwrap(), large);
    }
    #[test]
    fn archived_commit_and_restore_are_idempotent_and_reject_arbitrary_ids() {
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let lock = fixture.lock();
        let receipt = c
            .execute_inner(
                c.preview(Fixture::target()).unwrap(),
                &lock,
                &|| Ok(()),
                &fixture.writer(),
            )
            .unwrap();
        assert_eq!(
            c.pending_receipt_with_lock(&lock).unwrap().unwrap().id,
            receipt.id
        );
        c.commit_inner(&receipt, &lock, &|| Ok(())).unwrap();
        assert!(c.pending_receipt_with_lock(&lock).unwrap().is_none());
        c.commit_inner(&receipt, &lock, &|| Err(fail("不应再次运行退出门禁")))
            .unwrap();
        assert!(c.restore_with_lock(&receipt, &lock).is_err());
        fs::write(
            c.path(Kind::Proxy),
            b"enable_tun_mode: false\n# external edit after commit\n",
        )
        .unwrap();
        assert!(c.commit_inner(&receipt, &lock, &|| Ok(())).is_err());
        assert_eq!(
            fs::read(c.path(Kind::Proxy)).unwrap(),
            b"enable_tun_mode: false\n# external edit after commit\n"
        );
        let forged: UserRepairReceipt = serde_json::from_str(r#"{"id":"../../outside"}"#).unwrap();
        assert!(c.receipt_journal(&forged).is_err());
        let fixture = Fixture::new();
        let c = &fixture.coordinator;
        let lock = fixture.lock();
        let receipt = c
            .execute_inner(
                c.preview(Fixture::target()).unwrap(),
                &lock,
                &|| Ok(()),
                &fixture.writer(),
            )
            .unwrap();
        let journal = c.read_journal().unwrap();
        c.restore_inner(&journal, &lock, &|| Ok(()), &fixture.writer())
            .unwrap();
        c.restore_with_lock(&receipt, &lock).unwrap();
        assert!(c.commit_inner(&receipt, &lock, &|| Ok(())).is_err());
        assert!(c.pending_receipt_with_lock(&lock).unwrap().is_none());
    }
}
