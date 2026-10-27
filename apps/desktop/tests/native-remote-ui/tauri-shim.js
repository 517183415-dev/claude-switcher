// Only the two potentially system-reading startup calls are simulated.
// Every publisher command uses the unmodified Tauri transport and product ACL.
import {invoke as actualInvoke} from '../../node_modules/@tauri-apps/api/core.js';

export async function invoke(command, args = {}, options) {
  const rows = window.__remoteUiRows ?? (window.__remoteUiRows = []);
  const now = Math.floor(Date.now() / 1000);
  if (command === 'workspace_snapshot') {
    rows.push({command, simulated: true});
    return {
      runtime: {schema_version: 1, version: '0.2.0', edition: 'personal', slots: ['residential_broadband', 'account_service', 'kyc_service'], automatic_updates: false, ads_enabled: false},
      accounts: {current: null, profiles: [], claude_running: false, action_blocker: null},
      preferences: {ads: {slots_enabled: [false, false, false], request_purpose_acknowledged: false}, updates_paused_until: null},
      check_report: null, ads_manifest: null,
      update: {paused: false, state: 'idle', message: 'synthetic', next_check_at: null, candidate_version: null, can_download: false, can_install: false}, pending_materials: []
    };
  }
  if (command === 'check_environment') {
    rows.push({command, simulated: true});
    return {schema_version: 1, observed_at: now, completed_at: now, duration_ms: 0, network_generation: 'synthetic', checks: [], summary: {passed: 0, advisory: 0, blocking: 14}, launch: {allowed: false, reasons: ['synthetic']}};
  }
  try {
    const value = await actualInvoke(command, args, options);
    if (command.startsWith('publisher_remote_') || command === 'create_ad_signing_key') rows.push({command, ok: true, channel: args?.channel ?? null});
    return value;
  } catch (error) {
    if (command.startsWith('publisher_remote_') || command === 'create_ad_signing_key') rows.push({command, ok: false, channel: args?.channel ?? null, error: String(error)});
    throw error;
  }
}
