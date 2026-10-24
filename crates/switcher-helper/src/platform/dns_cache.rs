use super::RepairItemResult;

/// Refresh only after a verified administrator repair changed persistent system settings.
/// Caller retains its repair journal when this returns false.
pub(super) fn refresh_after_changes(
    results: &mut Vec<RepairItemResult>,
    mut run: impl FnMut(&str, &[&str]) -> Result<String, &'static str>,
) -> bool {
    if !results
        .iter()
        .any(|item| matches!(item.item.as_str(), "timezone" | "ipv6") && item.changed)
    {
        return true;
    }
    for (item, program, args) in [
        (
            "dns_cache_flush",
            "/usr/bin/dscacheutil",
            &["-flushcache"][..],
        ),
        (
            "dns_responder_reload",
            "/usr/bin/killall",
            &["-HUP", "mDNSResponder"][..],
        ),
    ] {
        let outcome = run(program, args);
        let success = outcome.is_ok();
        results.push(RepairItemResult {
            item: item.into(),
            changed: success,
            restored: false,
            error: outcome.err().map(str::to_string),
        });
        if !success {
            return false;
        }
    }
    true
}

// Scoped unit-test interception, with no runtime/environment activation surface.
#[cfg(test)]
pub(super) mod test_io {
    use super::super::*;
    use std::cell::RefCell;

    struct Case {
        timezone: String,
        ipv6: Ipv6Policy,
        failure: Option<&'static str>,
        commands: Vec<(String, Vec<String>)>,
        journal: Vec<String>,
        plan: RepairPlan,
        pending: bool,
    }
    thread_local! {
        static CASE: RefCell<Option<Case>> = const { RefCell::new(None) };
    }
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            CASE.with(|case| *case.borrow_mut() = None);
        }
    }
    fn scoped(changed: bool, failure: Option<&'static str>, run: impl FnOnce()) -> Case {
        CASE.with(|case| {
            assert!(case.borrow().is_none());
            *case.borrow_mut() = Some(Case {
                timezone: if changed { "UTC" } else { "Asia/Tokyo" }.into(),
                ipv6: Ipv6Policy::Off,
                failure,
                commands: Vec::new(),
                journal: Vec::new(),
                plan: plan(changed),
                pending: false,
            });
        });
        let _reset = Reset;
        run();
        CASE.with(|case| case.borrow_mut().take().unwrap())
    }
    pub(crate) fn command(program: &str, args: &[&str]) -> Option<Result<String, &'static str>> {
        CASE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let case = slot.as_mut()?;
            case.commands
                .push((program.into(), args.iter().map(|s| (*s).into()).collect()));
            Some(match (program, args) {
                ("/sbin/route", ["-n", "get", "1.1.1.1"]) => Ok("interface: utun5".into()),
                ("/usr/sbin/networksetup", ["-listallnetworkservices"]) => {
                    Ok("Services\nWi-Fi".into())
                }
                ("/usr/sbin/networksetup", ["-getinfo", "Wi-Fi"]) => Ok(format!(
                    "IPv6: {}",
                    if case.ipv6 == Ipv6Policy::Off {
                        "Off"
                    } else {
                        "Automatic"
                    }
                )),
                ("/usr/sbin/systemsetup", ["-gettimezone"]) => {
                    Ok(format!("Time Zone: {}", case.timezone))
                }
                ("/usr/sbin/systemsetup", ["-settimezone", zone @ ("Asia/Tokyo" | "UTC")]) => {
                    case.timezone = (*zone).into();
                    Ok(String::new())
                }
                ("/usr/bin/dscacheutil", ["-flushcache"])
                | ("/usr/bin/killall", ["-HUP", "mDNSResponder"]) => {
                    if case.failure == Some(program) {
                        Err("system_command_timeout")
                    } else {
                        Ok(String::new())
                    }
                }
                _ => panic!(
                    "unconfigured DNS repair command blocked before spawn: {program} {args:?}"
                ),
            })
        })
    }
    pub(crate) fn journal(operation: &str, code: Option<&str>) -> bool {
        CASE.with(|slot| {
            let mut slot = slot.borrow_mut();
            let Some(case) = slot.as_mut() else {
                return false;
            };
            match operation {
                "persist" => case.pending = true,
                "clear" => case.pending = false,
                _ => {}
            }
            case.journal.push(code.unwrap_or(operation).into());
            true
        })
    }
    pub(crate) fn pending_plan() -> Option<Option<RepairPlan>> {
        CASE.with(|slot| {
            slot.borrow()
                .as_ref()
                .map(|case| case.pending.then(|| case.plan.clone()))
        })
    }
    #[test]
    fn dns_cache_failed_refresh_recovers_pending_without_granting_timezone_authority() {
        for failed_program in ["/usr/bin/dscacheutil", "/usr/bin/killall"] {
            let case = scoped(true, Some(failed_program), || {
                let items = Platform.apply_plan(&plan(true));
                assert!(items.iter().any(|item| item.error.is_some()));
                assert!(pending_plan().unwrap().is_some());
                recover_pending_repair().unwrap();
                assert!(pending_plan().unwrap().is_none());
                // A second recovery is an idempotent no-op.
                recover_pending_repair().unwrap();
            });
            assert_eq!(case.timezone, "UTC");
            assert_eq!(case.journal, ["persist", "repair_restored", "clear"]);
            assert!(!case.pending);
            assert_eq!(
                case.commands
                    .iter()
                    .filter(|(program, _)| program == failed_program)
                    .count(),
                1
            );
        }
    }
    fn plan(changed: bool) -> RepairPlan {
        RepairPlan {
            id: "a".repeat(64),
            original: SystemState {
                timezone: if changed { "UTC" } else { "Asia/Tokyo" }.into(),
                network_service: "Wi-Fi".into(),
                ipv6: Ipv6Policy::Off,
            },
            target: RegionPolicy {
                timezone: "Asia/Tokyo".into(),
                network_service: "Wi-Fi".into(),
                ipv6: Ipv6Policy::Off,
            },
            expires_at: 100,
        }
    }
    #[test]
    fn dns_cache_no_system_change_skips_both_commands() {
        let case = scoped(false, None, || {
            let items = Platform.apply_plan(&plan(false));
            assert_eq!(items.len(), 2);
            assert!(
                items
                    .iter()
                    .all(|item| !item.changed && item.error.is_none())
            );
        });
        assert!(
            !case
                .commands
                .iter()
                .any(|(p, _)| p.starts_with("/usr/bin/"))
        );
        assert_eq!(
            case.journal,
            ["persist", "managed", "repair_applied", "clear"]
        );
    }
    #[test]
    fn dns_cache_changed_repair_runs_fixed_commands_and_then_commits() {
        let case = scoped(true, None, || {
            let items = Platform.apply_plan(&plan(true));
            assert_eq!(items.len(), 4);
            assert!(
                items
                    .iter()
                    .all(|item| item.error.is_none() && !item.restored)
            );
            assert!(items[0].changed && items[2].changed && items[3].changed);
        });
        let dns: Vec<_> = case
            .commands
            .iter()
            .filter(|(p, _)| p.starts_with("/usr/bin/"))
            .cloned()
            .collect();
        assert_eq!(
            dns,
            [
                ("/usr/bin/dscacheutil".into(), vec!["-flushcache".into()]),
                (
                    "/usr/bin/killall".into(),
                    vec!["-HUP".into(), "mDNSResponder".into()]
                ),
            ]
        );
        assert_eq!(
            case.journal,
            ["persist", "managed", "repair_applied", "clear"]
        );
    }
    #[test]
    fn dns_cache_failure_preserves_changes_and_journal_without_applied_completion() {
        for failed_program in ["/usr/bin/dscacheutil", "/usr/bin/killall"] {
            let case = scoped(true, Some(failed_program), || {
                let items = Platform.apply_plan(&plan(true));
                assert!(items[0].changed);
                assert!(items.iter().all(|item| !item.restored));
                let failure = items.last().unwrap();
                assert_eq!(failure.error.as_deref(), Some("system_command_timeout"));
                assert_eq!(
                    failure.item,
                    if failed_program.ends_with("killall") {
                        "dns_responder_reload"
                    } else {
                        "dns_cache_flush"
                    }
                );
                if failed_program.ends_with("killall") {
                    assert!(items[2].changed && items[2].error.is_none());
                }
            });
            assert_eq!(case.timezone, "Asia/Tokyo");
            assert_eq!(case.journal, ["persist"]);
            assert_eq!(case.commands.last().unwrap().0, failed_program);
        }
    }
}
