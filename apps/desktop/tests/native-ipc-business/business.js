import { verifyDenial } from './denials.mjs';
import { runDraft } from './draft.mjs';
import { runDraftPersistence } from './draft-persistence.mjs';
import { runImages } from './images.mjs';
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
    if (info.backup_persistence === 'read') {
      const draft = await invoke('ad_draft_read');
      const campaigns = draft.slots.flatMap(slot => slot.campaigns);
      if (campaigns.length !== 1) throw new Error('restored image reference missing');
      const preview = await invoke('ad_image_preview', { sha256: campaigns[0].image.sha256 });
      responses.backup_persistence = { draft, preview };
    } else if (info.draft_persistence) {
      await runDraftPersistence(invoke, info.edition, info.draft_persistence, responses);
    } else if (info.images) {
      await runImages(invoke, info.edition, responses);
    } else if (info.draft) {
      await runDraft(invoke, info.edition, responses);
    } else if (info.backup) {
      if (info.edition === 'public') {
        responses.backup = [];
        for (const command of ['ipc_business_backup_export', 'ipc_business_backup_restore']) {
          try { await invoke(command, {}); }
          catch (reason) { responses.backup.push(verifyDenial(command, reason)); continue; }
          throw new Error('public backup command unexpectedly allowed');
        }
      } else {
        // Synthetic values exist only in this embedded page and native memory, never in receipts.
        const passphrase = 'synthetic-ipc-backup-passphrase';
        const encrypted = await invoke('ipc_business_backup_export', { passphrase });
        if (!Array.isArray(encrypted) || encrypted.length <= 76 || encrypted.length > 65536) throw new Error('backup export bound');
        responses.backup = [];
        for (const name of ['wrong_password', 'tampered', 'truncated', 'vault_failure', 'success']) {
          let bytes = encrypted.slice();
          if (name === 'tampered') bytes[bytes.length - 1] ^= 1;
          if (name === 'truncated') bytes = bytes.slice(0, 75);
          responses.backup.push(await invoke('ipc_business_backup_restore', {
            case: name, bytes, passphrase: name === 'wrong_password' ? 'synthetic-incorrect-passphrase' : passphrase
          }));
          bytes.fill(0);
        }
        encrypted.fill(0);
      }
    } else if (info.persistence) {
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
