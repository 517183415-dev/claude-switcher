use std::collections::HashMap;
use switcher_domain::system::*;

/// Only the native authenticator can create this proof in production.
pub struct VerifiedPeer {
    pub(crate) uid: u32,
    pub(crate) role: ClientRole,
    pub(crate) connection: usize,
}

struct Session {
    token: String,
    role: ClientRole,
    expires_at: u64,
    sequence: u64,
    plan: Option<RepairPlan>,
    authorization: Option<(Vec<u8>, bool, u64)>,
}

/// Fixed role union for the native system peer gate; request strings never contribute.
pub(crate) fn allowed_peer_requirement(record: &InstallRecord) -> Result<String, &'static str> {
    let mut roles = vec![ClientRole::Gui, ClientRole::Guardian];
    if !record.is_source_local() {
        roles.push(ClientRole::Installer);
    }
    roles
        .into_iter()
        .map(|role| record.client_requirement(role).map(|r| format!("({r})")))
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join(" or "))
}

pub struct Service {
    record: InstallRecord,
    sessions: HashMap<usize, Session>,
    installation_health: crate::installation_health::HealthTracker,
}

pub trait SystemActions {
    fn pending_repair(&self) -> Result<Option<String>, &'static str> {
        Ok(None)
    }
    fn installation_health_challenge(
        &self,
        record: &InstallRecord,
        now: u64,
    ) -> Result<InstallHealthChallenge, &'static str> {
        crate::installation_health::verified_challenge(record, now)
    }
    fn installation_health_receipt(
        &self,
        receipt: &InstallHealthReceipt,
    ) -> Result<(), &'static str> {
        crate::installation_health::publish_receipt(receipt)
    }

    fn consume_authorization(&self, _external_form: &[u8]) -> Result<(), &'static str> {
        Err("authorization_unavailable")
    }
    fn random_token(&self) -> Result<String, &'static str>;
    fn read_state(&self, service: &str) -> Result<SystemState, &'static str>;
    fn validate_policy(&self, target: &RegionPolicy) -> Result<(), &'static str>;
    fn validate_repair_environment(&self, target: &RegionPolicy) -> Result<(), &'static str> {
        self.validate_policy(target)
    }
    fn apply_plan(&self, plan: &RepairPlan) -> Vec<RepairItemResult>;
    fn prepare_plan(&self, _plan: &RepairPlan) -> Result<(), &'static str> {
        Ok(())
    }
    fn plan_not_started(&self, _plan: &RepairPlan) -> Result<(), &'static str> {
        Ok(())
    }
    fn recover_repair(&self, _plan_id: &str) -> Result<HelperResponse, &'static str> {
        Err("repair_recovery_unavailable")
    }
    fn repair_status(&self, _plan_id: &str) -> Result<HelperResponse, &'static str> {
        Err("repair_status_unavailable")
    }
    fn reconcile_managed_timezone(&self, _expected: &str) -> Result<HelperResponse, &'static str> {
        Err("timezone_guard_unavailable")
    }
    fn guardian_status(&self, owner_uid: u32) -> Result<(), &'static str>;
    fn restore_managed_system(&self) -> Result<(), &'static str> {
        Err("uninstall_restore_unavailable")
    }
    fn commit_installation(&self, _owner_uid: u32) -> Result<(), &'static str> {
        Err("installation_commit_unavailable")
    }
}

impl Service {
    pub fn new(record: InstallRecord) -> Result<Self, &'static str> {
        record.validate()?;
        Ok(Self {
            record,
            sessions: HashMap::new(),
            installation_health: Default::default(),
        })
    }

    pub fn disconnect(&mut self, connection: usize) {
        self.sessions.remove(&connection);
    }

    pub fn handle(
        &mut self,
        peer: &VerifiedPeer,
        request: HelperRequest,
        now: u64,
        system: &impl SystemActions,
    ) -> HelperResponse {
        let result = self.dispatch(peer, request, now, system);
        result.unwrap_or_else(|code| HelperResponse::status(code, false))
    }

    fn dispatch(
        &mut self,
        peer: &VerifiedPeer,
        request: HelperRequest,
        now: u64,
        system: &impl SystemActions,
    ) -> Result<HelperResponse, &'static str> {
        if peer.uid != self.record.owner_uid {
            return Err("owner_uid");
        }
        if peer.role != request.role {
            return Err("role_identity");
        }
        self.record.client_requirement(request.role)?;
        // Expiry removes write authority, but a still-bound unconsumed plan can record a
        // definite no-start result before its session is discarded.
        if let HelperAction::ApplyRepair { plan_id } = &request.operation {
            let expired_plan = self
                .sessions
                .get(&peer.connection)
                .filter(|session| {
                    session.expires_at <= now
                        && session.role == peer.role
                        && request.session.as_deref() == Some(session.token.as_str())
                        && request.sequence > session.sequence
                })
                .and_then(|session| session.plan.as_ref())
                .filter(|plan| &plan.id == plan_id)
                .cloned();
            if let Some(plan) = expired_plan {
                system.plan_not_started(&plan)?;
                self.sessions.remove(&peer.connection);
                let mut response = HelperResponse::status("repair_not_started", false);
                response.plan = Some(plan);
                return Ok(response);
            }
        }
        self.sessions.retain(|_, session| session.expires_at > now);
        if matches!(request.operation, HelperAction::BeginSession) {
            if request.session.is_some() || request.sequence != 0 {
                return Err("session_begin");
            }
            if self.sessions.len() >= 64 {
                return Err("session_limit");
            }
            let token = system.random_token()?;
            self.sessions.insert(
                peer.connection,
                Session {
                    token: token.clone(),
                    role: peer.role,
                    expires_at: now.saturating_add(300),
                    sequence: 0,
                    plan: None,
                    authorization: None,
                },
            );
            let mut response = HelperResponse::status("session_ready", true);
            response.session = Some(token);
            return Ok(response);
        }
        let session = self
            .sessions
            .get_mut(&peer.connection)
            .ok_or("session_missing")?;
        if request.session.as_deref() != Some(session.token.as_str()) || session.role != peer.role {
            return Err("session_binding");
        }
        if request.sequence == 0 || request.sequence <= session.sequence {
            return Err("replayed_message");
        }
        session.sequence = request.sequence;
        let mut response = HelperResponse::status("ok", true);
        match request.operation {
            HelperAction::BeginSession => unreachable!(),
            HelperAction::InstallationHealth {
                generation,
                nonce,
                loop_sequence,
            } => {
                let challenge = system.installation_health_challenge(&self.record, now)?;
                let receipt = self.installation_health.observe(
                    challenge,
                    &self.record,
                    peer.role,
                    (&generation, &nonce, loop_sequence),
                    now,
                )?;
                response.code = if let Some(receipt) = receipt {
                    system.installation_health_receipt(&receipt)?;
                    "installation_health_ready"
                } else {
                    "installation_health_pending"
                }
                .into();
            }
            HelperAction::ReadSystemState => response.state = Some(system.read_state("Wi-Fi")?),
            HelperAction::ReconcileManagedTimezone { expected_timezone } => {
                if peer.role != ClientRole::Guardian {
                    return Err("role_action");
                }
                return system.reconcile_managed_timezone(&expected_timezone);
            }
            HelperAction::GuardianStatus => system.guardian_status(self.record.owner_uid)?,
            HelperAction::RestoreManagedSystem => {
                if !matches!(peer.role, ClientRole::Gui | ClientRole::Installer) {
                    return Err("role_action");
                }
                system.restore_managed_system()?;
            }
            HelperAction::CommitInstallation => {
                if !matches!(peer.role, ClientRole::Gui | ClientRole::Installer) {
                    return Err("role_action");
                }
                system.commit_installation(self.record.owner_uid)?;
            }
            HelperAction::PreviewRepair { target } => {
                if !matches!(peer.role, ClientRole::Gui | ClientRole::Installer) {
                    return Err("role_action");
                }
                system.validate_policy(&target)?;
                let plan = RepairPlan {
                    id: system.random_token()?,
                    original: system.read_state(&target.network_service)?,
                    target,
                    expires_at: now.saturating_add(30),
                };
                system.prepare_plan(&plan)?;
                session.authorization = None;
                session.plan = Some(plan.clone());
                response.plan = Some(plan);
            }
            HelperAction::AuthorizeRepair {
                plan_id,
                recovery,
                external_form,
            } => {
                session.authorization = None;
                if !matches!(peer.role, ClientRole::Gui | ClientRole::Installer) {
                    return Err("role_action");
                }
                let plan = if recovery {
                    let status = system.repair_status(&plan_id)?;
                    if status.code != "repair_recovery_required" {
                        return Err("repair_recovery_not_required");
                    }
                    status.plan.ok_or("plan_missing")?
                } else {
                    session.plan.clone().ok_or("plan_missing")?
                };
                if plan.id != plan_id {
                    return Err("plan_missing");
                }
                if !recovery {
                    validate_plan(&plan, now, system)?;
                }
                system.consume_authorization(&external_form)?;
                session.authorization =
                    Some((plan_binding(&plan)?, recovery, now.saturating_add(30)));
                response.code = "repair_authorized".into();
            }
            HelperAction::RecoverRepair { plan_id } => {
                if !matches!(peer.role, ClientRole::Gui | ClientRole::Installer) {
                    return Err("role_action");
                }
                let status = system.repair_status(&plan_id)?;
                let plan = status.plan.ok_or("plan_missing")?;
                if plan.id != plan_id {
                    return Err("plan_missing");
                }
                consume_grant(session, &plan, true, now)?;
                return system.recover_repair(&plan_id);
            }
            HelperAction::RepairStatus { plan_id } => {
                if !matches!(peer.role, ClientRole::Gui | ClientRole::Installer) {
                    return Err("role_action");
                }
                return system.repair_status(&plan_id);
            }
            HelperAction::ApplyRepair { plan_id } => {
                if !matches!(peer.role, ClientRole::Gui | ClientRole::Installer) {
                    return Err("role_action");
                }
                let plan = session.plan.take().ok_or("plan_missing")?;
                if plan.id != plan_id {
                    return Err("plan_missing");
                }
                let preflight: Result<(), &'static str> = (|| {
                    consume_grant(session, &plan, false, now)?;
                    validate_plan(&plan, now, system)?;
                    Ok(())
                })();
                if preflight.is_err() {
                    system.plan_not_started(&plan)?;
                    let mut rejected = HelperResponse::status("repair_not_started", false);
                    rejected.plan = Some(plan);
                    return Ok(rejected);
                }
                response.items = system.apply_plan(&plan);
                response.ok = response.items.iter().all(|i| i.error.is_none());
                response.code = if response.ok {
                    "applied"
                } else {
                    "repair_failed"
                }
                .into();
                if !response.ok {
                    let status = system.repair_status(&plan.id)?;
                    if status.code == "repair_not_started" {
                        return Ok(status);
                    }
                }
                response.state = Some(system.read_state(&plan.target.network_service)?);
                response.plan = Some(plan);
            }
        }
        Ok(response)
    }
}

fn plan_binding(plan: &RepairPlan) -> Result<Vec<u8>, &'static str> {
    serde_json::to_vec(plan).map_err(|_| "plan_binding")
}
fn consume_grant(
    session: &mut Session,
    plan: &RepairPlan,
    recovery: bool,
    now: u64,
) -> Result<(), &'static str> {
    let binding = plan_binding(plan)?;
    match session.authorization.take() {
        Some((bytes, scope, expiry)) if bytes == binding && scope == recovery && expiry > now => {
            Ok(())
        }
        _ => Err("repair_authorization_required"),
    }
}
fn validate_plan(
    plan: &RepairPlan,
    now: u64,
    system: &impl SystemActions,
) -> Result<(), &'static str> {
    if plan.expires_at <= now {
        return Err("plan_expired");
    }
    system.validate_repair_environment(&plan.target)?;
    let current = system.read_state(&plan.target.network_service)?;
    if current.timezone != plan.original.timezone
        || current.ipv6 != plan.original.ipv6
        || current.network_service != plan.original.network_service
    {
        return Err("system_changed");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) struct Fake;
    impl SystemActions for Fake {
        fn consume_authorization(&self, form: &[u8]) -> Result<(), &'static str> {
            if form == b"test-only-proof" {
                Ok(())
            } else {
                Err("authorization_invalid")
            }
        }
        fn random_token(&self) -> Result<String, &'static str> {
            Ok("synthetic-session-token".into())
        }
        fn read_state(&self, service: &str) -> Result<SystemState, &'static str> {
            Ok(SystemState {
                timezone: "America/New_York".into(),
                network_service: service.into(),
                ipv6: Ipv6Policy::Automatic,
            })
        }
        fn validate_policy(&self, p: &RegionPolicy) -> Result<(), &'static str> {
            if p.timezone.contains(';') {
                Err("timezone")
            } else {
                Ok(())
            }
        }
        fn apply_plan(&self, _: &RepairPlan) -> Vec<RepairItemResult> {
            vec![RepairItemResult {
                item: "timezone".into(),
                changed: true,
                restored: false,
                error: None,
            }]
        }
        fn guardian_status(&self, _: u32) -> Result<(), &'static str> {
            Ok(())
        }
    }
    fn service() -> Service {
        Service::new(InstallRecord {
            schema_version: 2,
            owner_uid: 501,
            edition: switcher_domain::Edition::Personal,
            trust: InstallTrust::DeveloperId {
                team_id: "TESTTEAM01".into(),
            },
            helper_version: "0.2.0".into(),
            minimum_app_version: "0.2.0".into(),
        })
        .unwrap()
    }
    fn peer(role: ClientRole) -> VerifiedPeer {
        VerifiedPeer {
            uid: 501,
            role,
            connection: 1,
        }
    }
    fn request(role: ClientRole, operation: HelperAction, seq: u64) -> HelperRequest {
        HelperRequest {
            role,
            session: if seq == 0 {
                None
            } else {
                Some("synthetic-session-token".into())
            },
            sequence: seq,
            operation,
        }
    }
    #[derive(Default)]
    struct AuthorizationBoundary {
        consumed: std::cell::Cell<bool>,
        mutations: std::cell::Cell<u32>,
        environment_changed: std::cell::Cell<bool>,
        recovery: std::cell::RefCell<Option<RepairPlan>>,
    }
    impl SystemActions for AuthorizationBoundary {
        fn random_token(&self) -> Result<String, &'static str> {
            Fake.random_token()
        }
        fn read_state(&self, service: &str) -> Result<SystemState, &'static str> {
            Fake.read_state(service)
        }
        fn validate_policy(&self, target: &RegionPolicy) -> Result<(), &'static str> {
            if self.environment_changed.get() {
                Err("environment_changed")
            } else {
                Fake.validate_policy(target)
            }
        }
        fn consume_authorization(&self, form: &[u8]) -> Result<(), &'static str> {
            if form != b"test-only-proof" || self.consumed.replace(true) {
                Err("authorization_invalid")
            } else {
                Ok(())
            }
        }
        fn apply_plan(&self, plan: &RepairPlan) -> Vec<RepairItemResult> {
            self.mutations.set(self.mutations.get() + 1);
            Fake.apply_plan(plan)
        }
        fn guardian_status(&self, _: u32) -> Result<(), &'static str> {
            Ok(())
        }
        fn repair_status(&self, _: &str) -> Result<HelperResponse, &'static str> {
            let mut result = HelperResponse::status("repair_recovery_required", false);
            result.plan = self.recovery.borrow().clone();
            Ok(result)
        }
        fn recover_repair(&self, _: &str) -> Result<HelperResponse, &'static str> {
            self.mutations.set(self.mutations.get() + 1);
            Ok(HelperResponse::status("repair_restored", true))
        }
    }
    fn authorization_fixture(system: &AuthorizationBoundary) -> (Service, VerifiedPeer) {
        let mut s = service();
        let p = peer(ClientRole::Gui);
        assert!(
            s.handle(
                &p,
                request(p.role, HelperAction::BeginSession, 0),
                1,
                system
            )
            .ok
        );
        assert!(
            s.handle(
                &p,
                request(
                    p.role,
                    HelperAction::PreviewRepair {
                        target: RegionPolicy {
                            timezone: "Asia/Tokyo".into(),
                            network_service: "Wi-Fi".into(),
                            ipv6: Ipv6Policy::Off,
                        }
                    },
                    1
                ),
                2,
                system
            )
            .ok
        );
        (s, p)
    }
    fn authorize_action(recovery: bool) -> HelperAction {
        HelperAction::AuthorizeRepair {
            plan_id: "synthetic-session-token".into(),
            recovery,
            external_form: b"test-only-proof".to_vec(),
        }
    }
    fn apply_action() -> HelperAction {
        HelperAction::ApplyRepair {
            plan_id: "synthetic-session-token".into(),
        }
    }

    #[derive(Default)]
    struct F12Pending {
        authorization: AuthorizationBoundary,
        pending: std::cell::RefCell<Option<RepairPlan>>,
        error: std::cell::Cell<Option<&'static str>>,
        writes: std::cell::RefCell<Vec<&'static str>>,
    }
    impl SystemActions for F12Pending {
        fn pending_repair(&self) -> Result<Option<String>, &'static str> {
            if let Some(error) = self.error.get() {
                return Err(error);
            }
            Ok(self.pending.borrow().as_ref().map(|plan| plan.id.clone()))
        }
        fn random_token(&self) -> Result<String, &'static str> {
            Fake.random_token()
        }
        fn read_state(&self, service: &str) -> Result<SystemState, &'static str> {
            Fake.read_state(service)
        }
        fn validate_policy(&self, target: &RegionPolicy) -> Result<(), &'static str> {
            Fake.validate_policy(target)
        }
        fn guardian_status(&self, _: u32) -> Result<(), &'static str> {
            Ok(())
        }
        fn prepare_plan(&self, _: &RepairPlan) -> Result<(), &'static str> {
            self.writes.borrow_mut().push("prepare");
            Ok(())
        }
        fn plan_not_started(&self, _: &RepairPlan) -> Result<(), &'static str> {
            self.writes.borrow_mut().push("not_started");
            Ok(())
        }
        fn consume_authorization(&self, form: &[u8]) -> Result<(), &'static str> {
            self.writes.borrow_mut().push("authorize");
            self.authorization.consume_authorization(form)
        }
        fn apply_plan(&self, plan: &RepairPlan) -> Vec<RepairItemResult> {
            self.writes.borrow_mut().push("apply");
            Fake.apply_plan(plan)
        }
        fn restore_managed_system(&self) -> Result<(), &'static str> {
            self.writes.borrow_mut().push("restore");
            Ok(())
        }
        fn commit_installation(&self, _: u32) -> Result<(), &'static str> {
            self.writes.borrow_mut().push("commit");
            Ok(())
        }
        fn reconcile_managed_timezone(&self, _: &str) -> Result<HelperResponse, &'static str> {
            self.writes.borrow_mut().push("reconcile");
            Ok(HelperResponse::status("reconciled", true))
        }
        fn installation_health_challenge(
            &self,
            _: &InstallRecord,
            _: u64,
        ) -> Result<InstallHealthChallenge, &'static str> {
            self.writes.borrow_mut().push("health");
            Err("synthetic_health_not_allowed")
        }
        fn installation_health_receipt(&self, _: &InstallHealthReceipt) -> Result<(), &'static str> {
            self.writes.borrow_mut().push("receipt");
            Ok(())
        }
        fn repair_status(&self, _: &str) -> Result<HelperResponse, &'static str> {
            let mut response = HelperResponse::status("repair_recovery_required", false);
            response.plan = self.pending.borrow().clone();
            Ok(response)
        }
        fn recover_repair(&self, _: &str) -> Result<HelperResponse, &'static str> {
            self.writes.borrow_mut().push("recover");
            self.pending.borrow_mut().take();
            Ok(HelperResponse::status("repair_restored", true))
        }
    }
    fn f12_pending_fixture() -> (F12Pending, Service, VerifiedPeer) {
        let system = F12Pending::default();
        let (service, peer) = authorization_fixture(&system.authorization);
        *system.pending.borrow_mut() = service.sessions.get(&peer.connection).unwrap().plan.clone();
        (system, service, peer)
    }
    fn f12_status(id: &str) -> HelperAction {
        HelperAction::RepairStatus { plan_id: id.into() }
    }
    fn f12_recover(id: &str) -> HelperAction {
        HelperAction::RecoverRepair { plan_id: id.into() }
    }
    #[test]
    fn f12_pending_blocks_every_non_recovery_write_before_side_effects() {
        for name in ["preview", "apply", "authorize", "restore", "commit", "reconcile", "health"] {
            let (system, mut service, mut peer) = f12_pending_fixture();
            let action = match name {
                "preview" => HelperAction::PreviewRepair {
                    target: system.pending.borrow().as_ref().unwrap().target.clone(),
                },
                "apply" => {
                    let plan = service.sessions.get(&peer.connection).unwrap().plan.as_ref().unwrap();
                    let binding = plan_binding(plan).unwrap();
                    service.sessions.get_mut(&peer.connection).unwrap().authorization =
                        Some((binding, false, 30));
                    apply_action()
                }
                "authorize" => authorize_action(false),
                "restore" => HelperAction::RestoreManagedSystem,
                "commit" => HelperAction::CommitInstallation,
                "reconcile" => {
                    peer.role = ClientRole::Guardian;
                    service.sessions.get_mut(&peer.connection).unwrap().role = peer.role;
                    HelperAction::ReconcileManagedTimezone { expected_timezone: "UTC".into() }
                }
                "health" => HelperAction::InstallationHealth {
                    generation: "a".repeat(32), nonce: "b".repeat(64), loop_sequence: 1,
                },
                _ => unreachable!(),
            };
            let response = service.handle(&peer, request(peer.role, action, 2), 3, &system);
            assert!(!response.ok, "{name}");
            assert_eq!(response.code, "repair_recovery_required", "{name}");
            assert!(system.writes.borrow().is_empty(), "{name}");
            assert!(system.pending.borrow().is_some(), "{name}");
        }
    }
    #[test]
    fn f12_pending_blocks_expired_apply_before_no_start_receipt() {
        for now in [40, 400] {
            let (system, mut service, peer) = f12_pending_fixture();
            let response = service.handle(&peer, request(peer.role, apply_action(), 2), now, &system);
            assert_eq!(response.code, "repair_recovery_required", "{now}");
            assert!(!response.ok);
            assert!(system.writes.borrow().is_empty());
            assert!(system.pending.borrow().is_some());
        }
    }
    #[test]
    fn f12_pending_allows_sessions_and_readonly_diagnostics() {
        let (system, _, peer) = f12_pending_fixture();
        let mut service = service();
        for (sequence, action) in [
            (0, HelperAction::BeginSession),
            (1, HelperAction::ReadSystemState),
            (2, HelperAction::GuardianStatus),
        ] {
            assert!(service.handle(&peer, request(peer.role, action, sequence), 3, &system).ok);
        }
        let response = service.handle(&peer, request(peer.role, f12_status("synthetic-session-token"), 3), 4, &system);
        assert_eq!(response.code, "repair_recovery_required");
        assert!(response.plan.is_some());
        assert!(system.writes.borrow().is_empty());
    }
    #[test]
    fn f12_pending_rejects_wrong_plan_ids_before_status_or_authorization() {
        for action in [
            f12_status("other"),
            f12_recover("other"),
            HelperAction::AuthorizeRepair { plan_id: "other".into(), recovery: true, external_form: b"test-only-proof".to_vec() },
        ] {
            let (system, mut service, peer) = f12_pending_fixture();
            let response = service.handle(&peer, request(peer.role, action, 2), 3, &system);
            assert!(!response.ok);
            assert!(response.plan.is_none());
            assert!(system.writes.borrow().is_empty());
        }
    }
    #[test]
    fn f12_pending_preserves_owner_role_session_and_replay_boundaries() {
        for case in ["owner", "role", "session", "replay", "guardian", "connection", "expired"] {
            let (system, mut service, mut peer) = f12_pending_fixture();
            let mut req = request(peer.role, f12_status("synthetic-session-token"), 2);
            match case {
                "owner" => peer.uid = 502,
                "role" => req.role = ClientRole::Guardian,
                "session" => req.session = Some("wrong".into()),
                "replay" => req.sequence = 1,
                "guardian" => {
                    peer.role = ClientRole::Guardian;
                    req.role = peer.role;
                    service.sessions.get_mut(&peer.connection).unwrap().role = peer.role;
                }
                "connection" => peer.connection = 2,
                _ => (),
            }
            let now = if case == "expired" { 400 } else { 3 };
            assert!(!service.handle(&peer, req, now, &system).ok, "{case}");
            assert!(system.writes.borrow().is_empty(), "{case}");
        }
    }
    #[test]
    fn f12_pending_requires_fresh_recovery_grant_then_reopens_after_clear() {
        let (system, mut service, peer) = f12_pending_fixture();
        assert!(!service.handle(&peer, request(peer.role, f12_recover("synthetic-session-token"), 2), 3, &system).ok);
        assert!(system.writes.borrow().is_empty());
        assert!(service.handle(&peer, request(peer.role, authorize_action(true), 3), 4, &system).ok);
        assert!(service.handle(&peer, request(peer.role, f12_recover("synthetic-session-token"), 4), 5, &system).ok);
        assert!(system.pending.borrow().is_none());
        assert_eq!(*system.writes.borrow(), ["authorize", "recover"]);
        let target = RegionPolicy { timezone: "UTC".into(), network_service: "Wi-Fi".into(), ipv6: Ipv6Policy::Off };
        assert!(service.handle(&peer, request(peer.role, HelperAction::PreviewRepair { target }, 5), 6, &system).ok);
        assert_eq!(*system.writes.borrow(), ["authorize", "recover", "prepare"]);
    }
    #[test]
    fn f12_pending_recovery_rejects_expired_or_apply_scoped_grants() {
        for (recovery, expiry) in [(false, 30), (true, 2)] {
            let (system, mut service, peer) = f12_pending_fixture();
            let plan = system.pending.borrow().clone().unwrap();
            service.sessions.get_mut(&peer.connection).unwrap().authorization =
                Some((plan_binding(&plan).unwrap(), recovery, expiry));
            assert!(!service.handle(&peer, request(peer.role, f12_recover(&plan.id), 2), 3, &system).ok);
            assert!(system.writes.borrow().is_empty());
            assert!(system.pending.borrow().is_some());
        }
    }
    #[test]
    fn f12_pending_read_error_never_bypasses_gate() {
        for action in [HelperAction::BeginSession, HelperAction::ReadSystemState, HelperAction::RestoreManagedSystem] {
            let (system, mut service, peer) = f12_pending_fixture();
            system.error.set(Some("repair_status_permissions"));
            let sequence = if matches!(action, HelperAction::BeginSession) { 0 } else { 2 };
            let response = service.handle(&peer, request(peer.role, action, sequence), 3, &system);
            assert!(!response.ok);
            assert_eq!(response.code, "repair_status_permissions");
            assert!(system.writes.borrow().is_empty());
        }
    }
    #[test]
    fn native_proof_binding_rejects_invalid_expired_replayed_and_changed_plans() {
        for case in [
            "invalid",
            "expired",
            "replayed",
            "target_changed",
            "original_changed",
            "expiry_changed",
            "environment_changed",
            "other_connection",
            "other_owner",
            "other_role",
        ] {
            let system = AuthorizationBoundary::default();
            let (mut s, p) = authorization_fixture(&system);
            let mut action = authorize_action(false);
            if case == "invalid"
                && let HelperAction::AuthorizeRepair { external_form, .. } = &mut action
            {
                external_form.clear();
            }
            let authorized = s.handle(&p, request(p.role, action, 2), 3, &system);
            assert_eq!(authorized.ok, case != "invalid", "{case}");
            if case == "replayed" {
                assert!(
                    !s.handle(&p, request(p.role, authorize_action(false), 3), 4, &system)
                        .ok
                );
            }
            let plan = s
                .sessions
                .get_mut(&p.connection)
                .unwrap()
                .plan
                .as_mut()
                .unwrap();
            match case {
                "target_changed" => plan.target.timezone = "Europe/London".into(),
                "original_changed" => plan.original.timezone = "Etc/UTC".into(),
                "expiry_changed" => plan.expires_at += 1,
                "environment_changed" => system.environment_changed.set(true),
                _ => (),
            }
            let mut caller = peer(ClientRole::Gui);
            match case {
                "other_connection" => caller.connection = 2,
                "other_owner" => caller.uid = 502,
                "other_role" => caller.role = ClientRole::Guardian,
                _ => (),
            }
            let now = if case == "expired" { 40 } else { 5 };
            let result = s.handle(
                &caller,
                request(caller.role, apply_action(), 4),
                now,
                &system,
            );
            assert!(!result.ok, "{case}");
            assert_eq!(system.mutations.get(), 0, "{case}");
        }
    }
    #[test]
    fn authorized_apply_is_single_use_and_recovery_requires_new_scoped_proof() {
        let system = AuthorizationBoundary::default();
        let (mut s, p) = authorization_fixture(&system);
        let plan = s.sessions.get(&p.connection).unwrap().plan.clone().unwrap();
        *system.recovery.borrow_mut() = Some(plan);
        assert!(
            s.handle(&p, request(p.role, authorize_action(false), 2), 3, &system)
                .ok
        );
        assert!(
            s.handle(&p, request(p.role, apply_action(), 3), 4, &system)
                .ok
        );
        assert!(
            !s.handle(&p, request(p.role, apply_action(), 4), 5, &system)
                .ok
        );
        let recover = || HelperAction::RecoverRepair {
            plan_id: "synthetic-session-token".into(),
        };
        assert!(!s.handle(&p, request(p.role, recover(), 5), 6, &system).ok);
        assert_eq!(system.mutations.get(), 1);
        // A second, fresh native credential; cached/replayed proof was rejected above.
        system.consumed.set(false);
        assert!(
            s.handle(&p, request(p.role, authorize_action(true), 6), 7, &system)
                .ok
        );
        assert!(s.handle(&p, request(p.role, recover(), 7), 8, &system).ok);
        assert!(!s.handle(&p, request(p.role, recover(), 8), 9, &system).ok);
        assert_eq!(system.mutations.get(), 2);
    }
    #[test]
    fn recovery_plan_change_or_apply_scope_cannot_authorize_recovery() {
        for change in [false, true] {
            let system = AuthorizationBoundary::default();
            let (mut s, p) = authorization_fixture(&system);
            *system.recovery.borrow_mut() = s.sessions.get(&p.connection).unwrap().plan.clone();
            assert!(
                s.handle(&p, request(p.role, authorize_action(change), 2), 3, &system)
                    .ok
            );
            if change {
                system
                    .recovery
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .original
                    .timezone = "Etc/UTC".into();
            }
            assert!(
                !s.handle(
                    &p,
                    request(
                        p.role,
                        HelperAction::RecoverRepair {
                            plan_id: "synthetic-session-token".into()
                        },
                        3
                    ),
                    4,
                    &system
                )
                .ok
            );
            assert_eq!(system.mutations.get(), 0);
        }
    }
    #[test]
    fn apply_without_administrator_authorization_never_mutates() {
        let mut service = service();
        let p = peer(ClientRole::Gui);
        service.handle(&p, request(p.role, HelperAction::BeginSession, 0), 1, &Fake);
        service.handle(
            &p,
            request(
                p.role,
                HelperAction::PreviewRepair {
                    target: RegionPolicy {
                        timezone: "Asia/Tokyo".into(),
                        network_service: "Wi-Fi".into(),
                        ipv6: Ipv6Policy::Off,
                    },
                },
                1,
            ),
            2,
            &Fake,
        );
        let result = service.handle(
            &p,
            request(
                p.role,
                HelperAction::ApplyRepair {
                    plan_id: "synthetic-session-token".into(),
                },
                2,
            ),
            3,
            &Fake,
        );
        assert!(!result.ok, "unauthorized apply must be denied");
        assert!(result.items.iter().all(|item| !item.changed));
    }
    #[test]
    fn expired_matching_plan_records_no_start_before_any_mutation() {
        struct NeverWrite(std::cell::Cell<bool>);
        impl SystemActions for NeverWrite {
            fn random_token(&self) -> Result<String, &'static str> {
                Fake.random_token()
            }
            fn read_state(&self, service: &str) -> Result<SystemState, &'static str> {
                Fake.read_state(service)
            }
            fn validate_policy(&self, policy: &RegionPolicy) -> Result<(), &'static str> {
                Fake.validate_policy(policy)
            }
            fn apply_plan(&self, _: &RepairPlan) -> Vec<RepairItemResult> {
                panic!("expired plan must not mutate")
            }
            fn plan_not_started(&self, _: &RepairPlan) -> Result<(), &'static str> {
                self.0.set(true);
                Ok(())
            }
            fn guardian_status(&self, _: u32) -> Result<(), &'static str> {
                Ok(())
            }
        }
        let system = NeverWrite(std::cell::Cell::new(false));
        let mut s = service();
        let p = peer(ClientRole::Gui);
        s.handle(
            &p,
            request(p.role, HelperAction::BeginSession, 0),
            1,
            &system,
        );
        s.handle(
            &p,
            request(
                p.role,
                HelperAction::PreviewRepair {
                    target: RegionPolicy {
                        timezone: "Asia/Tokyo".into(),
                        network_service: "Wi-Fi".into(),
                        ipv6: Ipv6Policy::Off,
                    },
                },
                1,
            ),
            2,
            &system,
        );
        let result = s.handle(
            &p,
            request(
                p.role,
                HelperAction::ApplyRepair {
                    plan_id: "synthetic-session-token".into(),
                },
                2,
            ),
            33,
            &system,
        );
        assert_eq!(result.code, "repair_not_started");
        assert!(!result.ok);
        assert!(system.0.get());
        assert!(result.plan.is_some());
    }
    #[test]
    fn expired_session_records_no_start_for_its_bound_unconsumed_plan() {
        struct NeverWrite(std::cell::Cell<bool>);
        impl SystemActions for NeverWrite {
            fn random_token(&self) -> Result<String, &'static str> {
                Fake.random_token()
            }
            fn read_state(&self, service: &str) -> Result<SystemState, &'static str> {
                Fake.read_state(service)
            }
            fn validate_policy(&self, policy: &RegionPolicy) -> Result<(), &'static str> {
                Fake.validate_policy(policy)
            }
            fn apply_plan(&self, _: &RepairPlan) -> Vec<RepairItemResult> {
                panic!("expired plan must not mutate")
            }
            fn plan_not_started(&self, _: &RepairPlan) -> Result<(), &'static str> {
                self.0.set(true);
                Ok(())
            }
            fn guardian_status(&self, _: u32) -> Result<(), &'static str> {
                Ok(())
            }
        }
        let system = NeverWrite(std::cell::Cell::new(false));
        let mut s = service();
        let p = peer(ClientRole::Gui);
        s.handle(
            &p,
            request(p.role, HelperAction::BeginSession, 0),
            1,
            &system,
        );
        s.handle(
            &p,
            request(
                p.role,
                HelperAction::PreviewRepair {
                    target: RegionPolicy {
                        timezone: "Asia/Tokyo".into(),
                        network_service: "Wi-Fi".into(),
                        ipv6: Ipv6Policy::Off,
                    },
                },
                1,
            ),
            2,
            &system,
        );
        let result = s.handle(
            &p,
            request(
                p.role,
                HelperAction::ApplyRepair {
                    plan_id: "synthetic-session-token".into(),
                },
                2,
            ),
            302,
            &system,
        );
        assert_eq!(result.code, "repair_not_started");
        assert!(!result.ok);
        assert!(system.0.get());
        assert!(result.plan.is_some());
    }
    #[test]
    fn owner_role_session_sequence_and_expiry_matrix() {
        let mut s = service();
        let p = peer(ClientRole::Gui);
        assert_eq!(
            s.handle(
                &VerifiedPeer {
                    uid: 502,
                    ..peer(ClientRole::Gui)
                },
                request(p.role, HelperAction::BeginSession, 0),
                100,
                &Fake
            )
            .code,
            "owner_uid"
        );
        assert_eq!(
            s.handle(
                &p,
                request(ClientRole::Guardian, HelperAction::BeginSession, 0),
                100,
                &Fake
            )
            .code,
            "role_identity"
        );
        assert!(
            s.handle(
                &p,
                request(p.role, HelperAction::BeginSession, 0),
                100,
                &Fake
            )
            .ok
        );
        assert!(
            s.handle(
                &p,
                request(p.role, HelperAction::ReadSystemState, 1),
                101,
                &Fake
            )
            .ok
        );
        assert_eq!(
            s.handle(
                &p,
                request(p.role, HelperAction::ReadSystemState, 1),
                102,
                &Fake
            )
            .code,
            "replayed_message"
        );
        assert_eq!(
            s.handle(
                &VerifiedPeer {
                    connection: 2,
                    ..peer(ClientRole::Gui)
                },
                request(p.role, HelperAction::ReadSystemState, 2),
                102,
                &Fake
            )
            .code,
            "session_missing"
        );
        assert_eq!(
            s.handle(
                &p,
                request(p.role, HelperAction::ReadSystemState, 2),
                400,
                &Fake
            )
            .code,
            "session_missing"
        );
    }
    #[test]
    fn timezone_guard_requires_guardian_owner_live_session_and_sequence() {
        struct Guard(std::cell::Cell<usize>);
        impl SystemActions for Guard {
            fn random_token(&self) -> Result<String, &'static str> {
                Fake.random_token()
            }
            fn read_state(&self, _: &str) -> Result<SystemState, &'static str> {
                panic!("full state read")
            }
            fn validate_policy(&self, _: &RegionPolicy) -> Result<(), &'static str> {
                panic!("policy")
            }
            fn apply_plan(&self, _: &RepairPlan) -> Vec<RepairItemResult> {
                panic!("full repair")
            }
            fn guardian_status(&self, _: u32) -> Result<(), &'static str> {
                panic!("guardian status")
            }
            fn reconcile_managed_timezone(
                &self,
                expected: &str,
            ) -> Result<HelperResponse, &'static str> {
                assert_eq!(expected, "Asia/Tokyo");
                self.0.set(self.0.get() + 1);
                Ok(HelperResponse::status("timezone_guard_unchanged", true))
            }
        }
        for case in [
            "gui",
            "installer",
            "owner",
            "session",
            "expired",
            "sequence",
            "valid",
        ] {
            let system = Guard(std::cell::Cell::new(0));
            let mut s = service();
            let role = match case {
                "gui" => ClientRole::Gui,
                "installer" => ClientRole::Installer,
                _ => ClientRole::Guardian,
            };
            let mut p = peer(role);
            if case != "session" {
                assert!(
                    s.handle(&p, request(role, HelperAction::BeginSession, 0), 1, &system)
                        .ok
                );
            }
            if case == "owner" {
                p.uid = 502;
            }
            let result = s.handle(
                &p,
                request(
                    role,
                    HelperAction::ReconcileManagedTimezone {
                        expected_timezone: "Asia/Tokyo".into(),
                    },
                    if case == "sequence" { 0 } else { 1 },
                ),
                if case == "expired" { 302 } else { 2 },
                &system,
            );
            assert_eq!(result.ok, case == "valid", "{case}");
            assert_eq!(system.0.get(), usize::from(case == "valid"), "{case}");
        }
    }
    #[test]
    fn guardian_cannot_request_root_recovery_or_read_transaction_results() {
        let mut s = service();
        let p = peer(ClientRole::Guardian);
        s.handle(&p, request(p.role, HelperAction::BeginSession, 0), 1, &Fake);
        for (sequence, operation) in [
            HelperAction::RecoverRepair {
                plan_id: "plan".into(),
            },
            HelperAction::RepairStatus {
                plan_id: "plan".into(),
            },
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                s.handle(
                    &p,
                    request(p.role, operation, sequence as u64 + 1),
                    2,
                    &Fake
                )
                .code,
                "role_action"
            );
        }
    }
    #[test]
    fn guardian_cannot_write_and_plan_is_single_use() {
        let target = RegionPolicy {
            timezone: "America/New_York".into(),
            network_service: "Wi-Fi".into(),
            ipv6: Ipv6Policy::Off,
        };
        let mut s = service();
        let p = peer(ClientRole::Guardian);
        s.handle(&p, request(p.role, HelperAction::BeginSession, 0), 1, &Fake);
        assert_eq!(
            s.handle(
                &p,
                request(
                    p.role,
                    HelperAction::PreviewRepair {
                        target: target.clone()
                    },
                    1
                ),
                2,
                &Fake
            )
            .code,
            "role_action"
        );
        let p = peer(ClientRole::Gui);
        s.handle(&p, request(p.role, HelperAction::BeginSession, 0), 3, &Fake);
        assert!(
            s.handle(
                &p,
                request(p.role, HelperAction::PreviewRepair { target }, 1),
                4,
                &Fake
            )
            .ok
        );
        assert!(
            s.handle(
                &p,
                request(
                    p.role,
                    HelperAction::AuthorizeRepair {
                        plan_id: "synthetic-session-token".into(),
                        recovery: false,
                        external_form: b"test-only-proof".to_vec()
                    },
                    2
                ),
                4,
                &Fake
            )
            .ok
        );
        assert!(
            s.handle(
                &p,
                request(
                    p.role,
                    HelperAction::ApplyRepair {
                        plan_id: "synthetic-session-token".into()
                    },
                    3
                ),
                5,
                &Fake
            )
            .ok
        );
        assert_eq!(
            s.handle(
                &p,
                request(
                    p.role,
                    HelperAction::ApplyRepair {
                        plan_id: "synthetic-session-token".into()
                    },
                    4
                ),
                6,
                &Fake
            )
            .code,
            "plan_missing"
        );
    }
}

#[cfg(test)]
mod source_local_tests {
    use super::*;
    fn local() -> InstallRecord {
        InstallRecord {
            schema_version: 2,
            owner_uid: 501,
            edition: switcher_domain::Edition::Personal,
            trust: InstallTrust::SourceLocal {
                generation: "a".repeat(32),
                gui_cdhash: "1".repeat(40),
                guardian_cdhash: "2".repeat(40),
                helper_cdhash: "3".repeat(40),
            },
            helper_version: "0.2.0".into(),
            minimum_app_version: "0.2.0".into(),
        }
    }
    #[test]
    fn source_local_system_gate_contains_only_fixed_gui_and_guardian_pins() {
        let record = local();
        let requirement = allowed_peer_requirement(&record).unwrap();
        assert_eq!(
            requirement,
            format!(
                "({}) or ({})",
                record.client_requirement(ClientRole::Gui).unwrap(),
                record.client_requirement(ClientRole::Guardian).unwrap()
            )
        );
        assert!(!requirement.contains("installer"));
        assert!(!requirement.contains(&"3".repeat(40)));
        let mut developer = record.clone();
        developer.trust = InstallTrust::DeveloperId {
            team_id: "TESTTEAM01".into(),
        };
        assert!(
            allowed_peer_requirement(&developer)
                .unwrap()
                .contains("local.claudeswitcher.installer")
        );
        let mut replaced = record.clone();
        if let InstallTrust::SourceLocal { generation, .. } = &mut replaced.trust {
            *generation = "b".repeat(32);
        }
        assert_ne!(record, replaced);
    }
    #[test]
    fn source_local_installer_cannot_begin_session() {
        let mut service = Service::new(local()).unwrap();
        let peer = VerifiedPeer {
            uid: 501,
            role: ClientRole::Installer,
            connection: 1,
        };
        let request = HelperRequest {
            role: ClientRole::Installer,
            session: None,
            sequence: 0,
            operation: HelperAction::BeginSession,
        };
        let response = service.handle(&peer, request, 1, &super::tests::Fake);
        assert!(!response.ok);
        assert_eq!(response.code, "local_installer_role_unavailable");
    }
    #[test]
    fn source_local_unknown_fields_missing_pins_and_legacy_records_are_rejected() {
        let original = serde_json::to_value(local()).unwrap();
        let mut mixed = original.clone();
        mixed["trust"]["team_id"] = "TESTTEAM01".into();
        assert!(serde_json::from_value::<InstallRecord>(mixed).is_err());
        let mut missing = original.clone();
        missing["trust"]
            .as_object_mut()
            .unwrap()
            .remove("helper_cdhash");
        assert!(serde_json::from_value::<InstallRecord>(missing).is_err());
        let mut unknown = original.clone();
        unknown["extra"] = true.into();
        assert!(serde_json::from_value::<InstallRecord>(unknown).is_err());
        let mut legacy = original;
        legacy["schema_version"] = 1.into();
        legacy.as_object_mut().unwrap().remove("trust");
        legacy["team_id"] = "TESTTEAM01".into();
        assert!(serde_json::from_value::<InstallRecord>(legacy).is_err());
    }
}
