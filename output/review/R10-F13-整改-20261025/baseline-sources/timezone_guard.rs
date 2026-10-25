//! Independent, hourly reconciliation of an administrator-authorized timezone.
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const INTERVAL: Duration = Duration::from_secs(3600);
const RETRY_INTERVAL: Duration = Duration::from_secs(5);

/// The blocking XPC call has exactly one slot. A late call never creates a second writer.
#[derive(Default)]
struct Scheduler {
    flight: Option<tokio::task::JoinHandle<Result<(), &'static str>>>,
    next_attempt: Option<Instant>,
}

impl Scheduler {
    async fn tick_at<F>(&mut self, now: Instant, operation: F)
    where
        F: FnOnce() -> Result<(), &'static str> + Send + 'static,
    {
        if self
            .flight
            .as_ref()
            .is_some_and(|flight| !flight.is_finished())
        {
            return;
        }
        if let Some(flight) = self.flight.take() {
            let delay = match flight.await {
                Ok(Err("timezone_guard_lifecycle_busy" | "timezone_guard_transaction_pending")) => {
                    RETRY_INTERVAL
                }
                _ => INTERVAL,
            };
            self.next_attempt = Some(now + delay);
        }
        if self.next_attempt.is_some_and(|next| now < next) {
            return;
        }
        self.flight = Some(tokio::task::spawn_blocking(operation));
    }
}

/// Read the configured target without waiting for unrelated lifecycle transactions.
fn reconcile_with<F>(v2: &Path, call: F) -> Result<(), &'static str>
where
    F: FnOnce(String) -> Result<(), &'static str>,
{
    crate::storage::validate_no_symlinks(v2).map_err(|_| "timezone_guard_configuration")?;
    let bytes =
        std::fs::read(v2.join("config.json")).map_err(|_| "timezone_guard_policy_missing")?;
    let config: crate::config::CheckConfig =
        serde_json::from_slice(&bytes).map_err(|_| "timezone_guard_policy_invalid")?;
    let expected = config
        .timezone_target
        .filter(|zone| crate::config::valid_timezone(zone))
        .ok_or("timezone_guard_policy_missing")?;
    call(expected)
}

#[cfg(target_os = "macos")]
fn reconcile(v2: &Path) -> Result<(), &'static str> {
    reconcile_with(v2, |expected| {
        let current = std::fs::read_link("/etc/localtime").ok();
        if current.as_deref()
            == Some(
                Path::new("/var/db/timezone/zoneinfo")
                    .join(&expected)
                    .as_path(),
            )
        {
            return Ok(());
        }
        let status = std::process::Command::new("/usr/sbin/systemsetup")
            .args(["-settimezone", &expected])
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .status()
            .map_err(|_| "timezone_guard_command")?;
        if status.success() {
            Ok(())
        } else {
            Err("timezone_guard_command")
        }
    })
}

/// Poll immediately for RunAtLoad, then dispatch only when the result-dependent delay expires.
pub(crate) fn spawn(
    state_path: &Path,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<tokio::task::JoinHandle<()>, &'static str> {
    let v2 = state_path
        .parent()
        .ok_or("guardian_state_directory")?
        .to_path_buf();
    Ok(tokio::spawn(async move {
        let mut ticker = tokio::time::interval(RETRY_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut scheduler = Scheduler::default();
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() { return; }
                }
                scheduled = ticker.tick() => {
                    let directory: PathBuf = v2.clone();
                    scheduler.tick_at(scheduled.into_std(), move || {
                        #[cfg(target_os = "macos")]
                        {
                            let result = reconcile(&directory);
                            if let Err(code) = result {
                                eprintln!("timezone_guard={code}");
                            }
                            result
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
                            let _ = directory;
                            Ok(())
                        }
                    }).await;
                }
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "legacy-tz-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            crate::storage::create_private_directory(&root).unwrap();
            let config = crate::config::CheckConfig {
                timezone_target: Some("Asia/Tokyo".into()),
                ..Default::default()
            };
            crate::storage::atomic_json(&root.join("config.json"), &config).unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn startup_reconciles_target_despite_busy_lock_and_pending_repair() {
        let fixture = Fixture::new();
        let _lock = crate::storage::LifecycleLock::try_acquire(&fixture.0).unwrap();
        crate::storage::atomic_json(&fixture.0.join("region-repair.json"), &"pending").unwrap();
        let mut called = false;
        reconcile_with(&fixture.0, |zone| {
            assert_eq!(zone, "Asia/Tokyo");
            called = true;
            Ok(())
        })
        .unwrap();
        assert!(called);
    }
    #[test]
    fn invalid_target_never_invokes_command() {
        let fixture = Fixture::new();
        std::fs::write(
            fixture.0.join("config.json"),
            r#"{"country_target":null,"timezone_target":"../invalid","locale_target":null}"#,
        )
        .unwrap();
        assert!(reconcile_with(&fixture.0, |_| panic!("invalid timezone")).is_err());
    }
    #[tokio::test]
    async fn startup_then_hourly_schedule_is_preserved() {
        let mut scheduler = Scheduler::default();
        let now = Instant::now();
        scheduler.tick_at(now, || Ok(())).await;
        assert!(scheduler.flight.is_some());
        while !scheduler.flight.as_ref().unwrap().is_finished() {
            tokio::task::yield_now().await;
        }
        scheduler.tick_at(now, || panic!("hourly delay")).await;
        scheduler
            .tick_at(now + INTERVAL - Duration::from_secs(1), || {
                panic!("too early")
            })
            .await;
        assert!(scheduler.flight.is_none());
        scheduler.tick_at(now + INTERVAL, || Ok(())).await;
        assert!(scheduler.flight.is_some());
        scheduler.flight.take().unwrap().await.unwrap().unwrap();
    }
}
