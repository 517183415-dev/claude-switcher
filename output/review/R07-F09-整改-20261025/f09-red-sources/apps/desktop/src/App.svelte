<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import type { RuntimeInfo } from './generated/RuntimeInfo';
  import type { WorkspaceSnapshot } from './generated/WorkspaceSnapshot';
  import type { CheckReport } from './generated/CheckReport';
  import type { CheckId } from './generated/CheckId';
  import type { OperationOutcome } from './generated/OperationOutcome';
  import type { ActivityEntry } from './generated/ActivityEntry';
  import type { AdSlot } from './generated/AdSlot';
  import type { Campaign } from './generated/Campaign';
  import type { Preferences } from './generated/Preferences';
  import type { UpdateAction } from './generated/UpdateAction';
  import Icon from './components/Icon.svelte';
  import AdCard from './components/AdCard.svelte';
  import RegionSettings from './components/RegionSettings.svelte';
  import { slots, slotNames, checkNames, statusNames, dateTime, dialogFocus, errorMessage, recoveryMessage } from './ui';
  import { previewSnapshot } from './fixture';
  import './styles.css';

  type Page = 'overview' | 'accounts' | 'environment' | 'activity' | 'settings' | 'operations';
  const personal = import.meta.env.VITE_EDITION === 'personal';
  const fixture = import.meta.env.DEV && new URLSearchParams(location.search).get('fixture') === '1';
  let page = $state<Page>('overview');
  let info = $state<RuntimeInfo | null>(null);
  let snapshot = $state<WorkspaceSnapshot | null>(null);
  let loading = $state(true);
  let busy = $state('');
  let canCancel = $state(false);
  let checkEpoch=0;
  let suppressReport=false;
  let error = $state('');
  let outcome = $state<OperationOutcome | null>(null);
  let activity = $state<ActivityEntry[]>([]);
  let activityLoaded = $state(false);
  let expanded = $state<CheckId | null>(null);
  let adConfirm = $state<AdSlot | null>(null);
  let adLinkMessage = $state('');
  let adRefreshMessage = $state('');
  let consentCheckbox = $state(false);
  let adConsentIndex = $state<number | null>(null);
  let now = $state(Math.floor(Date.now()/1000));
  let displayImages = $state<Record<string,string>>({});
  let attemptedImages = new Set<string>();
  let imageGeneration = $state(0);
  let operationsComponent = $state<typeof import('./pages/Operations.svelte').default | null>(null);
  const runtime = $derived(snapshot?.runtime ?? info);
  const report = $derived(snapshot?.check_report ?? null);
  const summary = $derived(report?.summary);
  const currentIssue = $derived(report?.checks.find(c => c.status !== 'pass') ?? null);
  const pageNames: Record<Page, string> = {overview:'准备好，开始工作。',accounts:'账号档案',environment:'环境检查',activity:'活动记录',settings:'工作空间设置',operations:personal?'广告运营':''};
  const navItems = [{id:'overview',label:'总览',icon:'home'},{id:'accounts',label:'账号',icon:'user'},{id:'environment',label:'环境检查',icon:'environment'},{id:'activity',label:'活动记录',icon:'activity'}] as const;

  async function refresh() {
    if(fixture)return;
    const epoch=checkEpoch,value=await invoke<WorkspaceSnapshot>('workspace_snapshot');
    if(epoch!==checkEpoch)return;
    snapshot={...value,check_report:suppressReport?null:value.check_report};attemptedImages.clear();imageGeneration++;
  }
  function clearReport(){suppressReport=true;if(snapshot)snapshot={...snapshot,check_report:null};}
  function announce(result: OperationOutcome) { outcome=result; error=''; }
  async function perform(command: string,args: Record<string,unknown>={},label='正在执行…') {
    if (busy) return;
    if (fixture) { announce({completed:false,message:'当前是合成预览，未执行系统、账号或网络操作。',recovery:null}); return; }
    if(command==='launch_checked'||(command==='account_action'&&(args.action==='select_a'||args.action==='select_b'))){checkEpoch++;clearReport();}
    busy=label; error=''; outcome=null;
    try { announce(await invoke<OperationOutcome>(command,args)); if(command==='launch_checked'||command==='account_action')suppressReport=false;await refresh(); }
    catch(e) { error=errorMessage(e); } finally { busy=''; }
  }
  async function refreshAds() {
    if(busy||!snapshot)return;
    if(fixture){adRefreshMessage='合成预览不刷新广告内容，未发起网络请求。';return;}
    busy='正在刷新并验证签名广告内容…';adRefreshMessage='';
    try {
      const result=await invoke<OperationOutcome>('refresh_ad_content');
      const message=result.message.includes('ad_refresh_not_due_or_merged')?'距上次手动刷新不足 30 秒，或已有刷新正在进行。请稍后重试。':result.message.includes('clock_recovery_required')?'本地时间需要恢复。请校正系统时间、重新建立时间基线后重试。':result.message;
      adRefreshMessage=`${result.completed?'刷新完成':'等待刷新'}：${message}`;
      await refresh();
    }catch(e){adRefreshMessage=`广告刷新未完成：${errorMessage(e)}`;}finally{busy='';}
  }
  async function runChecks() {
    if(busy)return;
    if(fixture){announce({completed:false,message:'合成预览不执行本机检查。',recovery:null});return;}
    const epoch=++checkEpoch;clearReport();
    busy='正在检查 14 项环境…';canCancel=true;error='';outcome=null;
    try{
      const result=await invoke<CheckReport>('check_environment');
      if(epoch!==checkEpoch)return;
      suppressReport=false;if(snapshot)snapshot={...snapshot,check_report:result};
      announce({completed:true,message:`检查完成：${result.summary.passed} 项通过 · ${result.summary.advisory} 项提示 · ${result.summary.blocking} 项阻止启动。`,recovery:null});
    }catch(e){
      if(epoch!==checkEpoch)return;
      error=errorMessage(e);clearReport();
      try{await refresh();}catch{/* Preserve the check failure and cleared evidence. */}
    }finally{if(epoch===checkEpoch){busy='';canCancel=false;}}
  }
  async function cancelCheck(){
    if(!canCancel||fixture)return;
    const epoch=++checkEpoch;clearReport();canCancel=false;busy='正在取消本次检查…';
    try{
      const result=await invoke<OperationOutcome>('cancel_check');
      if(epoch!==checkEpoch)return;
      announce(result);
    }catch(e){if(epoch===checkEpoch)error=errorMessage(e);}
    finally{if(epoch===checkEpoch){try{await refresh();}catch{/* Keep evidence cleared when reconnection fails. */}if(epoch===checkEpoch)busy='';}}
  }
  async function navigate(target: Page) {
    page=target;error='';outcome=null;
    if(target==='activity') {
      try { activity=fixture?[{observed_at:Math.floor(Date.now()/1000),level:'info',message:'合成界面预览，未执行本机检查或账号操作。'}]:await invoke<ActivityEntry[]>('activity_log');activityLoaded=true; }
      catch(e) { error=errorMessage(e); }
    }
    if(target==='operations'&&personal&&!operationsComponent) {
      try { operationsComponent=(await import('./pages/Operations.svelte')).default; }
      catch { error='运营页面加载失败，请重新打开页面。'; }
    }
    await tick();document.getElementById('main-content')?.scrollTo({top:0});document.getElementById('page-title')?.focus({preventScroll:true});
  }
  async function savePreferences(preferences: Preferences) {
    if(!snapshot||busy)return;
    if(fixture){snapshot={...snapshot,preferences};return;}
    busy='正在保存本机设置…';error='';
    try { snapshot={...snapshot,preferences:await invoke<Preferences>('save_preferences',{preferences})}; attemptedImages.clear(); imageGeneration++; }
    catch(e){error=errorMessage(e);}finally{busy='';}
  }
  function toggleAd(index: number,checked: boolean) {
    if(!snapshot)return;
    const prefs=structuredClone($state.snapshot(snapshot.preferences));prefs.ads.slots_enabled[index]=checked;
    if(checked&&!prefs.ads.request_purpose_acknowledged){consentCheckbox=false;adConsentIndex=index;return;}
    void savePreferences(prefs);
  }
  async function enableAds() {
    if(adConsentIndex===null||!snapshot||!consentCheckbox)return;
    const prefs=structuredClone($state.snapshot(snapshot.preferences));prefs.ads.slots_enabled[adConsentIndex]=true;prefs.ads.request_purpose_acknowledged=true;
    adConsentIndex=null;await savePreferences(prefs);
  }
  function campaignFor(slot: AdSlot): Campaign|null {
    if (!snapshot?.ads_manifest || snapshot.ads_manifest.expires_at<=now) return null;
    return snapshot?.ads_manifest?.slots.find(s=>s.slot===slot&&s.enabled)?.campaigns.find(c=>c.enabled&&c.starts_at<=now&&c.ends_at>now)??null;
  }
  const visibleImages = $derived(slots.map((slot,index) => snapshot?.preferences.ads.slots_enabled[index] ? `${slot}:${campaignFor(slot)?.image.sha256??''}` : '').join('|'));
  $effect(() => {
    const signature=visibleImages;
    void imageGeneration;
    if(fixture||!signature)return;
    for(const part of signature.split('|')) {
      const [slot,sha256]=part.split(':');
      if(!sha256||attemptedImages.has(part)||displayImages[sha256])continue;
      attemptedImages.add(part);
      void invoke<string>('ad_cached_image',{slot,sha256}).then(url=>{displayImages={...displayImages,[sha256]:url};}).catch(()=>{});
    }
  });
  async function openAd(slot: AdSlot,confirmed=false) {
    if(fixture){announce({completed:false,message:'合成预览不会打开商家链接。',recovery:null});return;}
    if(confirmed){adConfirm=null;await perform('open_ad_link',{slot,confirmed:true},'正在用系统浏览器打开…');return;}
    if(busy)return;busy='正在核验当前商家链接…';error='';
    try {const result=await invoke<OperationOutcome>('open_ad_link',{slot,confirmed:false});if(result.recovery==='confirm_ad_link'){adLinkMessage=result.message;adConfirm=slot;}else{announce(result);}}
    catch(e){error=errorMessage(e);}finally{busy='';}
  }
  async function exportActivity() {
    if(busy)return;
    if(fixture){
      const blob=new Blob([JSON.stringify(activity,null,2)],{type:'application/json'}),url=URL.createObjectURL(blob),link=document.createElement('a');
      link.href=url;link.download='claude-switcher-脱敏活动记录.json';link.click();URL.revokeObjectURL(url);
      return;
    }
    busy='正在导出本机脱敏记录…';error='';outcome=null;
    try{
      const [path,entries]=await invoke<[string,ActivityEntry[]]>('export_activity');
      activity=entries;
      announce({completed:true,message:`脱敏记录已保存到本机：${path}`,recovery:null});
    }catch(e){error=errorMessage(e);}finally{busy='';}
  }
  function profileLabel(state: string){return({uninitialized:'未建立',saved:'已保存',current:'当前使用',recovery_pending:'恢复待处理'} as Record<string,string>)[state]??'待确认';}
  function groupStatus(ids: CheckId[]) {
    if(!report)return{label:'尚未检查',detail:'取得实际证据后显示结果',good:false};
    const issue=report.checks.filter(c=>ids.includes(c.id)).find(c=>c.status!=='pass');
    return issue?{label:statusNames[issue.status],detail:issue.reason,good:false}:{label:'通过',detail:'本次证据已取得，启动前将重新核验',good:true};
  }
  const groups=[
    {title:'网络连接',description:'代理 · TUN · DNS · STUN',icon:'globe',ids:['proxy_process','tun_route','dns_route','stun_path','service_transport'] as CheckId[]},
    {title:'地区设置',description:'出口地区 · 时区 · 地区格式',icon:'pin',ids:['egress_region','timezone','locale','client_language'] as CheckId[]},
    {title:'后台服务',description:'守护进程 · 服务路由',icon:'settings',ids:['guardian','service_routes'] as CheckId[]},
  ];
  async function bootstrap() {
    if(fixture){snapshot=previewSnapshot();info=snapshot.runtime;return;}
    info=await invoke<RuntimeInfo>('runtime_info');
    if(import.meta.env.VITE_IPC_SMOKE === '1' && new URLSearchParams(location.search).has('ipc-smoke')){
      await tick();let denied=false;try{await invoke('forbidden_probe');}catch{denied=true;}
      await new Promise(requestAnimationFrame);const rendered=document.querySelectorAll('[data-slot]').length===3;
      await invoke('finish_smoke',{ok:denied&&rendered,denied,rendered});
    }else{
      await refresh();loading=false;
      await runChecks();
    }
  }
  onMount(()=>{
    const timer=setInterval(()=>{now=Math.floor(Date.now()/1000);},1000);
    const snapshotTimer=setInterval(()=>{if(!fixture&&!busy&&!loading)void refresh().catch(()=>{});},30_000);
    void bootstrap().catch(e=>error=errorMessage(e)).finally(()=>loading=false);
    return()=>{clearInterval(timer);clearInterval(snapshotTimer);};
  });
</script>

<svelte:head><link rel="icon" href="data:image/svg+xml,%3Csvg xmlns=%27http://www.w3.org/2000/svg%27 viewBox=%270 0 20 20%27%3E%3Crect x=%273%27 y=%273%27 width=%2714%27 height=%2714%27 rx=%273%27 fill=%27%23204f3c%27/%3E%3C/svg%3E"/></svelte:head>
<div class="app-shell">
  <aside class="sidebar" aria-label="主导航">
    <div class="brand"><Icon name="grid" size={22}/><span>工作空间</span></div>
    <nav aria-label="页面">
      {#each navItems as item}<button class:active={page===item.id} aria-current={page===item.id?'page':undefined} onclick={()=>navigate(item.id)}><Icon name={item.icon}/><span>{item.label}</span></button>{/each}
      {#if personal}<button class:active={page==='operations'} aria-current={page==='operations'?'page':undefined} onclick={()=>navigate('operations')}><Icon name="image"/><span>广告运营</span></button>{/if}
    </nav>
    <div class="sidebar-bottom"><button class:active={page==='settings'} aria-current={page==='settings'?'page':undefined} onclick={()=>navigate('settings')}><Icon name="settings"/><span>设置</span></button><div class="version">{(runtime?.edition??(personal?'personal':'public'))==='personal'?'Personal':'Public'} · {runtime?.version??'连接中'}</div></div>
  </aside>
  <div class="workspace">
    <main id="main-content" aria-busy={loading||!!busy}>
      {#if fixture}<div class="fixture-banner" role="status">合成界面预览 · 未执行本机检查、账号操作或网络请求</div>{/if}
      <header class="page-header"><div><h1 id="page-title" tabindex="-1">{pageNames[page]}</h1><p>{page==='overview'?'自动检查当前环境，确认准备就绪后启动 Claude。':page==='environment'?'查看实际证据，修复会先显示计划。':page==='accounts'?'选择本机账号，检查后自动切换并启动。':page==='activity'?'这里只显示脱敏后的本机操作记录。':page==='settings'?'管理更新、广告和本地时间恢复。':personal?'三个固定广告位，独立编辑、排期和发布。':''}</p></div><span class="edition-tag">{fixture?'合成预览':(runtime?.edition??(personal?'personal':'public'))==='personal'?'Personal · 本机自用':'Public · 公开发行'}</span></header>
      {#if loading}<div class="notice" role="status"><span class="spinner"></span>正在连接 Rust 核心…</div>{/if}
      {#if busy}<div class="notice progress" role="status"><span class="spinner"></span><span>{busy}</span>{#if canCancel}<button class="text-button" onclick={cancelCheck}>取消本次检查</button>{/if}</div>{/if}
      {#if error}<div class="notice danger" role="alert"><Icon name="alert"/><div><strong>操作未完成</strong><p>{error}</p><button class="text-button" onclick={()=>{error='';loading=true;void refresh().catch(e=>error=errorMessage(e)).finally(()=>loading=false);}}>重新连接</button></div></div>{/if}
      {#if outcome}<div class:success={outcome.completed} class="notice" role="status"><Icon name={outcome.completed?'check':'alert'}/><div><strong>{outcome.completed?'操作完成':'等待处理'}</strong><p>{outcome.message}</p>{#if outcome.recovery}<p>{recoveryMessage(outcome.recovery)}</p>{/if}</div><button class="dismiss" aria-label="关闭操作提示" onclick={()=>outcome=null}>×</button></div>{/if}
      {#if page==='overview'}
        <section class="hero-workbench" aria-label="账号与环境概况">
          <div class="account-hero"><h2>当前账号</h2><div class="account-row"><div class="avatar">{snapshot?.accounts.current??'—'}</div><div class="account-name"><strong>{snapshot?.accounts.current?`账号 ${snapshot.accounts.current}`:'尚未关联'}</strong><span><i class:neutral={!snapshot?.accounts.current}></i>{snapshot?.accounts.current?'已选择 · 本地档案':'首次切换时自动保存账号 A'}</span></div><button class="outline switch-button" disabled={!!busy} onclick={()=>navigate('accounts')}><Icon name="switch" size={18}/>选择账号</button></div>
            <div class="connection-path" aria-label="本机经过代理隧道连接 Claude"><div><span><Icon name="laptop" size={27}/></span><strong>本机</strong><small>设备与系统环境</small></div><Icon name="arrow" size={35}/><div><span><Icon name="server" size={27}/></span><strong>代理隧道</strong><small>加密连接 · 网络出口</small></div><Icon name="arrow" size={35}/><div><span><Icon name="star" size={28}/></span><strong>Claude</strong><small>服务与访问验证</small></div></div>
            <button class="primary launch-button" disabled={loading||!!busy||!snapshot} onclick={()=>perform('launch_checked',{},'正在重新检查并准备启动…')}>检查并启动 Claude<Icon name="arrow"/></button><p class="button-caption">启动前自动检查 14 项环境</p>
          </div>
          <div class="environment-hero"><h2>环境概况</h2><div class="score"><strong>{summary?.passed??'—'}</strong><span>/ 14</span></div><p class="summary">{summary?`${summary.passed} 项通过 · ${summary.advisory} 项提示${summary.blocking?` · ${summary.blocking} 项待处理`:''}`:'尚未检查 · 等待实际证据'}</p><div class="hero-divider"></div><div class="issue-card" class:clear={report&&!currentIssue}><Icon name={report&&!currentIssue?'check':'alert'} size={22}/><div><strong>{currentIssue?checkNames[currentIssue.id]:report?'环境检查已完成':'先检查，再启动'}</strong><p>{currentIssue?.reason??(report?'启动前会重新执行环境检查。':'未取得证据时，启动保持等待。')}</p></div></div><button class="detail-link" onclick={()=>navigate('environment')}>查看检查详情<Icon name="arrow" size={18}/></button></div>
        </section>
        {#if snapshot?.accounts.action_blocker}<div class="inline-wait"><Icon name="lock" size={16}/><span>{snapshot.accounts.action_blocker}</span><button class="text-button" onclick={()=>navigate('accounts')}>查看账号</button></div>{/if}
        <section class="status-grid" aria-label="环境分类摘要">{#each groups as group}{@const state=groupStatus(group.ids)}<article class="status-card"><div class="card-heading"><Icon name={group.icon} size={28}/><div><h3>{group.title}</h3><p>{group.description}</p></div></div><div class="card-status"><span class:good={state.good}><Icon name={state.good?'check':'alert'} size={18}/></span><div><strong>{state.label}</strong><p>{state.detail}</p></div></div><button class="detail-link" onclick={()=>navigate('environment')}>查看详情<Icon name="arrow" size={16}/></button></article>{/each}</section>
        <div class="section-heading"><div><h2>精选服务</h2><p>三个广告位独立开关，商业内容不参与启动判断。</p></div><div class="button-row"><button class="text-button" disabled={loading||!!busy||!snapshot} onclick={refreshAds}>刷新广告内容<Icon name="refresh" size={15}/></button><button class="text-button" onclick={()=>navigate('settings')}>广告设置</button></div></div>
        {#if adRefreshMessage}<p class="muted footnote" role="status">{adRefreshMessage}</p>{/if}
        <section class="ad-grid" aria-label="三个固定广告位">{#each slots as slot,index}<AdCard {slot} campaign={campaignFor(slot)} enabled={snapshot?.preferences.ads.slots_enabled[index]??false} imageUrl={displayImages[campaignFor(slot)?.image.sha256??'']??''} onclick={openAd}/>{/each}</section>
      {:else if page==='accounts'}
        <div class="notice"><Icon name="lock"/><p>档案与登录资料只在本机处理。登录状态仅按实际证据显示。</p></div>
        {#if snapshot?.accounts.action_blocker}<div class="notice warning"><Icon name="alert"/><p>{snapshot.accounts.action_blocker}</p></div>{/if}
        <p class="muted">选择账号后自动检查、退出 Claude、切换并启动。首次切换会保存账号 A；首次使用 B 时打开登录界面。</p>
        <section class="account-grid" aria-label="A 和 B 本机档案">{#each snapshot?.accounts.profiles??[] as profile}<article class="panel profile-card"><div class="avatar">{profile.id}</div><div class="profile-heading"><h2>账号 {profile.id}</h2><span class="badge">{profileLabel(profile.state)}</span></div><p>{profile.login_evidence}</p><button class="outline" disabled={loading||!!busy||profile.state==='recovery_pending'} onclick={()=>perform('account_action',{action:profile.id==='A'?'select_a':'select_b'},'正在检查环境并切换启动…')}>{profile.state==='current'?'启动当前账号':profile.state==='uninitialized'?'首次使用此账号':'切换并启动'}</button></article>{/each}</section>
        {#if !snapshot?.accounts.profiles.length}<div class="empty-state"><Icon name="user" size={35}/><h2>尚未读取档案状态</h2><p>连接核心后可查看已有 A/B 档案。</p></div>{/if}
        <section class="panel"><h2>恢复中断操作</h2><p class="muted">切换中断时保留账号资料，按记录恢复。</p><button class="text-button" disabled={!!busy||!snapshot} onclick={()=>perform('account_action',{action:'recover'},'正在恢复中断操作…')}>恢复中断事务</button></section>
      {:else if page==='environment'}
        <div class="page-toolbar"><span>{summary?`${summary.passed} 项通过 · ${summary.advisory} 项提示 · ${summary.blocking} 项待处理`:'14 项稳定检查 · 尚未取得报告'}</span><div class="button-row"><button class="primary" disabled={!!busy||!snapshot} onclick={runChecks}><Icon name="refresh" size={17}/>重新检查</button></div></div>
        {#if report}<div class="check-list">{#each report.checks as check}<article class="check-item"><button class="check-row" aria-expanded={expanded===check.id} onclick={()=>expanded=expanded===check.id?null:check.id}><span class="check-indicator" class:good={check.status==='pass'}>{check.status==='pass'?'✓':check.status==='warning'?'!':'·'}</span><span class="check-copy"><strong>{checkNames[check.id]}</strong><small>{check.reason}</small></span><span class={`check-tag ${check.status}`}>{statusNames[check.status]}</span><span class="chevron">{expanded===check.id?'−':'+'}</span></button>{#if expanded===check.id}<div class="check-evidence"><dl><div><dt>观察时间</dt><dd>{dateTime(check.observed_at)}</dd></div><div><dt>耗时</dt><dd>{check.duration_ms} ms</dd></div><div><dt>有效期至</dt><dd>{dateTime(check.valid_until)}</dd></div></dl><p class="eyebrow">脱敏证据</p>{#each check.evidence as evidence}<p class="evidence-line">{evidence}</p>{:else}<p class="muted">未取得更多证据。</p>{/each}</div>{/if}</article>{/each}</div><p class="muted footnote">必需项失败或缺失时，启动进入等待。401 表示需要认证；403 表示访问受限。</p>{:else}<div class="empty-state"><Icon name="environment" size={38}/><h2>准备检查环境</h2><p>检查返回完整 14 项报告。网络与 STUN 超时会单独标记。</p><button class="primary" disabled={!!busy||!snapshot} onclick={runChecks}>开始只读检查</button></div>{/if}
        <RegionSettings {fixture} environment onoutcome={announce}/>
      {:else if page==='activity'}
        <div class="page-toolbar"><span>本机记录 · 与导出使用同一脱敏结果</span><button class="outline" disabled={!!busy||!activity.length} onclick={exportActivity}>导出脱敏记录</button></div>
        {#if activity.length}<ol class="activity-list">{#each activity as entry}<li><span class="activity-dot"></span><time>{dateTime(entry.observed_at)}</time><p>{entry.message}</p><span class="badge">{entry.level==='error'?'错误':entry.level==='warning'?'提示':'记录'}</span></li>{/each}</ol>{:else}<div class="empty-state"><Icon name="activity" size={38}/><h2>{activityLoaded?'暂无活动记录':'正在读取记录'}</h2><p>检查、账号事务和恢复结果会记录在这里。</p></div>{/if}
      {:else if page==='settings'}
        <RegionSettings {fixture} onoutcome={announce}/>
        <section class="panel"><div class="section-heading"><div><h2>软件更新</h2><p>{(runtime?.edition??(personal?'personal':'public'))==='personal'?'Personal：已配置签名更新通道时，检查、下载、安装分别由本人操作。':'Public：已配置签名更新通道时，默认每 12 小时检查。'}</p></div><span class="badge">{runtime?.version??'—'}</span></div><p>{snapshot?.update.message??'尚未取得更新状态。'}</p>{#if snapshot?.update.next_check_at}<p class="muted">下次检查：{dateTime(snapshot.update.next_check_at)}</p>{/if}{#if snapshot?.update.candidate_version}<p>候选版本：{snapshot.update.candidate_version}</p>{/if}<div class="button-row"><button class="outline" disabled={!!busy||!snapshot} onclick={()=>perform('software_update',{action:'check' satisfies UpdateAction},'正在检查签名更新元数据…')}>检查更新</button><button class="outline" disabled={!!busy||!snapshot?.update.can_download} onclick={()=>perform('software_update',{action:'download'},'正在下载并验证候选工件…')}>下载候选</button><button class="outline" disabled={!!busy||!snapshot?.update.can_install} onclick={()=>perform('software_update',{action:'install'},'正在核验安装条件…')}>确认安装</button>{#if runtime?.automatic_updates&&!snapshot?.update.message.includes('源码构建版本')}<button class="text-button" disabled={!!busy||snapshot?.update.paused==null} onclick={()=>perform('software_update',{action:snapshot?.update.paused?'resume':'pause'},'正在保存自动更新计划…')}>{snapshot?.update.paused?'恢复自动检查':'暂停自动检查'}</button><button class="text-button" disabled={!!busy} onclick={()=>perform('software_update',{action:'defer'},'正在延后安装…')}>延后安装</button>{/if}</div><p class="muted footnote">安装要求：Claude 完全退出、账号事务空闲、组件兼容，且工件通过全部身份验证。</p></section>
        <section class="panel"><div class="section-heading"><h2>广告展示</h2><button class="outline" disabled={loading||!!busy||!snapshot} onclick={refreshAds}>刷新广告内容<Icon name="refresh" size={15}/></button></div><p class="muted">开启后仅向固定内容源读取签名清单与对应素材。所有广告位关闭时，广告请求为零。</p><p class="muted footnote">手动刷新仅访问固定内容源。核心限制至少间隔 30 秒，并合并进行中的刷新。</p>{#if adRefreshMessage}<p class="muted footnote" role="status">{adRefreshMessage}</p>{/if}{#each slots as slot,index}<label class="preference-row"><span><strong>{slotNames[slot]}</strong><small>只控制此广告位，互不覆盖</small></span><input type="checkbox" role="switch" checked={snapshot?.preferences.ads.slots_enabled[index]??false} disabled={!!busy||!snapshot} onchange={e=>{const checked=e.currentTarget.checked;e.currentTarget.checked=snapshot?.preferences.ads.slots_enabled[index]??false;toggleAd(index,checked);}} aria-label={`显示${slotNames[slot]}广告`}/></label>{/each}</section>
        <section class="panel"><h2>本地时间恢复</h2><p class="muted">发现时间回拨或跳变时，广告收起、软件安装暂停。校正系统时间后，重新建立时间基线。</p><div class="button-row"><button class="outline" disabled={!!busy} onclick={()=>perform('open_date_settings',{},'正在打开系统日期设置…')}>打开系统日期设置</button><button class="outline" disabled={!!busy||!snapshot} onclick={()=>perform('rebuild_time_baseline',{},'正在重建时间基线…')}>重新建立时间基线</button></div><p class="muted footnote">重新建立后，仍须从固定源取得有效签名内容。最高内容序号保持不变。</p></section>
        {#if snapshot?.pending_materials.length}<section class="panel"><h2>等待材料</h2><ul class="plain-list">{#each snapshot.pending_materials as material}<li>{material}</li>{/each}</ul></section>{/if}
      {:else if page==='operations'&&personal}
        {#if !operationsComponent}<div class="empty-state" role="status">正在载入运营页面…</div>{/if}
      {/if}
      {#if personal&&operationsComponent}{@const Operations=operationsComponent}<div hidden={page!=='operations'}><Operations {fixture} onoutcome={announce}/></div>{/if}
    </main>
    <footer><span>最近检查</span><strong>{report?dateTime(report.completed_at):'尚未检查'}</strong><span class="footer-separator"></span><button class="text-button" disabled={!!busy||!snapshot} onclick={runChecks}>重新检查<Icon name="refresh" size={14}/></button><span class="footer-local"><Icon name="lock" size={12}/>资料留在本机</span></footer>
  </div>
</div>
{#if adConsentIndex!==null}<div class="modal-backdrop"><div class="modal" role="dialog" aria-modal="true" aria-labelledby="ad-consent-title" tabindex="-1" use:dialogFocus={()=>adConsentIndex=null}><h2 id="ad-consent-title">开启广告展示</h2><p>应用将向固定产品内容源读取签名清单与素材。内容源会看到连接来源，但请求不会附加账号、设备或代理资料。</p><p>商业链接由系统浏览器打开。你可以随时关闭单个广告位或全部广告位。</p><label class="checkbox-label"><input type="checkbox" bind:checked={consentCheckbox}/>我已了解请求用途</label><div class="button-row"><button class="outline" onclick={()=>adConsentIndex=null}>暂不开启</button><button class="primary" disabled={!consentCheckbox} onclick={enableAds}>开启此广告位</button></div></div></div>{/if}
{#if adConfirm}<div class="modal-backdrop"><div class="modal" role="dialog" aria-modal="true" aria-labelledby="ad-link-title" tabindex="-1" use:dialogFocus={()=>adConfirm=null}><h2 id="ad-link-title">打开商家网站</h2><p class="domain-label">{adLinkMessage}</p><p class="muted">商家与规范主机由核心核验。链接将在系统浏览器打开。应用不会附加账号、设备或代理资料。</p><div class="button-row"><button class="outline" onclick={()=>adConfirm=null}>取消</button><button class="primary" disabled={!!busy} onclick={()=>adConfirm&&openAd(adConfirm,true)}>确认打开<Icon name="external" size={16}/></button></div></div></div>{/if}
