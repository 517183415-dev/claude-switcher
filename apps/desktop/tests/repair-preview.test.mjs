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
function component(invoke,environment=true){
 let destroy;let next=0;
 const script=source.split('<script lang="ts">')[1].split('</script>')[0].replace(/^import .*?;\s*$/gm,'');
 const sandbox={invoke,errorMessage:String,previewRegionRepair:()=>preview,$state:v=>v,$props:()=>({environment,fixture:false,onoutcome:()=>{}}),onMount:()=>{},onDestroy:f=>destroy=f,crypto:{randomUUID:()=>`request-${++next}`}};
 vm.runInNewContext(ts.transpileModule(script+'\nglobalThis.test={plan,execute,cancelPlan,recover,get preview(){return preview},get error(){return error}};',{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText,sandbox);
 return {...sandbox.test,get preview(){return sandbox.test.preview},get error(){return sandbox.test.error},destroy:()=>destroy()};
}
test('environment buttons use full shared preview and retain current region',async()=>{
 assert.match(app,/<RegionSettings[^>]*environment/);
 assert.doesNotMatch(app,/perform\('execute_repair'/);
 const calls=[];const c=component(async(command,args)=>{calls.push([command,args]);return preview;});
 await c.plan();assert.equal(c.preview,preview);assert.equal(calls.length,1);assert.equal(calls[0][0],'preview_repair');assert.equal(calls[0][1].region,undefined);assert.ok(calls[0][1].requestId);
});
test('full production preview renders unchanged system values and all user original/target values',async()=>{
 const start=source.indexOf('{#if preview}<div class="region-preview">');const end=source.indexOf('{/if}</section>',start);assert.ok(start>=0&&end>start);
 const fragment=source.slice(start,end+5);const code=`<script>let {preview}=$props();const itemNames={locale:'系统地区格式',client_language:'Claude 界面语言'};const stateLabel=s=>s;let busy='',fixture=false;const recover=()=>{},cancelPlan=()=>{},execute=()=>{};</script>${fragment}`;
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
