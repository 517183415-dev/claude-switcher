<script lang="ts">
import {onMount,onDestroy} from 'svelte';
import {invoke} from '@tauri-apps/api/core';
import type {RegionView} from '../generated/RegionView';
import type {RegionPreset} from '../generated/RegionPreset';
import type {RepairPreview} from '../generated/RepairPreview';
import type {OperationOutcome} from '../generated/OperationOutcome';
import {errorMessage} from '../ui';
import {previewRegionRepair} from '../fixture';
let {fixture=false,environment=false,onoutcome}:{fixture?:boolean;environment?:boolean;onoutcome:(outcome:OperationOutcome)=>void}=$props();
let view=$state<RegionView|null>(null),selected=$state<RegionPreset>('us'),preview=$state<RepairPreview|null>(null),busy=$state(''),error=$state(''),recoveryMessage=$state('');
const options:{id:RegionPreset;label:string}[]=[{id:'us',label:'美国'},{id:'jp',label:'日本'},{id:'tw',label:'台湾'},{id:'hk',label:'香港'},{id:'sg',label:'新加坡'},{id:'gb',label:'英国'},{id:'de',label:'德国'},{id:'fr',label:'法国'},{id:'kr',label:'韩国'},{id:'au',label:'澳大利亚'},{id:'ca',label:'加拿大'}];
async function load(){if(fixture)return;view=await invoke<RegionView>('region_view');}
let requestId:string|null=null;
let epoch=0;
let disposed=false;
async function cancelPlan(){
  const id=requestId;requestId=null;epoch++;preview=null;busy='';
  if(id&&!fixture){try{await invoke('cancel_repair',{requestId:id});}catch{/* Local confirmation remains invalid even if disconnected. */}}
}
async function plan(){
  if(busy||disposed)return;
  void cancelPlan();
  const current=++epoch,id=crypto.randomUUID();requestId=id;
  preview=null;error='';recoveryMessage='';
  if(fixture){preview=previewRegionRepair(selected);return;}
  busy='正在读取系统原值并预览地区修改…';
  try{
    const result=await invoke<RepairPreview>(environment?'preview_repair':'preview_region_change',environment?{requestId:id}:{region:selected,requestId:id});
    if(current!==epoch){await invoke('cancel_repair',{requestId:id});return;}
    preview=result;
  }catch(e){if(current===epoch){preview=null;requestId=null;error=errorMessage(e);}try{await invoke('cancel_repair',{requestId:id});}catch{/* Retry requires a new preview. */}}
  finally{if(current===epoch)busy='';}
}
async function execute(){
  if(busy||fixture||!requestId||!preview?.can_execute||preview.recovery_required)return;
  const id=requestId;requestId=null;preview=null;epoch++;
  busy='正在核验并执行已确认的地区计划…';error='';
  try{onoutcome(await invoke<OperationOutcome>('execute_repair',{requestId:id}));await load();}
  catch(e){error=errorMessage(e);}finally{busy='';}
}
async function recover(){
  if(busy||!preview?.recovery_required)return;
  error='';recoveryMessage='';
  if(fixture){recoveryMessage='合成预览未查询系统结果，也未处理修复事务。真实处理后请核对地区目标、系统时区与 IPv6，重新检查环境。';return;}
  const current=epoch;
  busy='正在查询系统结果并恢复修复事务…';
  try{
    const result=await invoke<OperationOutcome>('recover_repair');
    if(current!==epoch||disposed)return;
    onoutcome(result);
    if(result.completed){
      recoveryMessage='修复事务已处理，请核对地区目标、系统时区与 IPv6，重新检查环境。';
      preview=null;
      await load();
      if(current!==epoch||disposed)return;
      busy='';await plan();
    }
  }catch(e){error=errorMessage(e);}finally{busy='';}
}
const itemNames:Record<string,string>={locale:'系统地区格式',client_language:'Claude 界面语言',proxy_tun_config:'代理 TUN 配置',proxy_pinned_routes:'代理域名置顶规则'};
const stateLabel=(state:string)=>state==='ready'?'待修改':state==='unchanged'?'无需修改':'需手动处理';
onMount(()=>{void load().catch(e=>error=errorMessage(e));});
onDestroy(()=>{disposed=true;void cancelPlan();});
</script>
<section class="panel"><h2>{environment?'修复计划':'目标地区'}</h2>
{#if !environment}<p class="muted">选择明确的目标国家。程序使用固定国家、时区与地区格式组合。</p>
{#if view}<dl class="region-values"><div><dt>当前锁定国家</dt><dd>{view.country}</dd></div><div><dt>目标时区</dt><dd>{view.timezone}</dd></div><div><dt>目标地区格式</dt><dd>{view.locale}</dd></div></dl>{:else}<p class="muted">{fixture?'合成预览未读取真实地区锁定。':'尚未读取地区锁定，连接核心后重试。'}</p>{/if}
<label class="region-select">新目标地区<select bind:value={selected} disabled={!!busy} onchange={()=>void cancelPlan()}>{#each options as option}<option value={option.id}>{option.label}</option>{/each}</select></label>
<p class="muted footnote">更换国家需要先切换到对应代理节点。仅保存界面选项不会改变目标。系统修复成功后，核心才会持锁保存新的地区配置。</p>
{/if}
{#if busy}<p role="status">{busy}</p>{/if}{#if error}<div class="notice danger" role="alert">{error}</div>{/if}
<div class="button-row"><button class="outline" disabled={!!busy} onclick={plan}>{environment?'预览修复计划':'预览地区修改'}</button><button class="text-button" disabled={!!busy||fixture} onclick={()=>load().catch(e=>error=errorMessage(e))}>重新读取地区</button></div>
{#if recoveryMessage}<div class="notice warning" role="status">{recoveryMessage}</div>{/if}
{#if preview}<div class="region-preview"><h3>地区修复预览</h3><p>{preview.message}</p><div class="region-comparison"><span>系统原值</span><span>确认目标</span><strong>{preview.original.country} · {preview.original.timezone} · {preview.original.locale}</strong><strong>{preview.target.country} · {preview.target.timezone} · {preview.target.locale}</strong><span>{preview.network_service} · IPv6 {preview.original_ipv6}</span><span>{preview.network_service} · IPv6 {preview.target_ipv6}</span></div><h3>用户配置逐项预览</h3>
{#if preview.user_changes.length}<ul class="repair-items">{#each preview.user_changes as change,index}<li class:manual={change.state!=='ready'&&change.state!=='unchanged'}><div class="repair-heading"><strong>{itemNames[change.id]??`用户配置项目 ${index+1}`}</strong><span>{stateLabel(change.state)}</span></div><p class="repair-values"><span>{change.original}</span><span aria-label="修改为">→</span><span>{change.target}</span></p><p>{change.message}</p>{#if change.state!=='ready'&&change.state!=='unchanged'}<p class="manual-note">此项需人工处理，本次不会自动修改。处理后请重新检查环境。</p>{/if}</li>{/each}</ul>{:else}<p class="muted">当前没有用户配置修改项目。</p>{/if}
{#if preview.recovery_required}<div class="notice warning recovery-notice"><p>存在未完成的修复事务。程序先查询系统持久结果，再完成提交或恢复原值。</p><p class="footnote">系统结果未知时会保留现场。处理后请核对新的地区目标、系统时区与 IPv6，并重新检查环境。</p><button class="outline" disabled={!!busy} onclick={recover}>恢复修复事务</button></div>{/if}
<p class="muted">确认系统原值与目标值后执行。系统权限由已验证的 helper 请求。</p><div class="button-row"><button class="outline" disabled={!!busy} onclick={cancelPlan}>取消此计划</button><button class="primary" disabled={!!busy||fixture||!preview.can_execute||preview.recovery_required} onclick={execute}>确认执行地区计划</button></div></div>{/if}</section>
<style>
.region-values{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:14px;margin:20px 0}.region-values dt{font-size:11px;color:var(--muted)}.region-values dd{font-size:12px;margin:7px 0;overflow-wrap:anywhere}.region-select{display:flex;align-items:center;gap:14px;margin-top:20px;font-size:12px}.region-select select{min-width:170px;padding:9px;border:1px solid var(--line);border-radius:4px;background:#fffefb;color:var(--ink)}.region-preview{border-top:1px solid var(--line);margin-top:20px;padding-top:18px}.region-preview h3{font-size:14px}.region-comparison{display:grid;grid-template-columns:1fr 1fr;gap:10px;margin:15px 0;font-size:11px}.region-comparison strong{font-weight:500;overflow-wrap:anywhere}.region-comparison span{color:var(--muted)}
.repair-items{list-style:none;padding:0;margin:12px 0}.repair-items li{padding:14px;border:1px solid var(--line);border-radius:5px;margin:10px 0;font-size:12px}.repair-items li.manual{border-color:#cfb982;background:#fcf7e9}.repair-heading{display:flex;justify-content:space-between;gap:12px}.repair-heading span{color:var(--muted)}.repair-items p{font-size:12px;margin:9px 0 0;line-height:1.6}.repair-values{display:flex;gap:12px;flex-wrap:wrap;overflow-wrap:anywhere}.manual-note{color:#75591e}.region-preview .notice{margin:16px 0}.region-preview .recovery-notice{display:block}.recovery-notice p{margin:0 0 12px}.recovery-notice button{white-space:nowrap}</style>
