import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

test('local IPC results remain readable without an HTTP or navigation channel', async () => {
  const plan = { phase: 'local', edition: 'public', preferences: {},
    invalidArguments: ['account_action'], denied: ['ad_draft_read'] };
  let complete;
  const reported = new Promise(resolve => { complete = resolve; });
  const body = { dataset: { plan: JSON.stringify(plan) }, append() {},
    set textContent(value) { complete(value); } };
  const commands = [];
  const context = vm.createContext({ document: { body },
    window: { __TAURI_INTERNALS__: { async invoke(command) {
      commands.push(command);
      if (command === 'ad_draft_read') throw new Error('not allowed by ACL');
      if (command === 'account_action') throw new Error('invalid args: action');
      return { command };
    } } }, location: { replace() { assert.fail('local report must not navigate'); } } });
  vm.runInContext(readFileSync(new URL('./probe.js', import.meta.url), 'utf8'), context);
  const text = await reported;
  assert.ok(text.startsWith('IPC_REPORT:'));
  const result = JSON.parse(text.slice('IPC_REPORT:'.length));
  assert.equal(result.phase, 'local');
  assert.equal(result.rows.length, 9);
  assert.deepEqual(result.rows.map(row => row.command), commands);
  assert.equal(result.rows.at(-1).ok, false);
  assert.match(result.rows.at(-1).error, /not allowed/);
});


for (const phase of ['remote', 'other']) {
  for (const failure of ['not allowed by ACL', 'invalid args: action', null]) {
    test(`${phase} boundary stops on non-ACL results: ${failure}`, async () => {
      const plan = { phase, edition: 'public', denied: ['runtime_info', 'account_action'] };
      let complete;
      const reported = new Promise(resolve => { complete = resolve; });
      const body = { dataset: { plan: JSON.stringify(plan) }, append() {},
        set textContent(value) { complete(value); } };
      const commands = [];
      const context = vm.createContext({ document: { body }, location: { href: 'http://127.0.0.1:12345/' },
        window: { __TAURI_INTERNALS__: { async invoke(command) {
          commands.push(command);
          if (failure) throw new Error(failure);
          return {};
        } } } });
      vm.runInContext(readFileSync(new URL('./probe.js', import.meta.url), 'utf8'), context);
      const result = JSON.parse((await reported).slice('IPC_REPORT:'.length));
      assert.equal(result.phase, phase);
      assert.equal(result.bridge, true);
      assert.deepEqual(result.rows.map(row => row.command), commands);
      assert.equal(commands.length, failure === 'not allowed by ACL' ? 2 : 1);
    });
  }
  test(`${phase} missing bridge cannot fabricate IPC rows`, async () => {
    let complete;
    const reported = new Promise(resolve => { complete = resolve; });
    const body = { dataset: { plan: JSON.stringify({ phase, edition: 'public', denied: ['runtime_info'] }) },
      append() {}, set textContent(value) { complete(value); } };
    vm.runInNewContext(readFileSync(new URL('./probe.js', import.meta.url), 'utf8'), {
      document: { body }, window: {}, location: { href: 'http://127.0.0.1:12345/' }
    });
    const result = JSON.parse((await reported).slice('IPC_REPORT:'.length));
    assert.equal(result.bridge, false);
    assert.deepEqual(result.rows, []);
  });
}
