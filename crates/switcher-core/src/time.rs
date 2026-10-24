//! Local UTC continuity. No remote clock service or HTTP Date trust.
use serde::{Deserialize, Serialize};

pub const CLOCK_TOLERANCE_MS: i128 = 300_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockSample {
    pub utc_secs: i64,
    pub continuous_ms: u64,
    pub boot_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClockStatus {
    Trusted,
    NeedsRecovery { difference_secs: i64 },
    AwaitingVerifiedRefresh,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockState {
    pub status: ClockStatus,
    pub last_success_utc: Option<i64>,
    previous: Option<ClockSample>,
}

impl Default for ClockState {
    fn default() -> Self {
        Self {
            status: ClockStatus::Trusted,
            last_success_utc: None,
            previous: None,
        }
    }
}

impl ClockState {
    pub fn trusted(&self) -> bool {
        self.status == ClockStatus::Trusted
    }

    /// A manually established baseline permits a signed refresh, but still blocks installation.
    pub fn allows_verified_refresh(&self) -> bool {
        !matches!(self.status, ClockStatus::NeedsRecovery { .. })
    }

    pub fn observe(&mut self, sample: ClockSample) -> &ClockStatus {
        let mut difference_ms = 0i128;
        if let Some(previous) = &self.previous {
            if previous.boot_id == sample.boot_id {
                if sample.continuous_ms < previous.continuous_ms {
                    difference_ms = -CLOCK_TOLERANCE_MS - 1_000;
                } else {
                    difference_ms = (i128::from(sample.utc_secs) - i128::from(previous.utc_secs))
                        * 1_000
                        - i128::from(sample.continuous_ms - previous.continuous_ms);
                }
            } else if let Some(last_success) = self.last_success_utc {
                let rollback_ms = (i128::from(sample.utc_secs) - i128::from(last_success)) * 1_000;
                if rollback_ms < -CLOCK_TOLERANCE_MS {
                    difference_ms = rollback_ms;
                }
            }
        } else if let Some(last_success) = self.last_success_utc {
            let rollback_ms = (i128::from(sample.utc_secs) - i128::from(last_success)) * 1_000;
            if rollback_ms < -CLOCK_TOLERANCE_MS {
                difference_ms = rollback_ms;
            }
        }
        // Repeated small rollback steps must not evade the last verified UTC anchor.
        // A user-established baseline can replace that anchor only after a signed refresh.
        if self.status != ClockStatus::AwaitingVerifiedRefresh
            && let Some(last_success) = self.last_success_utc
        {
            let rollback_ms = (i128::from(sample.utc_secs) - i128::from(last_success)) * 1_000;
            if rollback_ms < -CLOCK_TOLERANCE_MS {
                difference_ms = rollback_ms;
            }
        }
        if difference_ms.abs() > CLOCK_TOLERANCE_MS {
            self.status = ClockStatus::NeedsRecovery {
                difference_secs: (difference_ms / 1_000)
                    .clamp(i128::from(i64::MIN), i128::from(i64::MAX))
                    as i64,
            };
        }
        self.previous = Some(sample);
        &self.status
    }

    /// Display reads keep a stable baseline while still checking cumulative clock drift.
    pub fn observe_passive(&mut self, sample: ClockSample) -> &ClockStatus {
        let previous = self.previous.clone();
        let status = self.status.clone();
        let same_boot = previous
            .as_ref()
            .is_some_and(|value| value.boot_id == sample.boot_id);
        self.observe(sample);
        if same_boot && self.status == status {
            self.previous = previous;
        }
        &self.status
    }

    /// Explicit local action after the user fixes system time. This alone does not resume anything.
    pub fn establish_baseline(&mut self, sample: ClockSample) {
        self.previous = Some(sample);
        self.status = ClockStatus::AwaitingVerifiedRefresh;
    }

    /// Call only after the fixed source's signature, sequence and validity pass at this sample's UTC.
    /// Replay counters live outside ClockState and are deliberately not reset here.
    pub fn record_verified_success(&mut self, sample: ClockSample) -> Result<(), &'static str> {
        self.observe(sample.clone());
        if matches!(self.status, ClockStatus::NeedsRecovery { .. }) {
            return Err("clock_recovery_required");
        }
        self.status = ClockStatus::Trusted;
        self.last_success_utc = Some(sample.utc_secs);
        Ok(())
    }
}

/// macOS's continuous clock includes sleep; Instant is intentionally not used here.
#[cfg(target_os = "macos")]
pub fn sample() -> Result<ClockSample, &'static str> {
    #[repr(C)]
    struct Timebase {
        numerator: u32,
        denominator: u32,
    }
    unsafe extern "C" {
        fn mach_continuous_time() -> u64;
        fn mach_timebase_info(info: *mut Timebase) -> i32;
    }
    let mut timebase = Timebase {
        numerator: 0,
        denominator: 0,
    };
    // SAFETY: the system writes exactly one properly aligned Timebase.
    if unsafe { mach_timebase_info(&mut timebase) } != 0 || timebase.denominator == 0 {
        return Err("continuous_clock");
    }
    // SAFETY: mach_continuous_time has no arguments or memory side effects.
    let ticks = unsafe { mach_continuous_time() };
    let continuous_ms = u128::from(ticks) * u128::from(timebase.numerator)
        / u128::from(timebase.denominator)
        / 1_000_000;
    let boot = std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "kern.bootsessionuuid"])
        .output()
        .map_err(|_| "boot_id")?;
    if !boot.status.success() {
        return Err("boot_id");
    }
    let boot_id = String::from_utf8(boot.stdout)
        .map_err(|_| "boot_id")?
        .trim()
        .to_owned();
    if boot_id.len() != 36 || !boot_id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        return Err("boot_id");
    }
    let utc_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "utc_before_epoch")?
        .as_secs()
        .try_into()
        .map_err(|_| "utc_overflow")?;
    Ok(ClockSample {
        utc_secs,
        continuous_ms: continuous_ms
            .try_into()
            .map_err(|_| "continuous_overflow")?,
        boot_id,
    })
}

#[cfg(not(target_os = "macos"))]
pub fn sample() -> Result<ClockSample, &'static str> {
    Err("unsupported_continuous_clock")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn at(utc: i64, elapsed: u64, boot: &str) -> ClockSample {
        ClockSample {
            utc_secs: utc,
            continuous_ms: elapsed,
            boot_id: boot.into(),
        }
    }
    #[test]
    fn passive_observation_keeps_normal_baseline_and_detects_cumulative_drift() {
        let mut clock = ClockState::default();
        clock.observe_passive(at(1_000, 10_000, "a"));
        let initial = serde_json::to_vec(&clock).unwrap();
        for offset in [1, 60, 3_600] {
            clock.observe_passive(at(1_000 + offset, 10_000 + offset as u64 * 1_000, "a"));
            assert_eq!(serde_json::to_vec(&clock).unwrap(), initial);
        }
        clock.observe_passive(at(900, 11_000, "a"));
        assert!(clock.trusted());
        clock.observe_passive(at(800, 12_000, "a"));
        assert!(clock.trusted());
        clock.observe_passive(at(700, 13_000, "a"));
        assert!(!clock.trusted());
        assert_eq!(clock.previous, Some(at(700, 13_000, "a")));
    }

    #[test]
    fn passive_first_boot_forward_and_continuous_rollback_checkpoint() {
        let mut clock = ClockState::default();
        clock.observe_passive(at(1_000, 10_000, "a"));
        assert_eq!(clock.previous, Some(at(1_000, 10_000, "a")));
        clock.observe_passive(at(1_001, 100, "b"));
        assert_eq!(clock.previous, Some(at(1_001, 100, "b")));
        clock.observe_passive(at(1_700, 1_100, "b"));
        assert!(!clock.trusted());
        clock.establish_baseline(at(1_700, 10_000, "b"));
        clock.observe_passive(at(1_701, 100, "b"));
        assert!(!clock.trusted());
    }

    #[test]
    fn passive_anomaly_is_sticky_after_reload_and_baseline_requires_verification() {
        let mut clock = ClockState::default();
        clock
            .record_verified_success(at(1_000, 10_000, "a"))
            .unwrap();
        clock.observe_passive(at(600, 11_000, "a"));
        let mut restored: ClockState =
            serde_json::from_slice(&serde_json::to_vec(&clock).unwrap()).unwrap();
        restored.observe_passive(at(601, 12_000, "a"));
        assert!(!restored.trusted());
        assert_eq!(restored.last_success_utc, Some(1_000));
        restored.establish_baseline(at(1_001, 13_000, "a"));
        restored.observe_passive(at(1_002, 14_000, "a"));
        assert_eq!(restored.status, ClockStatus::AwaitingVerifiedRefresh);
        assert_eq!(restored.last_success_utc, Some(1_000));
        restored
            .record_verified_success(at(1_003, 15_000, "a"))
            .unwrap();
        assert!(restored.trusted());
    }

    #[test]
    fn sleep_utc_timezone_and_recovery_do_not_reset_authenticated_history() {
        let mut clock = ClockState::default();
        clock
            .record_verified_success(at(10_000, 0, "boot-a"))
            .unwrap();
        clock.observe(at(13_600, 3_600_000, "boot-a")); // One-hour sleep; UTC and continuous time agree.
        assert!(clock.trusted());
        clock.observe(at(13_601, 3_601_000, "boot-a")); // Timezone/DST never enter the UTC sample.
        assert!(clock.trusted());
        clock.observe(at(10_000, 3_602_000, "boot-a"));
        assert!(matches!(clock.status, ClockStatus::NeedsRecovery { .. }));
        assert!(
            clock
                .record_verified_success(at(10_001, 3_603_000, "boot-a"))
                .is_err()
        );
        assert_eq!(clock.last_success_utc, Some(10_000));
        clock.establish_baseline(at(13_603, 3_603_000, "boot-a"));
        assert!(!clock.trusted());
        clock
            .record_verified_success(at(13_604, 3_604_000, "boot-a"))
            .unwrap();
        assert!(clock.trusted());
        let saved = serde_json::to_vec(&clock).unwrap();
        let mut restarted: ClockState = serde_json::from_slice(&saved).unwrap();
        restarted.observe(at(13_000, 100, "boot-b"));
        assert!(matches!(
            restarted.status,
            ClockStatus::NeedsRecovery { .. }
        ));
        assert_eq!(restarted.last_success_utc, Some(13_604));
    }
    #[test]
    fn forward_jump_and_monotonic_rollback_require_recovery() {
        let mut clock = ClockState::default();
        clock.observe(at(1_000, 10_000, "a"));
        clock.observe(at(1_600, 11_000, "a"));
        assert!(!clock.trusted());
        clock.establish_baseline(at(1_600, 11_000, "a"));
        clock
            .record_verified_success(at(1_601, 12_000, "a"))
            .unwrap();
        clock.observe(at(1_602, 100, "a"));
        assert!(!clock.trusted());
    }
    #[test]
    fn repeated_small_rollbacks_cannot_cross_the_verified_anchor() {
        let mut clock = ClockState::default();
        clock.record_verified_success(at(1_000, 0, "a")).unwrap();
        clock.observe(at(800, 1_000, "a"));
        assert!(clock.trusted());
        clock.observe(at(600, 2_000, "a"));
        assert!(!clock.trusted());
        assert_eq!(clock.last_success_utc, Some(1_000));
        clock.establish_baseline(at(600, 2_000, "a"));
        clock.record_verified_success(at(601, 3_000, "a")).unwrap();
        assert_eq!(clock.last_success_utc, Some(601));
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn real_macos_continuous_sample_is_available() {
        let sample = sample().unwrap();
        assert!(sample.utc_secs > 1_700_000_000);
        assert!(!sample.boot_id.is_empty());
    }
}
