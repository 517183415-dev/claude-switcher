import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import ts from 'typescript';

const source = fs.readFileSync(new URL('../src/pages/Operations.svelte', import.meta.url), 'utf8');
const script = source.match(/<script[^>]*>([\s\S]*?)<\/script>/)[1];
const ast = ts.createSourceFile('Operations.ts', script, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
const handlers = ast.statements.filter(node => ts.isFunctionDeclaration(node)
  && ['imageInput', 'restoreBackup'].includes(node.name?.text)).map(node => node.getText(ast)).join('\n');
assert.equal(ast.statements.filter(node => ts.isFunctionDeclaration(node)
  && ['imageInput', 'restoreBackup'].includes(node.name?.text)).length, 2);

function harness({ reject = false, passphrase = 'synthetic-passphrase' } = {}) {
  const calls = [];
  const campaign = {};
  const sandbox = {
    Uint8Array, Array, structuredClone, campaign, fixture: false, busy: '', error: '',
    backupError: '', backupOutcome: null, passphrase, images: {}, draft: null,
    selected: 'residential_broadband', period: 0, dirty: false,
    $state: { snapshot: value => value }, previews: async () => {},
    onoutcome: () => {}, errorMessage: error => error.message,
    invoke: async (command, args) => {
      calls.push({ command, args });
      if (reject) throw new Error('synthetic backend rejection');
      if (command === 'import_ad_image') return { asset: { sha256: 'synthetic' }, preview_data_url: 'data:image/png;base64,' };
      if (command === 'ad_draft_read') return null;
      return { status: 'waiting' };
    },
  };
  const js = ts.transpileModule(handlers + '\nglobalThis.handlers = { imageInput, restoreBackup };', {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None },
  }).outputText;
  vm.runInNewContext(js, sandbox);
  return { calls, sandbox, ...sandbox.handlers };
}

function input(size) {
  let reads = 0;
  // Synthetic File metadata isolates the UI gate. Backend tests cover actual
  // 33/64 MiB ciphertext; this does not claim WebView serialization coverage.
  const target = { value: 'synthetic', files: [{ size, arrayBuffer: async () => {
    reads += 1;
    return Uint8Array.from([137, 80, 78, 71]).buffer;
  } }] };
  return { event: { currentTarget: target, target }, target, reads: () => reads };
}

test('image handler accepts the inclusive 1 MiB boundary', async () => {
  for (const size of [1024 * 1024 - 1, 1024 * 1024]) {
    const h = harness(); const file = input(size);
    await h.imageInput(file.event);
    assert.equal(h.calls.length, 1);
    assert.equal(h.calls[0].command, 'import_ad_image');
    assert.deepEqual(Array.from(h.calls[0].args.bytes), [137, 80, 78, 71]);
    assert.equal(file.reads(), 1);
    assert.equal(file.target.value, '');
    assert.equal(h.sandbox.busy, '');
  }
});

test('image handler rejects over 1 MiB before reading or invoking', async () => {
  for (const size of [1024 * 1024 + 1, 8 * 1024 * 1024]) {
    const h = harness(); const file = input(size);
    await h.imageInput(file.event);
    assert.equal(h.calls.length, 0, 'over-limit image must not invoke the backend');
    assert.equal(file.reads(), 0);
    assert.match(h.sandbox.error, /1 MiB/);
    assert.equal(file.target.value, '');
  }
});

test('restore handler accepts the inclusive 64 MiB boundary', async () => {
  const h = harness(); const file = input(64 * 1024 * 1024);
  await h.restoreBackup(file.event);
  assert.deepEqual(h.calls.map(call => call.command), ['restore_publisher_backup', 'ad_draft_read']);
  assert.equal(h.calls[0].args.passphrase, 'synthetic-passphrase');
  assert.equal(file.reads(), 1);
  assert.equal(h.sandbox.passphrase, '');
  assert.equal(h.sandbox.busy, '');
});

test('restore handler rejects oversized files and short passwords before invoke', async () => {
  for (const [size, passphrase] of [[64 * 1024 * 1024 + 1, 'synthetic-passphrase'], [100, 'short']]) {
    const h = harness({ passphrase }); const file = input(size);
    await h.restoreBackup(file.event);
    assert.equal(h.calls.length, 0);
    assert.equal(file.reads(), 0);
    assert.match(h.sandbox.backupError, /不超过 64 MiB/);
    assert.equal(file.target.value, '');
  }
});

test('real handlers report backend failure and release busy state', async () => {
  for (const name of ['imageInput', 'restoreBackup']) {
    const h = harness({ reject: true }); const file = input(100);
    await h[name](file.event);
    assert.equal(h.calls.length, 1);
    assert.equal(h.sandbox.error, 'synthetic backend rejection');
    assert.equal(h.sandbox.busy, '');
    assert.equal(file.target.value, '');
    assert.equal(h.sandbox.backupOutcome, null);
  }
});
