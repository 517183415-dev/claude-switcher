//! Reassert the configured timezone with the legacy fixed systemsetup operation.
use super::*;

/// Retained for older callers. Unrelated transactions do not suppress the watchdog.
pub fn require_idle_timezone_guard(_root: &Path) -> Result<(), &'static str> {
    Ok(())
}

pub(super) fn reconcile(expected: &str) -> Result<HelperResponse, &'static str> {
    reconcile_target(expected, command)
}

fn reconcile_target(
    expected: &str,
    mut run: impl FnMut(&str, &[&str]) -> Result<String, &'static str>,
) -> Result<HelperResponse, &'static str> {
    validate_timezone(expected)?;
    let read_timezone = |output: String| -> Result<String, &'static str> {
        let zone = output
            .trim()
            .strip_prefix("Time Zone: ")
            .ok_or("timezone_unreadable")?;
        validate_timezone(zone)?;
        Ok(zone.into())
    };
    let current = read_timezone(run("/usr/sbin/systemsetup", &["-gettimezone"])?)?;
    let changed = current != expected;
    if changed {
        run("/usr/sbin/systemsetup", &["-settimezone", expected])?;
        if read_timezone(run("/usr/sbin/systemsetup", &["-gettimezone"])?)? != expected {
            return Err("timezone_guard_verification");
        }
    }
    let mut response = HelperResponse::status(
        if changed {
            "timezone_guard_restored"
        } else {
            "timezone_guard_unchanged"
        },
        true,
    );
    response.items.push(RepairItemResult {
        item: "timezone".into(),
        changed,
        restored: false,
        error: None,
    });
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_reconciles_without_managed_record_or_receipt() {
        let mut calls = Vec::new();
        let response = reconcile_target("Asia/Tokyo", |program, args| {
            calls.push((
                program.to_string(),
                args.iter().map(|v| v.to_string()).collect::<Vec<_>>(),
            ));
            Ok(format!(
                "Time Zone: {}",
                if calls.len() == 1 {
                    "UTC"
                } else {
                    "Asia/Tokyo"
                }
            ))
        })
        .unwrap();
        assert!(response.ok);
        assert_eq!(
            calls[1],
            (
                "/usr/sbin/systemsetup".into(),
                vec!["-settimezone".into(), "Asia/Tokyo".into()]
            )
        );
    }
    #[test]
    fn invalid_target_and_failed_readback_do_not_report_success() {
        assert!(reconcile_target("../invalid", |_, _| panic!("invalid target")).is_err());
        assert_eq!(
            reconcile_target("Asia/Tokyo", |_, _| Ok("Time Zone: UTC".into())).unwrap_err(),
            "timezone_guard_verification"
        );
    }
    #[test]
    fn unrelated_pending_transaction_does_not_disable_watchdog() {
        assert!(require_idle_timezone_guard(Path::new("/fixture/not-read")).is_ok());
    }
}
