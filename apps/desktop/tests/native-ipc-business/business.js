import { verifyDenial } from './denials.mjs';
// This page is embedded only in the compile-time ipc-business fixture.
(async () => {
  const invoke = window.__TAURI_INTERNALS__.invoke;
  const denials = [];
  const responses = {};
  let error = null;
  let ok = false;
  try {
    const info = await invoke('ipc_business_info');
    if (info.fixture !== 'ipc-business' || info.ordinary_runtime !== false) throw new Error('wrong runtime');
    for (const command of ['ipc_business_forbidden', 'workspace_snapshot', 'account_action', 'software_update', 'open_date_settings']) {
      try { await invoke(command); }
      catch (reason) { denials.push(verifyDenial(command, reason, command === 'ipc_business_forbidden')); continue; }
      throw new Error(`unexpectedly allowed: ${command}`);
    }
    if (info.persistence) {
      const input = { ads: { slots_enabled: info.edition === 'personal' ? [true, false, true] : [false, true, false], request_purpose_acknowledged: true }, updates_paused_until: info.edition === 'personal' ? 2000000001 : 2000000002 };
      await invoke('ipc_business_preferences_snapshot', { label: `persistence-${info.persistence}:before` });
      const value = info.persistence === 'write' ? await invoke('save_preferences', { preferences: input }) : info.saved;
      responses.persistence = { action: info.persistence, value };
      await invoke('ipc_business_preferences_snapshot', { label: `persistence-${info.persistence}:after` });
    } else if (info.preferences) {
      const pref = (slots, ack, paused = null) => ({ ads: { slots_enabled: slots, request_purpose_acknowledged: ack }, updates_paused_until: paused });
      const off = pref([false, false, false], false);
      const cases = [['all_off_unconfirmed', { preferences: off }, true]];
      for (let slot = 0; slot < 3; slot++) {
        const slots = [false, false, false]; slots[slot] = true;
        cases.push([`unconfirmed_${slot}`, { preferences: pref(slots, false) }, false]);
      }
      for (let slot = 0; slot < 3; slot++) {
        const slots = [false, false, false]; slots[slot] = true;
        cases.push([`enable_${slot}`, { preferences: pref(slots, true) }, true]);
      }
      cases.push(['all_off_again', { preferences: off }, true]);
      const bad = (id, preferences) => cases.push([id, { preferences }, false]);
      bad('unknown_outer', { ...off, unknown: true });
      bad('unknown_ads', { ...off, ads: { ...off.ads, unknown: true } });
      cases.push(['missing_argument', {}, false]);
      bad('missing_ads', { updates_paused_until: null });
      bad('missing_slots', { ads: { request_purpose_acknowledged: false }, updates_paused_until: null });
      bad('missing_ack', { ads: { slots_enabled: [false, false, false] }, updates_paused_until: null });
      bad('short_slots', pref([false, false], false));
      bad('long_slots', pref([false, false, false, false], false));
      bad('wrong_preferences_type', []);
      bad('wrong_ads_type', { ...off, ads: [] });
      bad('wrong_slots_type', pref('false', false));
      bad('wrong_slot_type', pref([false, 1, false], false));
      bad('wrong_ack_type', pref([false, false, false], 'false'));
      bad('wrong_updates_type', pref([false, false, false], false, -1));
      responses.preferences = [];
      for (const [id, input, accepted] of cases) {
        await invoke('ipc_business_preferences_snapshot', { label: `${id}:before` });
        let value = null, reason = null;
        try { value = await invoke('save_preferences', input); }
        catch (failure) { reason = String(failure); }
        responses.preferences.push({ id, input, accepted: reason === null, value, error: reason });
        await invoke('ipc_business_preferences_snapshot', { label: `${id}:after` });
        if ((reason === null) !== accepted) throw new Error(`unexpected preference result: ${id}`);
      }
    } else if (!info.preflight) {
      responses.preview = await invoke('activity_log');
      responses.exported = await invoke('export_activity');
      responses.after = await invoke('activity_log');
      if (!Array.isArray(responses.preview) || responses.preview.length !== 2) throw new Error('activity response missing');
    }
    ok = true;
  } catch (reason) { error = String(reason); }
  document.getElementById('status').textContent = ok ? '日志业务 IPC 验证完成' : error;
  await invoke('ipc_business_finish', { ok, responses, denials, error });
})();
