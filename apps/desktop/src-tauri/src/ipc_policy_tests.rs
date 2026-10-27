//! Ordinary capability policy through Tauri's real resolver, without a WebView,
//! DesktopState, native service, account, or command handler execution.
use std::collections::{BTreeMap, BTreeSet};
use tauri::{
    ipc::{Origin, RuntimeAuthority},
    utils::{
        acl::{
            ExecutionContext, capability::Capability, get_capabilities, manifest::Manifest,
            resolved::Resolved,
        },
        config::Config,
        platform::Target,
    },
};

fn registrations(edition: &str) -> BTreeSet<String> {
    let source = include_str!("main.rs");
    let marker = format!("#[cfg(all(not(feature = \"ipc-smoke\"), feature = \"{edition}\"))]");
    source
        .split_once(&marker)
        .expect("ordinary edition registration")
        .1
        .split_once("]);")
        .expect("registration terminator")
        .0
        .lines()
        .filter_map(|line| {
            let line = line.trim().trim_end_matches(',');
            [
                "commands::",
                "publisher_commands::",
                "publisher_recovery_commands::",
            ]
            .iter()
            .find_map(|prefix| line.strip_prefix(prefix).map(str::to_owned))
        })
        .collect()
}

pub(super) fn ordinary_policy() -> (RuntimeAuthority, BTreeSet<String>) {
    // These are the same generated manifests and capability files consumed by
    // generate_context!. Apply the edition's capability override, when present.
    let manifests: BTreeMap<String, Manifest> = serde_json::from_str(include_str!(concat!(
        env!("OUT_DIR"),
        "/acl-manifests.json"
    )))
    .unwrap();
    let files: BTreeMap<String, Capability> =
        serde_json::from_str(include_str!(concat!(env!("OUT_DIR"), "/capabilities.json"))).unwrap();
    let mut config: Config = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let edition: serde_json::Value = serde_json::from_str(if cfg!(feature = "personal") {
        include_str!("../../../../packaging/editions/personal.json")
    } else {
        include_str!("../../../../packaging/editions/public.json")
    })
    .unwrap();
    if let Some(capabilities) = edition.pointer("/app/security/capabilities") {
        config.app.security.capabilities = serde_json::from_value(capabilities.clone()).unwrap();
    }
    let capabilities = get_capabilities(&config, files, None).unwrap();
    let resolved = Resolved::resolve(&manifests, capabilities, Target::MacOS).unwrap();
    assert!(
        resolved.has_app_acl,
        "ordinary app commands must require ACL"
    );
    assert!(
        resolved.allowed_commands.values().flatten().all(|entry| {
            matches!(&entry.context, ExecutionContext::Local)
                && entry.windows.len() == 1
                && entry.windows[0].as_str() == "main"
                && entry.webviews.is_empty()
        }),
        "all grants must be local and limited to the main window"
    );
    let granted = resolved.allowed_commands.keys().cloned().collect();
    (tauri::runtime_authority!(manifests, resolved), granted)
}

#[test]
fn ordinary_capabilities_match_registration_and_reject_other_windows_and_remote_origins() {
    let edition = if cfg!(feature = "personal") {
        "personal"
    } else {
        "public"
    };
    let expected = registrations(edition);
    assert_eq!(expected.len(), if edition == "personal" { 48 } else { 23 });
    let (authority, granted) = ordinary_policy();
    assert_eq!(granted, expected, "ordinary ACL and handlers must agree");
    for command in &expected {
        assert!(
            authority
                .resolve_access(command, "main", "main", &Origin::Local)
                .is_some(),
            "{edition}: local main must allow {command}"
        );
        for (window, webview) in [
            ("other", "other"),
            ("other", "main"),
            ("main-copy", "main"),
            ("", "main"),
            ("MAIN", "main"),
        ] {
            assert!(
                authority
                    .resolve_access(command, window, webview, &Origin::Local)
                    .is_none(),
                "{edition}: window {window:?} must reject {command}"
            );
        }
        for url in ["https://example.invalid/", "http://127.0.0.1:5555/"] {
            let origin = Origin::Remote {
                url: url.parse().unwrap(),
            };
            assert!(
                authority
                    .resolve_access(command, "main", "main", &origin)
                    .is_none(),
                "{edition}: remote {url} must reject {command}"
            );
        }
    }
}

#[test]
fn ordinary_capabilities_reject_fixture_unknown_and_cross_edition_commands() {
    let (authority, _) = ordinary_policy();
    let mut denied: BTreeSet<String> = include_str!("../build.rs")
        .split('"')
        .enumerate()
        .filter(|(index, _)| index % 2 == 1)
        .map(|(_, token)| token)
        .filter(|token| {
            token.starts_with("ipc_") || matches!(*token, "forbidden_probe" | "finish_smoke")
        })
        .map(str::to_owned)
        .collect();
    assert!(denied.len() >= 15, "fixture command list must not be empty");
    if cfg!(feature = "public") {
        denied.extend(
            registrations("personal")
                .difference(&registrations("public"))
                .cloned(),
        );
    }
    denied.extend([
        "__unregistered__".into(),
        "plugin:shell|execute".into(),
        "plugin:fs|read_file".into(),
    ]);
    for command in denied {
        assert!(
            authority
                .resolve_access(&command, "main", "main", &Origin::Local)
                .is_none(),
            "ordinary local main must reject {command}"
        );
    }
}
