//! Pure projection of synthetic core readiness. No Keychain, backup or system access.
#![allow(dead_code)]
#[path = "../src/bridge.rs"]
mod bridge;
#[path = "../src/update_runtime.rs"]
mod update_runtime;
use switcher_core::migration::{
    DependencyAssessment, KeychainMetadataProbe, KeychainQuerySummary, MigrationBlocker,
    MigrationReadiness, ProbeStatus,
};
fn readiness(codes: &[&str], handling_available: bool) -> MigrationReadiness {
    MigrationReadiness {
        ready: codes.is_empty(),
        blockers: codes
            .iter()
            .map(|code| MigrationBlocker {
                code: (*code).into(),
                message: format!("阻塞：{code}"),
            })
            .collect(),
        keychain: KeychainMetadataProbe {
            schema_version: 1,
            status: ProbeStatus::MetadataObserved,
            queries: vec![KeychainQuerySummary {
                scope_id: "SYNTHETIC_PRIVATE_SCOPE".into(),
                os_status: 0,
                item_count: 1,
                attribute_summary_hash: "SYNTHETIC_PRIVATE_HASH".into(),
            }],
            dependency_assessment: if handling_available {
                DependencyAssessment::HandlingPlanAvailable
            } else {
                DependencyAssessment::Unresolved
            },
            reason: "SYNTHETIC_PRIVATE_REASON".into(),
        },
    }
}
#[test]
fn projection_keeps_all_blockers_and_excludes_keychain_metadata() {
    let view = bridge::account_preparation_view(readiness(
        &[
            "installation_unverified",
            "backup_missing",
            "keychain_dependencies_unresolved",
        ],
        false,
    ));
    assert!(!view.ready);
    assert!(!view.backup_allowed);
    assert_eq!(view.blockers.len(), 3);
    assert!(view.blockers.iter().all(|item| !item.recovery.is_empty()));
    let json = serde_json::to_string(&view).unwrap();
    for private in [
        "SYNTHETIC_PRIVATE_SCOPE",
        "SYNTHETIC_PRIVATE_HASH",
        "SYNTHETIC_PRIVATE_REASON",
        "queries",
        "item_count",
    ] {
        assert!(!json.contains(private));
    }
}
#[test]
fn metadata_observed_never_implies_dependency_preservation() {
    let view = bridge::account_preparation_view(readiness(&[], false));
    assert!(!view.ready);
    assert!(
        view.blockers
            .iter()
            .any(|item| item.code == "keychain_dependencies_unresolved")
    );
    assert!(!view.backup_allowed);
}
#[test]
fn unsupported_backup_is_never_offered() {
    for code in [
        "installation_unverified",
        "claude_running_or_unknown",
        "repair_recovery_pending",
        "unknown",
        "backup_unknown",
    ] {
        let view = bridge::account_preparation_view(readiness(&[code], true));
        assert!(!view.ready);
        assert!(!view.backup_allowed, "{code}");
    }
    let view = bridge::account_preparation_view(readiness(&["backup_missing"], false));
    assert!(!view.ready);
    assert!(!view.backup_allowed);
    assert!(!bridge::account_preparation_view(readiness(&[], true)).backup_allowed);
}
#[test]
fn only_consistent_complete_readiness_is_presented_as_ready() {
    let view = bridge::account_preparation_view(readiness(&[], true));
    assert!(view.ready);
    let mut core = readiness(&["installation_unverified"], true);
    core.ready = true;
    assert!(!bridge::account_preparation_view(core).ready);
    let mut core = readiness(&[], true);
    core.ready = false;
    assert!(!bridge::account_preparation_view(core).ready);
}

#[test]
fn scoped_permission_never_removes_global_keychain_gate() {
    use switcher_domain::desktop::InPlaceAction;
    let view = bridge::with_in_place_preparation(
        bridge::account_preparation_view(readiness(&[], false)),
        Some(InPlaceAction::InitializeA),
        Some(readiness(&[], false)),
    );
    assert!(view.in_place_ready);
    assert_eq!(view.in_place_action, Some(InPlaceAction::InitializeA));
    assert!(!view.ready);
    assert!(
        view.blockers
            .iter()
            .any(|item| item.code == "keychain_dependencies_unresolved")
    );
    let json = serde_json::to_string(&view).unwrap();
    for private in [
        "SYNTHETIC_PRIVATE_SCOPE",
        "SYNTHETIC_PRIVATE_HASH",
        "SYNTHETIC_PRIVATE_REASON",
        "queries",
    ] {
        assert!(!json.contains(private));
    }
}
#[test]
fn scoped_blockers_and_inconsistent_ready_fail_closed() {
    use switcher_domain::desktop::InPlaceAction;
    for code in [
        "backup_missing",
        "installation_unverified",
        "claude_running_or_unknown",
        "repair_recovery_pending",
        "claude_application_unverified",
        "account_recovery_pending",
        "intent_state_mismatch",
        "association_source_mismatch",
        "keychain_dependencies_unresolved",
    ] {
        let mut scoped = readiness(&[code], false);
        scoped.ready = true;
        let view = bridge::with_in_place_preparation(
            bridge::account_preparation_view(readiness(&[], false)),
            Some(InPlaceAction::AssociateA),
            Some(scoped),
        );
        assert!(!view.in_place_ready, "{code}");
        assert_eq!(view.in_place_blockers[0].code, code);
        assert!(!view.in_place_blockers[0].recovery.is_empty());
        assert!(!view.ready);
    }
    let mut scoped = readiness(&[], false);
    scoped.ready = false;
    let view = bridge::with_in_place_preparation(
        bridge::account_preparation_view(readiness(&[], false)),
        Some(InPlaceAction::InitializeA),
        Some(scoped),
    );
    assert!(!view.in_place_ready);
}
#[test]
fn inspected_current_account_never_offers_reinitialization() {
    use switcher_core::accounts::{AccountId, AccountStatus};
    use switcher_domain::desktop::InPlaceAction;
    for current in [Some(AccountId::A), Some(AccountId::B)] {
        for a_saved in [false, true] {
            let status = AccountStatus {
                current,
                a_saved,
                b_saved: false,
                recovery_pending: false,
            };
            assert_eq!(bridge::in_place_action_for_status(&status), None);
        }
    }
    let mut status = AccountStatus {
        current: None,
        a_saved: false,
        b_saved: false,
        recovery_pending: false,
    };
    assert_eq!(
        bridge::in_place_action_for_status(&status),
        Some(InPlaceAction::InitializeA)
    );
    status.a_saved = true;
    assert_eq!(
        bridge::in_place_action_for_status(&status),
        Some(InPlaceAction::AssociateA)
    );
}
#[test]
fn missing_scoped_context_never_implies_local_permission() {
    use switcher_domain::desktop::InPlaceAction;
    for (action, scoped) in [
        (None, Some(readiness(&[], false))),
        (Some(InPlaceAction::InitializeA), None),
    ] {
        let view = bridge::with_in_place_preparation(
            bridge::account_preparation_view(readiness(&[], false)),
            action,
            scoped,
        );
        assert_eq!(view.in_place_action, None);
        assert!(!view.in_place_ready);
        assert!(view.in_place_blockers.is_empty());
        assert!(!view.ready);
    }
}

#[test]
fn current_compatibility_readiness_matches_core_without_login_claims() {
    let mut core = readiness(&[], false);
    core.keychain = KeychainMetadataProbe::run();
    assert_eq!(
        core.keychain.dependency_assessment,
        DependencyAssessment::VerifiedPreserved
    );
    let view = bridge::account_preparation_view(core);
    assert!(view.ready);
    assert!(view.blockers.is_empty());
    assert!(!view.backup_allowed);
    let json = serde_json::to_string(&view).unwrap();
    for private in [
        "queries",
        "dependency_assessment",
        "verified_preserved",
        "login",
    ] {
        assert!(!json.contains(private), "{private}");
    }
}
#[test]
fn compatibility_assessments_preserve_backend_blockers_and_false_readiness() {
    let codes = [
        "installation_unverified",
        "claude_running_or_unknown",
        "repair_recovery_pending",
        "backup_missing",
        "keychain_dependencies_unresolved",
        "unknown_future_gate",
    ];
    for assessment in [
        DependencyAssessment::VerifiedPreserved,
        DependencyAssessment::HandlingPlanAvailable,
    ] {
        let mut core = readiness(&codes, true);
        core.ready = true;
        core.keychain.dependency_assessment = assessment;
        let view = bridge::account_preparation_view(core);
        assert!(!view.ready);
        assert!(!view.backup_allowed);
        assert_eq!(
            view.blockers
                .iter()
                .map(|b| b.code.as_str())
                .collect::<Vec<_>>(),
            codes
        );
        let mut core = readiness(&[], true);
        core.ready = false;
        core.keychain.dependency_assessment = assessment;
        let view = bridge::account_preparation_view(core);
        assert!(!view.ready);
        assert!(view.blockers.is_empty());
        assert!(!view.backup_allowed);
    }
}
#[test]
fn handling_plan_preserves_every_backend_blocker_and_ready_consistency() {
    let codes = [
        "installation_unverified",
        "claude_running_or_unknown",
        "repair_recovery_pending",
        "backup_missing",
        "keychain_dependencies_unresolved",
        "unknown_future_gate",
    ];
    let mut core = readiness(&codes, true);
    core.ready = true;
    let view = bridge::account_preparation_view(core);
    assert!(!view.ready);
    assert_eq!(
        view.blockers
            .iter()
            .map(|item| item.code.as_str())
            .collect::<Vec<_>>(),
        codes
    );
    assert!(!view.backup_allowed);
    let mut core = readiness(&[], true);
    core.ready = false;
    assert!(!bridge::account_preparation_view(core).ready);
}
#[test]
fn available_handling_plan_exposes_no_metadata_or_login_preservation_claim() {
    let view = bridge::account_preparation_view(readiness(&[], true));
    assert!(view.ready);
    assert!(view.blockers.is_empty());
    let json = serde_json::to_string(&view).unwrap();
    for private in [
        "SYNTHETIC_PRIVATE_SCOPE",
        "SYNTHETIC_PRIVATE_HASH",
        "SYNTHETIC_PRIVATE_REASON",
        "queries",
        "dependency_assessment",
        "login",
        "verified_preserved",
    ] {
        assert!(!json.contains(private), "{private}");
    }
}

#[test]
fn recovery_failure_guidance_does_not_require_new_application_or_keychain_evidence() {
    use switcher_domain::desktop::AccountAction;
    let result =
        bridge::account_action_failure(AccountAction::Recover, "合成恢复未完成：目录记录不一致");
    assert!(!result.completed);
    assert_eq!(result.message, "合成恢复未完成：目录记录不一致");
    let guidance = result.recovery.unwrap();
    for required in ["保留账号资料", "退出状态", "中断记录"] {
        assert!(guidance.contains(required), "{required}");
    }
    for new_dependency in ["安装身份", "备份收据", "Keychain", "登录验收"] {
        assert!(!guidance.contains(new_dependency), "{new_dependency}");
    }
    for action in [
        AccountAction::InitializeA,
        AccountAction::AssociateA,
        AccountAction::SelectA,
        AccountAction::SelectB,
    ] {
        let result = bridge::account_action_failure(action, "合成新操作阻断");
        assert!(!result.completed);
        assert_eq!(result.message, "合成新操作阻断");
        let guidance = result.recovery.unwrap();
        assert!(guidance.contains("保留账号资料"));
        for new_dependency in ["完成安装", "备份收据", "Keychain", "登录验收"] {
            assert!(!guidance.contains(new_dependency), "{new_dependency}");
        }
    }
}
