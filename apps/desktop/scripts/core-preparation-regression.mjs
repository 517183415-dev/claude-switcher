// Execute the actual App script handlers without a browser or native IPC.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import ts from '../node_modules/typescript/lib/typescript.js';
const source=fs.readFileSync(process.env.APP_SOURCE_PATH??new URL('../src/App.svelte',import.meta.url),'utf8').split('<script lang="ts">')[1].split('</script>')[0];
const report={summary:{passed:14,advisory:0,blocking:0},checks:[]};
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};};
function app(invoke,fixture=false){
 const code=source.replace(/^\s*import .*?;\s*$/gm,'').replaceAll('import.meta.env',{toString:()=>`({DEV:true,VITE_EDITION:'personal',VITE_IPC_SMOKE:'0'})`});
 let mounted;
 const sandbox={invoke,previewSnapshot:()=>({runtime:{edition:'personal'},check_report:null}),setInterval:()=>1,clearInterval:()=>{},slots:[],errorMessage:String,$state:v=>v,$derived:v=>v,$effect:()=>{},onMount:callback=>{mounted=callback;},tick:async()=>{},location:{search:fixture?'?fixture=1':''},URLSearchParams,document:{getElementById:()=>null},Set,console};
 vm.runInNewContext(ts.transpileModule(code+`\nglobalThis.test={runChecks,cancelCheck,navigate,refresh,perform,get snapshot(){return snapshot},set snapshot(v){snapshot=v},get busy(){return busy},get error(){return error},get outcome(){return outcome}};`,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText,sandbox);
 sandbox.test.snapshot={check_report:report};sandbox.test.mount=()=>mounted();return sandbox.test;
}
async function checks(){
 const pending=deferred();const calls=[];
 const a=app(async command=>{calls.push(command);if(command==='check_environment')return pending.promise;if(command==='cancel_check')return {completed:true,message:'已取消',recovery:null};if(command==='workspace_snapshot')return {check_report:report};throw Error(command);});
 const running=a.runChecks();
 assert.equal(a.snapshot.check_report,null,'starting a new check must clear old 14-item summary immediately');
 await a.cancelCheck();
 assert.equal(a.snapshot.check_report,null,'cancel refresh must not restore cached report');
 pending.resolve(report);await running;
 assert.equal(a.snapshot.check_report,null,'late canceled success must not restore report');
 assert.notEqual(a.outcome?.message,'检查完成：14 项通过 · 0 项提示 · 0 项阻止启动。');
 assert.ok(calls.includes('workspace_snapshot'),'cancel refreshes backend state');
 const b=app(async command=>{if(command==='check_environment')throw '隔离检查失败';if(command==='workspace_snapshot')return {check_report:report};throw Error(command);});
 await b.runChecks();assert.equal(b.snapshot.check_report,null,'failed refresh must not restore old report');assert.equal(b.error,'隔离检查失败');
 const stale=deferred(),fresh=deferred();let checkCount=0;
 const c=app(async command=>{if(command==='check_environment')return ++checkCount===1?stale.promise:fresh.promise;if(command==='cancel_check')return {completed:true,message:'已取消'};if(command==='workspace_snapshot')return {check_report:report};});
 const first=c.runChecks();await c.cancelCheck();const second=c.runChecks();stale.resolve(report);await first;assert.ok(c.busy,'late canceled finally must not clear the newer check busy state');assert.equal(c.snapshot.check_report,null);fresh.resolve(report);await second;assert.equal(c.snapshot.check_report,report);assert.equal(c.busy,'');
 const delayed=deferred();let requests=0;
 const d=app(async command=>{if(command==='workspace_snapshot')return ++requests===1?delayed.promise:{check_report:report};if(command==='check_environment')throw '隔离检查失败';});
 const oldRefresh=d.refresh();await d.runChecks();delayed.resolve({check_report:report});await oldRefresh;assert.equal(d.snapshot.check_report,null,'older snapshot response must not resurrect evidence');
 const fixture=app(async()=>{throw Error('fixture IPC forbidden');},true);await fixture.runChecks();assert.equal(fixture.snapshot.check_report,report);await fixture.navigate('accounts');
 const launch=deferred();const e=app(async command=>command==='launch_checked'?launch.promise:{check_report:report});const starting=e.perform('launch_checked');assert.equal(e.snapshot.check_report,null,'launch check must also clear old report');launch.resolve({completed:false,message:'启动门禁未通过'});await starting;assert.equal(e.snapshot.check_report,report,'launch refresh accepts only backend latest report');
 console.log('PASS: immediate invalidation, cancel late success, failure refresh, newer check race, stale snapshot race, fixture zero IPC');
}
async function legacyFlow(){
 const calls=[];
 const a=app(async(command,args)=>{calls.push([command,args]);if(command==='runtime_info')return {edition:'personal'};if(command==='workspace_snapshot')return {check_report:null};if(command==='check_environment')return report;throw Error(command);});
 const cleanup=a.mount();
 for(let i=0;i<20;i++)await Promise.resolve();
 assert.deepEqual(calls.map(c=>c[0]),['runtime_info','workspace_snapshot','check_environment'],'opening the normal application automatically checks the environment');
 cleanup();
 const f=app(async()=>{throw Error('fixture must not call native IPC');},true);
 f.mount()();for(let i=0;i<20;i++)await Promise.resolve();assert.equal(f.error,'');
 const pending=deferred(),actions=[];
 const b=app(async(command,args)=>{actions.push([command,args]);if(command==='account_action')return pending.promise;if(command==='workspace_snapshot')return {check_report:report};throw Error(command);});
 const switching=b.perform('account_action',{action:'select_b'});
 assert.equal(b.snapshot.check_report,null,'switching clears the old account environment result');
 pending.resolve({completed:true,message:'已切换并启动'});await switching;
 assert.equal(b.outcome.completed,true);
 assert.deepEqual(actions.map(c=>c[0]),['account_action','workspace_snapshot'],'first B selection has no separate initialization or migration-proof step');
 assert.deepEqual(JSON.parse(JSON.stringify(actions[0][1])),{action:'select_b'});
 console.log('PASS: automatic opening check, fixture zero IPC, single-action first-B switch and launch');
}
await checks();await legacyFlow();
