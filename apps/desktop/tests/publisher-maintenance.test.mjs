import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import test from 'node:test';
import ts from 'typescript';
import {parse} from 'svelte/compiler';

const source = fs.readFileSync(new URL('../src/pages/Operations.svelte', import.meta.url), 'utf8');
const script = source.match(/<script[^>]*>([\s\S]*?)<\/script>/)[1];
const ast = ts.createSourceFile('Operations.ts', script, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
const names = ['clearMaintenance', 'previewCleanup', 'confirmCleanup', 'previewGc', 'confirmGc'];
const handlers = ast.statements.filter(node => ts.isFunctionDeclaration(node) && names.includes(node.name?.text));
assert.equal(handlers.length, names.length, 'exercise the actual maintenance handlers');
const state = ast.statements.find(node => ts.isVariableStatement(node)
  && node.declarationList.declarations.some(declaration => declaration.name.getText(ast) === 'retain'));
assert.ok(state, 'exercise production defaults');
const js = ts.transpileModule(state.getText(ast) + '\n' + handlers.map(node => node.getText(ast)).join('\n') + `
globalThis.handlers={${names.join(',')}};
globalThis.read=()=>({retain,releaseResolution,cleanupPreview,gcPreview,maintenanceOutcome});
globalThis.change=(patch)=>{
 if('retain' in patch) retain=patch.retain;
 if('releaseResolution' in patch) releaseResolution=patch.releaseResolution;
 if('cleanupPreview' in patch) cleanupPreview=patch.cleanupPreview;
 if('gcPreview' in patch) gcPreview=patch.gcPreview;
};`, {compilerOptions: {target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None}}).outputText;

function cleanup(overrides = {}) {
  return {digest: 'synthetic-index-plan', retain: 128, history_before: 256, history_after: 128,
    assets_before: 128, assets_after: 12, candidate_bytes: 1000, pending_blocks_commit: false,
    release_resolution: false, replaces_checkpoint: false, retained_history: [], removed_history_records: [], protected_assets: [], candidate_assets: [], message: 'synthetic index preview', ...overrides};
}
function gc(overrides = {}) {
  return {digest: 'synthetic-gc-plan', batch_id: 'synthetic-batch', eligible: ['synthetic-asset'],
    already_missing: [], blocked: [], referenced: [], eligible_bytes: 1000,
    pending_blocks_commit: false, blocked_details: [], message: 'synthetic file preview', ...overrides};
}
function harness(overrides = {}) {
  const calls = [], outcomes = [];
  const sandbox = {
    $state: value => value, Number, fixture: false, busy: '', error: '', publisher: null,
    onoutcome: value => outcomes.push(value), errorMessage: error => error.message,
    invoke: async (command, args) => {
      calls.push({command, args});
      if(command==='preview_publisher_cleanup')return cleanup({retain: args.retain, release_resolution: args.releaseResolution});
      if(command==='preview_publisher_gc')return gc();
      if(command==='publisher_status')return {status: 'ready_local', history: []};
      return {completed: true, message: 'synthetic confirmed action', recovery: null};
    }, ...overrides,
  };
  vm.runInNewContext(js, sandbox);
  return {sandbox, calls, outcomes, ...sandbox.handlers, read: sandbox.read, change: sandbox.change};
}

test('production defaults are 128 and no implicit evidence release; preview only reads', async () => {
  const h = harness();
  assert.equal(h.read().retain, 128);
  assert.equal(h.read().releaseResolution, false);
  await h.previewCleanup();
  assert.deepEqual(h.calls.map(call=>call.command), ['preview_publisher_cleanup']);
  assert.equal(h.calls[0].args.retain, 128);
  assert.equal(h.calls[0].args.releaseResolution, false);
  assert.equal(h.read().cleanupPreview.digest, 'synthetic-index-plan');
  assert.equal(h.outcomes.length, 0);
});

test('cancelling a preview does not execute either phase', async () => {
  const h = harness();
  await h.previewCleanup();h.clearMaintenance();await h.confirmCleanup();await h.confirmGc();
  assert.equal(h.calls.length, 1);
  assert.equal(h.read().cleanupPreview, null);
  assert.equal(h.read().gcPreview, null);
});

test('first confirmation commits the bound index plan once and never starts GC', async () => {
  const h = harness();
  await h.previewCleanup();await h.confirmCleanup();await h.confirmCleanup();
  assert.deepEqual(h.calls.map(call=>call.command), ['preview_publisher_cleanup','commit_publisher_cleanup','publisher_status']);
  assert.equal(h.calls[1].args.digest, 'synthetic-index-plan');
  assert.equal(h.read().cleanupPreview, null);
  assert.equal(h.read().gcPreview, null);
  assert.equal(h.outcomes.length, 1);
});

test('changed options and known pending state invalidate index execution', async () => {
  for(const change of [{retain: 127}, {releaseResolution: true}, {cleanupPreview: cleanup({pending_blocks_commit: true})}]) {
    const h = harness();await h.previewCleanup();h.change(change);await h.confirmCleanup();
    assert.equal(h.calls.length, 1);
  }
  for(const retain of [-1,257,1.5,NaN]) {
    const h=harness();h.change({retain});await h.previewCleanup();
    assert.equal(h.calls.length, 0);assert.match(h.sandbox.error,/整数/);
  }
});

test('second preview and a separate confirmation are required for file GC', async () => {
  const h = harness();await h.confirmGc();assert.equal(h.calls.length,0);
  await h.previewGc();assert.deepEqual(h.calls.map(call=>call.command),['preview_publisher_gc']);
  await h.confirmGc();await h.confirmGc();
  assert.deepEqual(h.calls.map(call=>call.command),['preview_publisher_gc','commit_publisher_gc','publisher_status']);
  assert.equal(h.calls[1].args.digest,'synthetic-gc-plan');
  assert.equal(h.read().gcPreview,null);
});

test('GC pending and duplicate clicks cannot bypass the confirmation gate', async () => {
  const h=harness();h.change({gcPreview:gc({pending_blocks_commit:true})});await h.confirmGc();
  assert.equal(h.calls.length,0);
  let complete;
  h.change({gcPreview:gc()});
  h.sandbox.invoke=async command=>{h.calls.push({command});if(command==='commit_publisher_gc')await new Promise(resolve=>complete=resolve);return {completed:true,message:'synthetic',recovery:null};};
  const first=h.confirmGc();await h.confirmGc();assert.equal(h.calls.length,1);complete();await first;
  assert.equal(h.calls.filter(call=>call.command==='commit_publisher_gc').length,1);
});

test('backend rejection clears the consumed plan and never invents completion', async () => {
  const h=harness();h.change({cleanupPreview:cleanup()});
  h.sandbox.invoke=async()=>{throw new Error('synthetic stale preview');};
  await h.confirmCleanup();
  assert.equal(h.outcomes.length,0);assert.equal(h.read().cleanupPreview,null);
  assert.equal(h.sandbox.error,'synthetic stale preview');assert.equal(h.sandbox.busy,'');
});

test('fixture and busy handlers perform zero IPC', async () => {
  for(const overrides of [{fixture:true},{busy:'synthetic running'}]) {
    const h=harness(overrides);h.change({cleanupPreview:cleanup(),gcPreview:gc()});
    await h.previewCleanup();await h.confirmCleanup();await h.previewGc();await h.confirmGc();
    assert.equal(h.calls.length,0);
  }
});

test('production markup connects both previews, separate confirmations and cancellation', () => {
  const parsed=parse(source);
  const handlers=[];
  function visit(node){if(!node||typeof node!=='object')return;if(node.type==='Attribute'&&node.name==='onclick')handlers.push(source.slice(node.start,node.end));for(const value of Object.values(node)){if(Array.isArray(value))value.forEach(visit);else if(value&&typeof value==='object')visit(value);}}
  visit(parsed.html);
  for(const name of names)assert.ok(handlers.some(handler=>handler.includes(name)),name);
});
