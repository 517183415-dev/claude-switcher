import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import ts from 'typescript';

const source = fs.readFileSync(new URL('../src/pages/Operations.svelte', import.meta.url), 'utf8');
const script = source.match(/<script[^>]*>([\s\S]*?)<\/script>/)[1];
const ast = ts.createSourceFile('Operations.ts', script, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
const declaration = ast.statements.find(node => ts.isFunctionDeclaration(node) && node.name?.text === 'publish');
assert.ok(declaration, 'test must exercise the production publish handler');
const js = ts.transpileModule(declaration.getText(ast) + '\nglobalThis.publish = publish;', {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None },
}).outputText;

function harness(overrides = {}) {
  const calls = [], outcomes = [];
  const result = { completed: false, message: '合成终态，未记入已验证历史', recovery: '请编辑新的草稿' };
  const sandbox = {
    fixture: false, busy: '', draft: {}, error: '', publishChannel: 'production', publisher: null,
    onoutcome: value => outcomes.push(value), errorMessage: error => error.message,
    invoke: async (command, args) => {
      calls.push({ command, args });
      if (command === 'publish_ad') return result;
      return { status: 'resolution_recorded', message: result.message, history: [] };
    },
    ...overrides,
  };
  vm.runInNewContext(js, sandbox);
  return { sandbox, calls, outcomes, result };
}

test('non-completed terminal outcome is delivered and status is refreshed', async () => {
  const h = harness();
  await h.sandbox.publish('production');
  assert.deepEqual(h.calls.map(call => call.command), ['publish_ad', 'publisher_status']);
  assert.equal(h.outcomes[0], h.result);
  assert.equal(h.outcomes[0].completed, false);
  assert.equal(h.sandbox.publisher.status, 'resolution_recorded');
  assert.equal(h.sandbox.busy, '');
  assert.equal(h.sandbox.publishChannel, null);
});

test('status refresh failure retains the delivered terminal outcome', async () => {
  const h = harness();
  h.sandbox.invoke = async command => {
    if (command === 'publish_ad') return h.result;
    throw new Error('synthetic status read failure');
  };
  await h.sandbox.publish('preview');
  assert.equal(h.outcomes[0], h.result);
  assert.equal(h.outcomes[0].completed, false);
  assert.equal(h.sandbox.error, 'synthetic status read failure');
  assert.equal(h.sandbox.busy, '');
});

test('ordinary backend failure does not become a terminal outcome', async () => {
  const h = harness({ invoke: async () => { throw new Error('synthetic pending remains'); } });
  await h.sandbox.publish('preview');
  assert.equal(h.outcomes.length, 0);
  assert.equal(h.sandbox.error, 'synthetic pending remains');
  assert.equal(h.sandbox.busy, '');
});

test('fixture, busy and missing draft do not invoke IPC', async () => {
  for (const overrides of [{ fixture: true }, { busy: 'synthetic active' }, { draft: null }]) {
    const h = harness(overrides);
    await h.sandbox.publish('production');
    assert.equal(h.calls.length, 0);
    assert.ok(h.outcomes.every(result => result.completed === false));
  }
});
