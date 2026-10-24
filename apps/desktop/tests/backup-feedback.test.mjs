import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import {test} from 'node:test';
import ts from 'typescript';
import {parse} from 'svelte/compiler';

const source = readFileSync(new URL('../src/pages/Operations.svelte', import.meta.url), 'utf8');
function page(invoke, fixture = false) {
  let script = source.match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  script = script.replace(/^import .*;$/gm, '');
  const js = ts.transpileModule(script, {compilerOptions:{target:ts.ScriptTarget.ESNext, module:ts.ModuleKind.None}}).outputText;
  const outcomes = [];
  const globals = {outcomes,invoke, onMount:()=>{}, $props:()=>({fixture,onoutcome:result=>outcomes.push(result)}),
    $state:Object.assign(value=>value,{snapshot:value=>value}), $derived:value=>value,
    systemTimeZone:()=> 'UTC', slots:[], errorMessage:e=>String(e), crypto:globalThis.crypto};
  const context = vm.createContext(globals);
  return vm.runInContext(`${js}\n({backup,restoreBackup,publisherAction,setPassword:value=>passphrase=value,state:()=>({backupOutcome,backupError,passphrase,busy}),outcomes})`, context);
}
function fileEvent(size=10) { return {currentTarget:{value:'selected',files:[{size,arrayBuffer:async()=>new ArrayBuffer(2)}]}}; }
const saved = {completed:true,message:'已保存加密备份：test.csbackup',recovery:null};
const restored = {completed:false,message:'加密备份已恢复；等待下一次正式发布。',recovery:'先回读正式序号，再补齐素材并发布。'};

test('backup card renders local message and recovery in a polite status, with nearby errors', () => {
  parse(source); // Verify the actual Svelte template parses.
  const card = source.match(/<section class="panel"><h2>加密备份与恢复<\/h2>([\s\S]*?)<\/section>/)[1];
  assert.match(card, /role="status"[^>]*aria-live="polite"/);
  assert.match(card, /\{backupOutcome\.message\}/);
  assert.match(card, /\{backupOutcome\.recovery\}/);
  assert.match(card, /role="alert"[\s\S]*\{backupError\}/);
  assert.doesNotMatch(card, /backupOutcome\.completed/);
});
test('export shows the returned message and clears the password on success', async () => {
  const app = page(async command=>{assert.equal(command,'export_publisher_backup');return saved;});
  app.setPassword('synthetic-password');
  await app.backup();
  assert.equal(app.state().backupOutcome,saved);
  assert.equal(app.outcomes[0],saved);
  assert.equal(app.state().passphrase,'');
});
test('restore preserves a completed=false outcome and its recovery guidance', async () => {
  const calls=[];
  const app=page(async command=>{calls.push(command);return command==='restore_publisher_backup'?restored:{slots:[]};});
  app.setPassword('synthetic-password');
  const event=fileEvent();
  await app.restoreBackup(event);
  assert.equal(app.state().backupOutcome,restored);
  assert.equal(app.state().backupError,'');
  assert.equal(app.state().passphrase,'');
  assert.equal(event.currentTarget.value,'');
  assert.deepEqual(calls,['restore_publisher_backup','ad_draft_read']);
});
test('a new export clears stale feedback while pending and reports failures locally', async () => {
  let rejectNext;
  let count=0;
  const app=page(()=>++count===1?Promise.resolve(saved):new Promise((_,reject)=>{rejectNext=reject;}));
  app.setPassword('synthetic-password');await app.backup();
  app.setPassword('synthetic-password');const pending=app.backup();
  assert.equal(app.state().backupOutcome,null);
  assert.equal(app.state().backupError,'');
  rejectNext(new Error('synthetic backup error'));await pending;
  assert.match(app.state().backupError,/synthetic backup error/);
  assert.equal(app.state().passphrase,'synthetic-password');
});
test('restore validation clears stale feedback and reports the error beside the card', async () => {
  const app=page(async()=>saved);
  app.setPassword('synthetic-password');await app.backup();
  const event=fileEvent();await app.restoreBackup(event);
  assert.equal(app.state().backupOutcome,null);
  assert.match(app.state().backupError,/12/);
  assert.equal(event.currentTarget.value,'');
});
test('restore rejection stays local and clears the file selection', async () => {
  const app=page(async()=>{throw Error('synthetic restore error');});
  app.setPassword('synthetic-password');const event=fileEvent();await app.restoreBackup(event);
  assert.match(app.state().backupError,/synthetic restore error/);
  assert.equal(event.currentTarget.value,'');
});
test('fixture backup handlers make zero IPC calls even when invoked directly', async () => {
  let calls=0;const app=page(async()=>{calls++;return saved;},true);
  app.setPassword('synthetic-password');await app.backup();await app.restoreBackup(fileEvent());
  await app.publisherAction('open_publisher_backup_folder','opening',true);
  assert.equal(calls,0);
});

test('restore success remains visible if the subsequent draft refresh fails', async () => {
  const app=page(async command=>{if(command==='restore_publisher_backup')return restored;throw Error('synthetic refresh error');});
  app.setPassword('synthetic-password');await app.restoreBackup(fileEvent());
  assert.equal(app.state().backupOutcome,restored);
  assert.match(app.state().backupError,/synthetic refresh error/);
  assert.equal(app.state().passphrase,'');
});
test('opening the backup folder clears old errors and shows its real outcome', async () => {
  let fail=true;
  const opened={completed:true,message:'已打开本机加密备份目录',recovery:null};
  const app=page(async command=>{if(fail)throw Error('synthetic old error');return command==='open_publisher_backup_folder'?opened:{history:[]};});
  app.setPassword('synthetic-password');await app.backup();
  assert.match(app.state().backupError,/synthetic old error/);
  fail=false;await app.publisherAction('open_publisher_backup_folder','opening',true);
  assert.equal(app.state().backupError,'');
  assert.equal(app.state().backupOutcome,opened);
});
