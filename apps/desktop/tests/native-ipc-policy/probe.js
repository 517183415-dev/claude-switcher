// Scripted frontend only. The executable retains the ordinary Rust entry and ACL.
// Never invoke launch, account changes, system repair, Keychain, or remote writes.
document.body.append(' · script loaded');
const plan = JSON.parse(document.body.dataset.plan);
const rows = [];
const invoke = (command, args = {}) => window.__TAURI_INTERNALS__.invoke(command, args);

async function attempt(command, args = {}) {
  document.body.append(` · ${command}`);
  try {
    return { command, ok: true, value: await invoke(command, args) };
  } catch (error) {
    return { command, ok: false, error: String(error) };
  }
}

async function run() {
  if (plan.phase === 'remote') {
    const bridge = typeof window.__TAURI_INTERNALS__?.invoke === 'function';
    const result = bridge ? await attempt('runtime_info') : { ok: false, error: 'bridge_not_injected' };
    const report = { phase: 'remote', edition: plan.edition, bridge, rows: [result] };
    location.replace(`${plan.endpoint}/remote-result?data=${encodeURIComponent(JSON.stringify(report))}`);
    return;
  }

  for (const command of ['runtime_info', 'cancel_check', 'create_migration_backup']) {
    rows.push(await attempt(command));
  }
  rows.push(await attempt('save_preferences', { preferences: plan.preferences }));
  for (const command of ['workspace_snapshot', 'activity_log', 'export_activity']) {
    rows.push(await attempt(command));
  }
  // Missing required arguments exercise command registration and deserialization
  // without entering business code. Only reviewed commands are included.
  for (const command of plan.invalidArguments) rows.push(await attempt(command));
  for (const command of plan.denied) rows.push(await attempt(command));

  const report = { phase: 'local', edition: plan.edition, rows };
  document.body.textContent = `IPC_REPORT:${JSON.stringify(report)}`;
}

run().catch(error => {
  document.body.textContent = `IPC_REPORT:${JSON.stringify({ phase: 'failure', error: String(error) })}`;
});
