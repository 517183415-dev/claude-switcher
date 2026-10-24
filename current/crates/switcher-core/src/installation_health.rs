//! Installation-only liveness. This does not grant normal business health or acquire a lifecycle lock.
use switcher_domain::{
    Edition,
    system::{InstallHealthChallenge, InstallRecord},
};

#[cfg(any(target_os = "macos", test))]
fn validate_context(
    challenge: &InstallHealthChallenge,
    record: &InstallRecord,
    edition: Edition,
    uid: u32,
    boot: &str,
    now: u64,
) -> Result<(), &'static str> {
    if record.owner_uid != uid || record.edition != edition {
        return Err("installation_health_context");
    }
    challenge.validate_at(record, boot, now)
}

#[cfg(target_os = "macos")]
fn load_context(edition: Option<Edition>) -> Result<InstallHealthChallenge, &'static str> {
    use std::path::Path;
    use switcher_domain::system::INSTALL_RECORD;
    let record = switcher_helper::platform::load_root_record(Path::new(INSTALL_RECORD))?;
    let challenge = switcher_helper::installation_health::load_challenge()?;
    let boot = switcher_helper::installation_health::boot_session_uuid()?;
    validate_context(
        &challenge,
        &record,
        edition.unwrap_or(record.edition),
        unsafe { libc::geteuid() },
        &boot,
        crate::checks::unix_now(),
    )?;
    Ok(challenge)
}

#[cfg(target_os = "macos")]
fn report(
    challenge: InstallHealthChallenge,
    role: switcher_domain::system::ClientRole,
    loop_sequence: u64,
) -> Result<(), &'static str> {
    use switcher_domain::system::HelperAction;
    // connect performs the real BeginSession on this authenticated connection.
    let mut client = switcher_helper::xpc::NativeClient::connect(&challenge.record, role)?;
    let response = client.call(HelperAction::InstallationHealth {
        generation: challenge.generation,
        nonce: challenge.nonce,
        loop_sequence,
    })?;
    if response.ok {
        Ok(())
    } else {
        Err("installation_health_rejected")
    }
}

/// A fixed GUI-role probe. stdout from the caller is diagnostic, never a commit receipt.
pub fn gui_probe(edition: Edition, generation: &str, nonce: &str) -> Result<(), &'static str> {
    #[cfg(target_os = "macos")]
    {
        let challenge = load_context(Some(edition))?;
        if challenge.generation != generation || challenge.nonce != nonce {
            return Err("installation_health_challenge_mismatch");
        }
        report(challenge, switcher_domain::system::ClientRole::Gui, 0)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (edition, generation, nonce);
        Err("macos_required")
    }
}

#[cfg(target_os = "macos")]
fn guardian_challenge(loop_sequence: u64) -> Result<InstallHealthChallenge, &'static str> {
    use std::{os::unix::fs::MetadataExt, path::Path};
    use switcher_domain::system::SOURCE_LOCAL_TRANSACTION;
    if loop_sequence == 0 {
        return Err("installation_health_loop_sequence");
    }
    let pending = std::fs::symlink_metadata(Path::new(SOURCE_LOCAL_TRANSACTION))
        .map_err(|_| "installation_health_not_pending")?;
    if !pending.is_file()
        || pending.uid() != 0
        || pending.mode() & 0o7777 != 0o600
        || pending.nlink() != 1
    {
        return Err("installation_health_pending_permissions");
    }
    load_context(None)
}

/// A fixed Guardian-role probe. Only the driver's actual timer tick supplies this sequence.
pub fn guardian_tick(loop_sequence: u64) -> Result<(), &'static str> {
    if loop_sequence == 0 {
        return Err("installation_health_loop_sequence");
    }
    #[cfg(target_os = "macos")]
    {
        report(
            guardian_challenge(loop_sequence)?,
            switcher_domain::system::ClientRole::Guardian,
            loop_sequence,
        )
    }
    #[cfg(not(target_os = "macos"))]
    Err("macos_required")
}

/// At most one bounded XPC call may be in flight. A slow helper cannot block protection ticks.
#[derive(Default)]
pub(crate) struct GuardianProbe {
    flight: Option<tokio::task::JoinHandle<()>>,
    next_attempt: Option<std::time::Instant>,
}
impl GuardianProbe {
    pub(crate) fn tick(&mut self, sequence: u64) {
        self.tick_at(sequence, std::time::Instant::now(), move |sequence| {
            #[cfg(target_os = "macos")]
            {
                // A successful response only acknowledges one role. Keep reporting while
                // pending, so a restarted helper can collect both roles again.
                let _ = guardian_tick(sequence);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = sequence;
        });
    }
    fn tick_at<F>(&mut self, sequence: u64, now: std::time::Instant, report: F)
    where
        F: FnOnce(u64) + Send + 'static,
    {
        if self.next_attempt.is_some_and(|next| now < next) {
            return;
        }
        self.next_attempt = Some(now + std::time::Duration::from_secs(10));
        self.tick_with(sequence, report);
    }
    fn tick_with<F>(&mut self, sequence: u64, report: F)
    where
        F: FnOnce(u64) + Send + 'static,
    {
        if self
            .flight
            .as_ref()
            .is_some_and(|flight| !flight.is_finished())
        {
            return;
        }
        self.flight = Some(tokio::task::spawn_blocking(move || report(sequence)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    };
    #[test]
    fn gui_context_requires_current_owner_edition_and_full_install_record() {
        use switcher_domain::system::InstallTrust;
        let record = InstallRecord {
            schema_version: 2,
            owner_uid: 501,
            edition: Edition::Personal,
            trust: InstallTrust::SourceLocal {
                generation: "a".repeat(32),
                gui_cdhash: "1".repeat(40),
                guardian_cdhash: "2".repeat(40),
                helper_cdhash: "3".repeat(40),
            },
            helper_version: "0.2.0".into(),
            minimum_app_version: "0.2.0".into(),
        };
        let challenge = InstallHealthChallenge {
            schema_version: 1,
            record: record.clone(),
            generation: "a".repeat(32),
            nonce: "b".repeat(64),
            boot_session_uuid: "12345678-1234-1234-1234-123456789abc".into(),
            expires_at: 200,
        };
        let boot = challenge.boot_session_uuid.as_str();
        assert!(validate_context(&challenge, &record, Edition::Personal, 501, boot, 100).is_ok());
        assert!(validate_context(&challenge, &record, Edition::Public, 501, boot, 100).is_err());
        assert!(validate_context(&challenge, &record, Edition::Personal, 502, boot, 100).is_err());
        assert!(validate_context(&challenge, &record, Edition::Personal, 501, boot, 200).is_err());
        assert!(
            validate_context(
                &challenge,
                &record,
                Edition::Personal,
                501,
                "other-boot",
                100
            )
            .is_err()
        );
        let mut changed = record.clone();
        changed.helper_version = "0.2.1".into();
        assert!(validate_context(&challenge, &changed, Edition::Personal, 501, boot, 100).is_err());
    }
    #[tokio::test]
    async fn guardian_probe_is_single_flight_and_does_not_block_ticks() {
        let (tx, rx) = mpsc::channel();
        let seen = Arc::new(AtomicU64::new(0));
        let observed = seen.clone();
        let mut probe = GuardianProbe::default();
        probe.tick_with(1, move |sequence| {
            observed.store(sequence, Ordering::SeqCst);
            let _ = rx.recv_timeout(std::time::Duration::from_secs(2));
        });
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while seen.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        probe.tick_with(2, |_| panic!("second concurrent probe"));
        assert_eq!(seen.load(Ordering::SeqCst), 1);
        tx.send(()).unwrap();
        probe.flight.take().unwrap().await.unwrap();
        let observed = seen.clone();
        probe.tick_with(3, move |sequence| {
            observed.store(sequence, Ordering::SeqCst);
        });
        probe.flight.take().unwrap().await.unwrap();
        assert_eq!(seen.load(Ordering::SeqCst), 3);
    }
    #[tokio::test]
    async fn successful_pending_challenge_is_reported_again_after_retry_interval() {
        let mut probe = GuardianProbe::default();
        let seen = Arc::new(AtomicU64::new(0));
        let start = std::time::Instant::now();
        let challenge_nonce = "b".repeat(64);
        for (sequence, seconds, expected) in [(1, 0, 1), (2, 1, 1), (3, 10, 2)] {
            let observed = seen.clone();
            let nonce = challenge_nonce.clone();
            probe.tick_at(
                sequence,
                start + std::time::Duration::from_secs(seconds),
                move |_| {
                    assert_eq!(nonce, "b".repeat(64));
                    observed.fetch_add(1, Ordering::SeqCst);
                },
            );
            if let Some(flight) = probe.flight.take() {
                flight.await.unwrap();
            }
            assert_eq!(seen.load(Ordering::SeqCst), expected);
        }
    }
    #[tokio::test]
    async fn failed_probe_worker_does_not_prevent_a_later_tick() {
        let mut probe = GuardianProbe::default();
        probe.tick_with(1, |_| panic!("isolated installation probe failure"));
        assert!(probe.flight.take().unwrap().await.is_err());
        let seen = Arc::new(AtomicU64::new(0));
        let observed = seen.clone();
        probe.tick_with(2, move |sequence| {
            observed.store(sequence, Ordering::SeqCst);
        });
        probe.flight.take().unwrap().await.unwrap();
        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn guardian_zero_sequence_is_rejected_before_native_io() {
        assert_eq!(guardian_tick(0), Err("installation_health_loop_sequence"));
    }
}
