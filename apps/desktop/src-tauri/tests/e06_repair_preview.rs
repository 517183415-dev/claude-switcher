//! Pure UI confirmation state. No native client, filesystem or system operations.
#![allow(dead_code)]
#[path = "../src/bridge.rs"]
mod bridge;
#[path = "../src/update_runtime.rs"]
mod update_runtime;
use bridge::RepairPlanState;
#[test]
fn e06_repair_preview_newer_request_rejects_late_old_preview() {
    let mut plans = RepairPlanState::default();
    plans.begin("old".into());
    plans.begin("new".into());
    assert!(!plans.matches("old"));
    assert!(plans.matches("new"));
}
#[test]
fn e06_repair_preview_cancel_old_request_does_not_cancel_new_plan() {
    let mut plans = RepairPlanState::default();
    plans.begin("old".into());
    plans.begin("new".into());
    plans.cancel("old");
    assert!(plans.matches("new"));
    plans.cancel("new");
    assert!(!plans.matches("new"));
}
#[test]
fn e06_repair_preview_execution_consumes_only_matching_request_and_never_invents_a_plan() {
    let mut plans = RepairPlanState::default();
    assert!(plans.take("missing").is_none());
    plans.begin("shown".into());
    assert!(plans.take("stale").is_none());
    assert!(plans.matches("shown"));
    assert!(plans.take("shown").is_none());
    assert!(!plans.matches("shown"));
    assert!(plans.take("shown").is_none());
}

#[test]
fn e06_repair_preview_cancel_before_worker_start_rejects_completion() {
    let mut plans = RepairPlanState::default();
    plans.begin("queued-a".into());
    plans.cancel("queued-a");
    // None isolates confirmation bookkeeping, without invoking a native repair.
    assert!(!plans.complete("queued-a", None));
    assert!(!plans.matches("queued-a"));
    assert!(plans.take("queued-a").is_none());
}
#[test]
fn e06_repair_preview_reverse_worker_completion_preserves_new_request() {
    let mut plans = RepairPlanState::default();
    plans.begin("queued-a".into());
    plans.cancel("queued-a");
    plans.begin("queued-b".into());
    assert!(plans.complete("queued-b", None));
    // The old worker starts/completes after B. Completion never calls begin.
    assert!(!plans.complete("queued-a", None));
    plans.cancel("queued-a");
    assert!(plans.matches("queued-b"));
    assert!(!plans.matches("queued-a"));
}
