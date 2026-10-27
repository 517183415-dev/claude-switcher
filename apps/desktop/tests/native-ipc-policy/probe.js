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
  if (plan.phase === 'remote' || plan.phase === 'other') {
    const bridge = typeof window.__TAURI_INTERNALS__?.invoke === 'function';
    if (bridge) {
      for (const command of plan.denied) {
        const result = await attempt(command);
        rows.push(result);
        // Stop before further calls if this context unexpectedly has permission.
        if (result.ok || !result.error.toLowerCase().includes('not allowed')) break;
      }
    }
    document.body.textContent = `IPC_REPORT:${JSON.stringify({
      phase: plan.phase, edition: plan.edition, url: location.href, bridge, rows
    })}`;
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
