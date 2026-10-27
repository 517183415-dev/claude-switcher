import assert from 'node:assert/strict';
import test from 'node:test';

// Tauri defines this property as non-writable and non-configurable.
// The test adapter must forward business calls without replacing it.
test('startup fixtures preserve the read-only Tauri transport and forward publisher calls', async () => {
  const calls = [];
  const transport = async (command, args, options) => {
    calls.push({command, args, options});
    if (command === 'publisher_remote_select') throw new Error('synthetic rejection');
    return {ok: true};
  };
  globalThis.window = {__TAURI_INTERNALS__: {}, __remoteUiRows: []};
  Object.defineProperty(window.__TAURI_INTERNALS__, 'invoke', {value: transport});
  const {invoke} = await import('./tauri-shim.js');
  assert.equal((await invoke('workspace_snapshot')).accounts.current, null);
  assert.equal((await invoke('check_environment')).launch.allowed, false);
  assert.deepEqual(await invoke('publisher_remote_save', {channel: 'preview'}, {headers: {}}), {ok: true});
  await assert.rejects(invoke('publisher_remote_select', {channel: 'preview'}), /synthetic rejection/);
  assert.equal(window.__TAURI_INTERNALS__.invoke, transport);
  assert.deepEqual(calls.map(call => call.command), ['publisher_remote_save', 'publisher_remote_select']);
  assert.deepEqual(window.__remoteUiRows.map(row => [row.command, row.simulated ?? false, row.ok ?? null]), [
    ['workspace_snapshot', true, null], ['check_environment', true, null],
    ['publisher_remote_save', false, true], ['publisher_remote_select', false, false]
  ]);
});
