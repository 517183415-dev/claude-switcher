<script lang="ts">
import {invoke} from '@tauri-apps/api/core';
import type {AdChannel} from '../generated/AdChannel';
import {errorMessage} from '../ui';

type R2Config={account_id:string;bucket:string;public_origin:string;channel:AdChannel};
type RemoteTarget={target:string;config:R2Config;selected:boolean};
type RemoteSettings={preview:R2Config|null;production:R2Config|null;targets:RemoteTarget[];unavailable_targets:string[]};
let {fixture=false,disabled=false}:{fixture?:boolean;disabled?:boolean}=$props();
let settings=$state<RemoteSettings|null>(null),channel=$state<AdChannel>('preview');
let useRemote=$state(false),account=$state(''),bucket=$state(''),origin=$state('');
let accessKey=$state(''),secretKey=$state(''),running=$state(false),message=$state(''),error=$state('');
function fill(){const config=settings?.[channel];useRemote=!!config;account=config?.account_id??'';bucket=config?.bucket??'';origin=config?.public_origin??'';accessKey='';secretKey='';}
async function read(){
 if(running||disabled||fixture)return;running=true;error='';message='';
 try{settings=await invoke<RemoteSettings>('publisher_remote_read');fill();}
 catch(e){error=errorMessage(e);}finally{running=false;}
}
async function save(){
 if(running||disabled||fixture||!settings)return;running=true;error='';message='';
 try{
  settings=await invoke<RemoteSettings>('publisher_remote_save',{channel,config:useRemote?{account_id:account.trim(),bucket:bucket.trim(),public_origin:origin.trim(),channel}:null,accessKey:useRemote?accessKey:'',secretKey:useRemote?secretKey:''});
  fill();message='配置已保存。此操作没有连接远程服务；请通过发布操作完成上传和公开回读。';
 }catch(e){error=errorMessage(e);}finally{accessKey='';secretKey='';running=false;}
}
async function selectTarget(target:string){
 if(running||disabled||fixture||!settings)return;running=true;error='';message='';
 try{settings=await invoke<RemoteSettings>('publisher_remote_select',{channel,target});fill();message='已选回保留的发布目标。此操作没有连接远程服务。';}
 catch(e){error=errorMessage(e);}finally{running=false;}
}
</script>

<section class="panel">
 <div class="section-heading"><div><h2>发布来源</h2><p>预览和正式渠道分别设置。R2 配置与写入凭据一起保存在本机钥匙串。</p></div><button class="outline" disabled={running||disabled||fixture} onclick={read}>读取已保存配置</button></div>
 {#if settings}
 <div class="remote-fields">
  <label>渠道<select value={channel} disabled={running||disabled} onchange={e=>{channel=e.currentTarget.value as AdChannel;fill();}}><option value="preview">预览</option><option value="production">正式</option></select></label>
  <label>发布来源<select bind:value={useRemote} disabled={running||disabled}><option value={false}>本机目录</option><option value={true}>Cloudflare R2</option></select></label>
  <p class="wide">该渠道已保存的来源：{settings[channel]?.public_origin??'本机目录'}</p>
  {#if useRemote}
  <label>Cloudflare 账号 ID<input bind:value={account} disabled={running||disabled} spellcheck="false" placeholder="32 位账号 ID"/></label>
  <label>存储桶名称<input bind:value={bucket} disabled={running||disabled} spellcheck="false"/></label>
  <label class="wide">公开域名<input type="url" bind:value={origin} disabled={running||disabled} placeholder="https://ads.example.com"/></label>
  <label>访问密钥 ID<input type="password" bind:value={accessKey} disabled={running||disabled} autocomplete="off"/></label>
  <label>访问密钥<input type="password" bind:value={secretKey} disabled={running||disabled} autocomplete="off"/></label>
  <p class="wide">两个渠道需要不同的存储桶和访问密钥。修改配置时请重新输入密钥。已有未完成发布时，先恢复原目标的发布，再更换来源。</p>
  {/if}
 </div>
 <button class="outline" disabled={running||disabled||fixture||(useRemote&&(!account||!bucket||!origin||!accessKey||!secretKey))} onclick={save}>保存该渠道配置</button>
 <p>更换来源或选择本机目录会保留旧目标及其凭据。保留目标和当前目标的凭据轮换均需重新输入密钥。</p>
 {#each settings.targets.filter(item=>item.config.channel===channel&&!item.selected) as item (item.target)}
 <p>{item.config.bucket} · {item.config.public_origin} <button class="outline" disabled={running||disabled||fixture} onclick={()=>selectTarget(item.target)}>选回此目标</button></p>
 {/each}
 {#if settings.unavailable_targets.length}<div class="notice danger" role="alert">有 {settings.unavailable_targets.length} 个旧发布目标缺少访问配置。请重新保存对应配置；完整核对前无法执行多目标清理。</div>{/if}
 {/if}
 {#if running}<p role="status">正在处理发布配置…</p>{/if}
 {#if message}<p role="status">{message}</p>{/if}
 {#if error}<div class="notice danger" role="alert">{error}</div>{/if}
</section>

<style>
.remote-fields{display:grid;grid-template-columns:1fr 1fr;gap:14px;margin:16px 0}.remote-fields label{display:flex;flex-direction:column;gap:7px;font-size:12px}.remote-fields input,.remote-fields select{border:1px solid var(--line);border-radius:4px;padding:9px;min-width:0;background:var(--paper,#fffefb);color:var(--ink)}.remote-fields p{font-size:12px;color:var(--muted);overflow-wrap:anywhere}.wide{grid-column:1/-1}
</style>
