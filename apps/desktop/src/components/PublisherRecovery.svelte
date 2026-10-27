<script lang="ts">
import {onDestroy} from 'svelte';
import {invoke} from '@tauri-apps/api/core';
import type {OperationOutcome} from '../generated/OperationOutcome';
import {errorMessage} from '../ui';
type Preview={digest:string;backup_filename:string;generation:string};
type Retained={generation:string;state_verified:boolean;history:unknown[];pending_release_id:string|null;pending_target_unknown:boolean};
let {fixture=false,disabled=false,onchanged,onoutcome,onholding}:{fixture?:boolean;disabled?:boolean;onchanged:()=>Promise<void>;onoutcome:(result:OperationOutcome)=>void;onholding:(holding:boolean)=>void}=$props();
let passphrase=$state(''),preview=$state<Preview|null>(null),running=$state(false),refreshPending=$state(false),error=$state(''),message=$state('');
let retained=$state<Retained[]>([]);
let disposed=false;
async function cancel(){
 try{await invoke('publisher_recovery_cancel');preview=null;onholding(false);message='已取消。已生成的加密备份继续保留。';}
 catch(e){error=errorMessage(e);}
}
async function prepare(){
 if(running||disabled||fixture||passphrase.length<12)return;
 running=true;error='';message='';onholding(true);
 try{
  const result=await invoke<Preview>('publisher_recovery_prepare',{passphrase});
  if(disposed){await invoke('publisher_recovery_cancel');return;}
  preview=result;
 }catch(e){error=errorMessage(e);onholding(false);}finally{passphrase='';running=false;}
}
async function confirm(){
 if(running||!preview)return;running=true;error='';const digest=preview.digest;preview=null;
 try{const result=await invoke<OperationOutcome>('publisher_recovery_confirm',{digest});message=result.message;onoutcome(result);}
 catch(e){error=errorMessage(e);}finally{try{await refreshAfterConfirm();}finally{running=false;}}
}
async function refreshAfterConfirm(){
 try{await onchanged();refreshPending=false;onholding(false);}
 catch(e){refreshPending=true;error=`${error?`${error}；`:''}读取最新资料失败，请重试：${errorMessage(e)}`;}
}
async function retryRefresh(){
 if(running||!refreshPending)return;running=true;error='';
 try{await refreshAfterConfirm();}finally{running=false;}
}
async function importArchive(e:Event){
 const input=e.currentTarget as HTMLInputElement,file=input.files?.[0];
 if(!file||running||disabled||fixture)return;
 if(file.size>64*1024*1024||passphrase.length<12){error='归档须不超过 64 MiB；请先输入至少 12 个字符的口令。';input.value='';return;}
 running=true;error='';message='';onholding(true);
 try{const result=await invoke<OperationOutcome>('publisher_archive_import',{bytes:Array.from(new Uint8Array(await file.arrayBuffer())),passphrase});message=result.message;onoutcome(result);}
 catch(e){error=errorMessage(e);}finally{passphrase='';running=false;onholding(false);input.value='';}
}
async function retainedAction(generation?:string){
 if(running||disabled||fixture)return;running=true;error='';
 try{
  if(generation){const result=await invoke<OperationOutcome>('publisher_retained_export',{generation,passphrase});message=result.message;onoutcome(result);}
  else retained=await invoke<Retained[]>('publisher_retained_list');
 }catch(e){error=errorMessage(e);}finally{if(generation)passphrase='';running=false;}
}
onDestroy(()=>{disposed=true;if(!fixture)void invoke('publisher_recovery_cancel').catch(()=>{});onholding(false);});
</script>

<section class="panel">
 <div class="section-heading"><div><h2>旧发布资料恢复</h2><p>来源不明的旧发布事务不会自动重放。软件先保存并核验完整加密归档，再等待你确认。</p></div></div>
 <p>确认后只建立新的本机代次。旧目录、历史、事务和密钥继续保留。请核对新草稿，再明确发布。</p>
 {#if refreshPending}
 <button class="outline" disabled={running} onclick={retryRefresh}>重新读取恢复结果</button>
 {:else if !preview}
 <label class="password">完整归档口令（至少 12 个字符）<input type="password" bind:value={passphrase} autocomplete="new-password" disabled={running||disabled||fixture}/></label>
 <div class="button-row"><button class="outline" disabled={running||disabled||fixture||passphrase.length<12} onclick={prepare}>保存完整备份并准备恢复</button><label class="outline file-button">导入完整归档到保留目录<input type="file" accept=".csbackup" disabled={running||disabled||fixture||passphrase.length<12} onchange={importArchive}/></label></div>
 {:else}
 <div class="notice"><div><strong>完整备份已写入并读回校验</strong><p>{preview.backup_filename}</p><p>确认期间已锁定发布资料与本机发布目录。取消会释放锁，并保留备份。</p><div class="button-row"><button class="outline" disabled={running} onclick={cancel}>取消恢复</button><button class="primary" disabled={running} onclick={confirm}>确认保留旧资料并建立新代次</button></div></div></div>
 {/if}
 {#if running}<p role="status">正在处理完整加密归档，请稍候…</p>{/if}
 {#if !preview&&!refreshPending}<button class="text-button" disabled={running||disabled||fixture} onclick={()=>retainedAction()}>读取保留资料</button>{/if}
 {#if !preview&&!refreshPending&&retained.length}<ul>{#each retained as item}<li><code>{item.generation}</code>：{item.state_verified?`索引已核对，${item.history.length} 条历史`:'索引尚未通过核对'}{item.pending_release_id?'；含旧待定事务':''}{item.pending_target_unknown?'；原目标不明':''}<button class="text-button" disabled={running||disabled||fixture||passphrase.length<12||!item.state_verified} onclick={()=>retainedAction(item.generation)}>加密导出</button></li>{/each}</ul>{/if}
 {#if message}<p role="status">{message}</p>{/if}
 {#if error}<div class="notice danger" role="alert">{error}</div>{/if}
</section>
<style>
.password{display:flex;flex-direction:column;gap:8px;max-width:420px;margin:16px 0;font-size:12px}.password input{border:1px solid var(--line);border-radius:4px;padding:9px;color:var(--ink);background:var(--paper,#fffefb)}p{font-size:12px;color:var(--muted);overflow-wrap:anywhere}.file-button{position:relative;overflow:hidden;cursor:pointer}.file-button input{position:absolute;width:1px;height:1px;opacity:0}.file-button:focus-within{outline:2px solid var(--green)}
</style>
