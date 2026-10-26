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
    if (!info.preflight) {
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
