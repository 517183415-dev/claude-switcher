<script lang="ts">
  import type { AdSlot } from '../generated/AdSlot';
  import type { Campaign } from '../generated/Campaign';
  import Icon from './Icon.svelte';
  import { slotNames, dateTime, hostLabel } from '../ui';
  let { slot, campaign = null, enabled = false, preview = false, imageUrl = '', onclick }: {
    slot: AdSlot; campaign?: Campaign | null; enabled?: boolean; preview?: boolean;
    imageUrl?: string; onclick?: (slot: AdSlot) => void;
  } = $props();
</script>
<article class:inactive={!enabled || !campaign} class="ad-card" data-slot={slot} aria-label={slotNames[slot]}>
  <div class="ad-eyebrow"><span>{slotNames[slot]}</span><span>{preview ? '预览' : '推广'}</span></div>
  {#if enabled && campaign}
    {#if imageUrl}<img src={imageUrl} alt={`${campaign.merchant_name} · ${campaign.title}`} width="300" height="124" />
    {:else}<div class="ad-art"><Icon name={slot === 'residential_broadband' ? 'globe' : slot === 'account_service' ? 'user' : 'lock'} size={30}/><span>素材等待本地校验</span></div>{/if}
    <h3>{campaign.title}</h3><p class="ad-description">{campaign.description}</p>
    <div class="ad-merchant">{campaign.merchant_name} · {hostLabel(campaign.target_url)}</div>
    {#if preview}<p class="ad-schedule">{dateTime(campaign.starts_at)} — {dateTime(campaign.ends_at)}</p>{/if}
    <button class="ad-link" disabled={preview || !onclick} onclick={() => onclick?.(slot)}>{campaign.button_text}<Icon name="external" size={15}/></button>
  {:else}
    <div class="ad-empty"><Icon name={slot === 'residential_broadband' ? 'globe' : slot === 'account_service' ? 'user' : 'lock'} size={26}/></div>
    <h3>{enabled ? '暂无有效内容' : '此广告位已关闭'}</h3>
    <p class="ad-description">{enabled ? '内容通过签名与排期校验后显示。' : '不会读取此广告位的素材。'}</p>
    <span class="ad-offline">{preview ? '共用客户端展示组件' : '账号功能继续按环境检查结果工作'}</span>
  {/if}
</article>
<style>
  .ad-card{min-width:0;border:1px solid var(--line);background:var(--paper);padding:18px;border-radius:9px;display:flex;flex-direction:column;gap:10px}.ad-eyebrow{display:flex;justify-content:space-between;font-size:11px;color:var(--muted);letter-spacing:.03em}.ad-eyebrow span:last-child{border:1px solid var(--line);padding:1px 5px;border-radius:3px;font-size:10px}h3{font-size:16px;margin:0;color:var(--ink)}.ad-description{margin:0;color:var(--muted);font-size:12px;line-height:1.7;flex:1;overflow-wrap:anywhere}.ad-empty{color:var(--green);padding:6px 0}.ad-offline,.ad-merchant,.ad-schedule{font-size:10px;color:var(--muted);margin:0;overflow-wrap:anywhere}.ad-art{height:104px;background:var(--tint);color:var(--green);display:flex;flex-direction:column;align-items:center;justify-content:center;gap:8px;font-size:11px;border-radius:5px}img{width:100%;height:124px;object-fit:contain;background:var(--tint);border-radius:5px}.ad-link{color:var(--green);font-size:12px;background:none;border:0;text-align:left;display:flex;gap:6px;align-items:center;padding:4px 0;font-weight:650}.ad-link:disabled{opacity:.6}.inactive{background:transparent}
</style>
