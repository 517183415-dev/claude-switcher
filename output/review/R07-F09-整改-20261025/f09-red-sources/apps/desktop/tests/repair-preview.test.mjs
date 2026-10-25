import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,unlinkSync} from 'node:fs';
import {test} from 'node:test';
import vm from 'node:vm';
import ts from 'typescript';
import {compile} from 'svelte/compiler';
import {render} from 'svelte/server';
const baseline=process.env.REPAIR_BASELINE==='1';
const source=readFileSync(new URL(baseline?'../../../docs/acceptance/evidence/e06/core-repair-preview-baseline/apps__desktop__src__components__RegionSettings.svelte':'../src/components/RegionSettings.svelte',import.meta.url),'utf8');
const app=readFileSync(new URL(baseline?'../../../docs/acceptance/evidence/e06/core-repair-preview-baseline/apps__desktop__src__App.svelte':'../src/App.svelte',import.meta.url),'utf8');
const preview={can_execute:true,message:'仅用户配置需要修改',original:{country:'US',timezone:'America/Los_Angeles',locale:'en_US'},target:{country:'US',timezone:'America/Los_Angeles',locale:'en_US'},network_service:'Wi-Fi',original_ipv6:'关闭',target_ipv6:'关闭',user_changes:[{id:'locale',state:'ready',original:'zh_CN',target:'en_US',message:'系统地区格式'},{id:'client_language',state:'ready',original:'zh-CN',target:'en-US',message:'Claude 界面语言'}],recovery_required:false};
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return {promise,resolve,reject};};
function component(invoke,environment=true,props={}){
 let destroy;let next=0;
 const script=source.split('<script lang="ts">')[1].split('</script>')[0].replace(/^import .*?;\s*$/gm,'');
 const sandbox={invoke,errorMessage:String,previewRegionRepair:()=>preview,$state:v=>v,$props:()=>({environment,fixture:false,onoutcome:()=>{},...props}),onMount:()=>{},onDestroy:f=>destroy=f,crypto:{randomUUID:()=>`request-${++next}`}};
 const primary=source.match(/<button class="outline"[^>]*onclick=\{([^}]+)\}>\{environment/)[1];
 vm.runInNewContext(ts.transpileModule(script+'\nglobalThis.test={plan,execute,cancelPlan,recover,primary:()=>('+primary+')(),setSelected:value=>{selected=value},get busy(){return busy},get preview(){return preview},get error(){return error}};',{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText,sandbox);
 return {...sandbox.test,get preview(){return sandbox.test.preview},get error(){return sandbox.test.error},get busy(){return sandbox.test.busy},destroy:()=>destroy()};
}
test('environment buttons use full shared preview and retain current region',async()=>{
 assert.match(app,/<RegionSettings[^>]*environment/);
 assert.doesNotMatch(app,/perform\('execute_repair'/);
 const calls=[];const c=component(async(command,args)=>{calls.push([command,args]);return preview;});
 await c.plan();assert.equal(c.preview,preview);assert.equal(calls.length,1);assert.equal(calls[0][0],'preview_repair');assert.equal(calls[0][1].region,undefined);assert.ok(calls[0][1].requestId);
});
test('full production preview renders unchanged system values and all user original/target values',async()=>{
 const start=source.indexOf('{#if preview}<div class="region-preview">');const end=source.indexOf('{/if}</section>',start);assert.ok(start>=0&&end>start);
 const fragment=source.slice(start,end+5);const code=`<script>let {preview}=$props();const itemNames={locale:'系统地区格式',client_language:'Claude 界面语言'};const stateLabel=s=>s;let busy='',fixture=false,parentBusy=false;const recover=()=>{},cancelPlan=()=>{},execute=()=>{};</script>${fragment}`;
 const file=new URL(`./.repair-preview-${process.pid}.mjs`,import.meta.url);writeFileSync(file,compile(code,{generate:'server',filename:'RepairPreview.svelte'}).js.code);
 try{const {default:View}=await import(file.href);const html=render(View,{props:{preview}}).body;for(const value of ['America/Los_Angeles','IPv6 关闭','zh_CN','en_US','zh-CN','en-US','系统地区格式','Claude 界面语言'])assert.ok(html.includes(value),value);}finally{unlinkSync(file);}
});
test('execute requires visible valid preview, consumes its request ID once',async()=>{
 const calls=[];const c=component(async(command,args)=>{calls.push([command,args]);if(command==='preview_repair')return preview;return {completed:true};});
 await c.execute();assert.equal(calls.length,0);await c.plan();const id=calls[0][1].requestId;await c.execute();assert.equal(calls[1][0],'execute_repair');assert.equal(calls[1][1].requestId,id);await c.execute();assert.equal(calls.filter(x=>x[0]==='execute_repair').length,1);
});
test('failed repreview and cancel invalidate the old executable plan',async()=>{
 let attempts=0;const calls=[];const c=component(async(command,args)=>{calls.push([command,args]);if(command==='preview_repair'){if(++attempts===2)throw Error('preview failed');return preview;}return {completed:true};});
 await c.plan();await c.plan();assert.equal(c.preview,null);await c.execute();assert.equal(calls.filter(x=>x[0]==='execute_repair').length,0);await c.plan();await c.cancelPlan();assert.equal(c.preview,null);await c.execute();assert.equal(calls.filter(x=>x[0]==='execute_repair').length,0);assert.ok(calls.some(x=>x[0]==='cancel_repair'));
});
test('navigation/unmount and late old success cannot restore or execute a preview',async()=>{
 const old=deferred();const calls=[];let attempt=0;const c=component(async(command,args)=>{calls.push([command,args]);if(command==='preview_repair')return ++attempt===1?old.promise:preview;return {completed:true};});
 const pending=c.plan();await c.cancelPlan();await c.plan();const fresh=c.preview;old.resolve({...preview,message:'stale'});await pending;assert.equal(c.preview,fresh);c.destroy();assert.equal(c.preview,null);await c.execute();assert.equal(calls.filter(x=>x[0]==='execute_repair').length,0);
});

test('blocked or recovery-required preview cannot execute',async()=>{
 for(const denied of [{...preview,can_execute:false},{...preview,recovery_required:true}]){
  const calls=[];const c=component(async(command,args)=>{calls.push([command,args]);return denied;});
  await c.plan();await c.execute();assert.equal(calls.filter(x=>x[0]==='execute_repair').length,0);
 }
});

test('late recovery completion after navigation cannot create a fresh executable plan',async()=>{
 const recovery=deferred();const calls=[];
 const c=component(async(command,args)=>{calls.push([command,args]);if(command==='preview_repair')return {...preview,can_execute:false,recovery_required:true};if(command==='recover_repair')return recovery.promise;return {completed:true};});
 await c.plan();const pending=c.recover();c.destroy();recovery.resolve({completed:true});await pending;await c.plan();assert.equal(c.preview,null);assert.equal(calls.filter(x=>x[0]==='preview_repair').length,1);
});


// R07 drives the actual markup handler and the actual App callback props.
function parent(invoke,{fixture=false}={}){
 const tag=app.match(/<RegionSettings[^>]*\benvironment\b[^>]*\/>/)[0];
 const callbackProps=[...tag.matchAll(/(onoutcome|onrepairbusy|onrepairsuccess)=\{([^}]+)\}/g)].map(([,name,expression])=>`${name}:${expression}`);
 const busyExpression=tag.match(/parentBusy=\{([^}]+)\}/)?.[1]??'false';
 const script=app.split('<script lang="ts">')[1].split('</script>')[0].replace(/^\s*import .*?;\s*$/gm,'').replaceAll('import.meta.env.VITE_EDITION',"'public'").replaceAll('import.meta.env.DEV','true').replaceAll('import.meta.env.VITE_IPC_SMOKE',"'0'");
 const sandbox={invoke,errorMessage:String,$state:v=>v,$derived:v=>v,$effect:()=>{},onMount:()=>{},onDestroy:()=>{},slots:[],URLSearchParams,location:{search:fixture?'?fixture=1':''},tick:async()=>{},Date,Set};
 const access=`globalThis.test={runChecks,perform,refresh,cancelCheck,seed:value=>{snapshot=value;loading=false},get report(){return snapshot?.check_report},get busy(){return busy},get repairBusy(){return typeof repairBusy==='undefined'?false:repairBusy},get suppressed(){return suppressReport},get outcome(){return outcome},get error(){return error},get props(){return {${callbackProps.join(',')},get parentBusy(){return ${busyExpression}}}}};`;
 vm.runInNewContext(ts.transpileModule(script+'\n'+access,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText,sandbox);
 return sandbox.test;
}
const OLD={id:'OLD',summary:{passed:14,advisory:0,blocking:0},checks:[]};
const FRESH={...OLD,id:'FRESH'};
const settle=async()=>{for(let i=0;i<12;i++)await Promise.resolve();};
function linked(overrides={},options={}){
 const calls=[],outcomes=[];
 const invoke=async(command,args)=>{
  calls.push([command,args]);
  if(command in overrides)return overrides[command](args);
  if(command==='preview_repair'||command==='preview_region_change')return preview;
  if(command==='check_environment')return FRESH;
  if(command==='workspace_snapshot')return {check_report:OLD};
  if(command==='region_view')return {country:'US',timezone:'America/Los_Angeles',locale:'en_US'};
  if(['execute_repair','recover_repair','cancel_repair','cancel_check'].includes(command))return {completed:true,message:'synthetic result',recovery:null};
  throw Error('unexpected synthetic IPC: '+command);
 };
 const a=parent(invoke,options);a.seed({check_report:OLD});
 const props=a.props;
 const c=component(invoke,options.environment??true,{fixture:options.fixture??false,...props,onoutcome:result=>{outcomes.push(result);props.onoutcome(result);}});
 const counts=()=>Object.fromEntries(['preview_repair','preview_region_change','execute_repair','check_environment'].map(command=>[command,calls.filter(c=>c[0]===command).length]));
 const receipt=name=>console.log('R07_RECEIPT '+JSON.stringify({case:name,counts:counts(),calls,outcomes,parentBusy:a.busy,repairBusy:a.repairBusy,report:a.report,suppressed:a.suppressed,childBusy:c.busy,error:c.error}));
 return {a,c,calls,outcomes,counts,receipt};
}

test('f09 ordinary markup click clears OLD and performs preview/execute/full-check 1/1/1',async()=>{
 const pending=deferred();const h=linked({preview_repair:()=>pending.promise});
 const action=h.c.primary();const start={report:h.a.report,busy:h.a.repairBusy};pending.resolve(preview);await action;h.receipt('ordinary-success');
 assert.equal(start.report,null);assert.equal(start.busy,true);
 assert.deepEqual(h.counts(),{preview_repair:1,preview_region_change:0,execute_repair:1,check_environment:1});
 assert.equal(h.a.report,FRESH);assert.equal(h.a.repairBusy,false);
 const p=h.calls.find(c=>c[0]==='preview_repair'),e=h.calls.find(c=>c[0]==='execute_repair');assert.equal(p[1].requestId,e[1].requestId);
});
test('f09 ordinary triple clicks during preview and execute never replay',async()=>{
 const p=deferred(),e=deferred();const h=linked({preview_repair:()=>p.promise,execute_repair:()=>e.promise});
 const first=h.c.primary();await Promise.all([h.c.primary(),h.c.primary()]);p.resolve(preview);await settle();
 await Promise.all([h.c.primary(),h.c.primary(),h.c.primary()]);e.resolve({completed:true,message:'done'});await first;h.receipt('triple-click-both-stages');
 assert.deepEqual(h.counts(),{preview_repair:1,preview_region_change:0,execute_repair:1,check_environment:1});
});
test('f09 denied and failed ordinary actions suppress old evidence without full check',async()=>{
 for(const [name,overrides,execute] of [
  ['blocked',{preview_repair:()=>({...preview,can_execute:false})},0],
  ['recovery-required',{preview_repair:()=>({...preview,recovery_required:true})},0],
  ['preview-error',{preview_repair:()=>{throw Error('synthetic preview failure')}},0],
  ['not-completed',{execute_repair:()=>({completed:false,message:'incomplete'})},1],
  ['execute-error',{execute_repair:()=>{throw Error('synthetic execute failure')}},1],
 ]){
  const h=linked(overrides);await h.c.primary();await h.a.refresh();h.receipt(name);
  assert.equal(h.counts().execute_repair,execute,name);assert.equal(h.counts().check_environment,0,name);
  assert.equal(h.a.report,null,name);assert.equal(h.a.suppressed,true,name);assert.equal(h.a.repairBusy,false,name);
  assert.equal(h.outcomes.some(x=>x.completed),false,name);
 }
});
test('f09 cancel preview prevents execute and keeps OLD suppressed',async()=>{
 const p=deferred();const h=linked({preview_repair:()=>p.promise});const action=h.c.primary();await h.c.cancelPlan();p.resolve(preview);await action;await h.a.refresh();h.receipt('cancel-preview');
 assert.equal(h.counts().execute_repair,0);assert.equal(h.counts().check_environment,0);assert.equal(h.a.report,null);assert.equal(h.a.repairBusy,false);assert.equal(h.c.preview,null);
});
test('f09 destroy during execute holds parent guard until settlement and suppresses stale completion',async()=>{
 const e=deferred();const h=linked({execute_repair:()=>e.promise});const action=h.c.primary();await settle();h.c.destroy();
 await h.a.perform('launch_checked');await h.a.runChecks();const held=h.a.repairBusy;
 e.resolve({completed:true,message:'late'});await action;await h.a.refresh();h.receipt('destroy-execute');
 assert.equal(held,true);assert.equal(h.counts().execute_repair,1);assert.equal(h.counts().check_environment,0);assert.equal(h.calls.some(x=>x[0]==='launch_checked'),false);
 assert.equal(h.a.repairBusy,false);assert.equal(h.a.report,null);assert.equal(h.outcomes.length,0);assert.equal(h.calls.some(x=>x[0]==='region_view'),false);
});
test('f09 cancel execute ignores outcome and cannot release another component repair',async()=>{
 const e=deferred();const h=linked({execute_repair:()=>e.promise});const action=h.c.primary();await settle();await h.c.cancelPlan();e.resolve({completed:true});await action;
 // A disposed component must not release the subsequently acquired parent guard.
 const p=deferred();const props=h.a.props;const newer=component(async command=>command==='preview_repair'?p.promise:{completed:true},true,props);
 const next=newer.primary();h.c.destroy();const held=h.a.repairBusy;p.resolve({...preview,can_execute:false});await next;h.receipt('cancel-execute-new-owner');
 assert.equal(held,true);assert.equal(h.outcomes.length,0);assert.equal(h.counts().check_environment,0);assert.equal(h.a.report,null);
});
test('f09 parent busy blocks repair and repair blocks launch/check/account actions',async()=>{
 const old=deferred();const h=linked({check_environment:()=>old.promise});const checking=h.a.runChecks();await h.c.primary();
 assert.equal(h.counts().preview_repair,0);old.resolve(FRESH);await checking;
 const p=deferred();const k=linked({preview_repair:()=>p.promise});const fixing=k.c.primary();await k.a.runChecks();await k.a.perform('launch_checked');await k.a.perform('account_action',{action:'select_b'});
 p.resolve({...preview,can_execute:false});await fixing;k.receipt('parent-concurrency');
 assert.equal(k.calls.some(x=>['check_environment','launch_checked','account_action'].includes(x[0])),false);assert.equal(k.a.repairBusy,false);
});
test('f09 old check and snapshot races cannot replace cleared or FRESH evidence',async()=>{
 const old=deferred(),snapshot=deferred(),p=deferred();let checks=0;
 const h=linked({check_environment:()=>++checks===1?old.promise:FRESH,workspace_snapshot:()=>snapshot.promise,preview_repair:()=>p.promise});
 const refresh=h.a.refresh(),checking=h.a.runChecks();const cancelling=h.a.cancelCheck();
 // cancelCheck awaits refresh: release that snapshot before starting repair.
 snapshot.resolve({check_report:OLD});await refresh;await cancelling;await settle();
 const fixing=h.c.primary();old.resolve(OLD);await checking;const during=h.a.report;p.resolve(preview);await fixing;h.receipt('old-check-snapshot-races');
 assert.equal(during,null);assert.equal(h.a.report,FRESH);assert.equal(h.counts().check_environment,2);
});
test('f09 selected jp remains visible preview until explicit same-id execution',async()=>{
 const h=linked({}, {environment:false});h.c.setSelected('jp');await h.c.primary();const p=h.calls.find(x=>x[0]==='preview_region_change');
 assert.equal(p[1].region,'jp');assert.equal(h.c.preview,preview);assert.equal(h.counts().execute_repair,0);
 await h.c.execute();h.receipt('region-jp-explicit');assert.equal(h.calls.find(x=>x[0]==='execute_repair')[1].requestId,p[1].requestId);assert.equal(h.counts().execute_repair,1);assert.equal(h.counts().check_environment,1);
});
test('f09 recovery loads fresh preview but never auto executes or full checks',async()=>{
 let n=0;const h=linked({preview_repair:()=>++n===1?{...preview,recovery_required:true}:preview});
 await h.c.primary();await h.c.recover();h.receipt('recovery-fresh-preview');
 assert.equal(h.counts().preview_repair,2);assert.equal(h.counts().execute_repair,0);assert.equal(h.counts().check_environment,0);assert.equal(h.c.preview,preview);assert.equal(h.a.report,null);
});
test('f09 fixture ordinary click is zero IPC',async()=>{
 const h=linked({}, {fixture:true});await h.c.primary();await h.c.execute();await h.c.recover();h.receipt('fixture-zero-ipc');assert.equal(h.calls.length,0);
});
