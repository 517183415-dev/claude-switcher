<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import type { ProbeResult } from './generated/ProbeResult';
  let result = $state<ProbeResult | null>(null);
  onMount(() => {
    void (async () => {
      result = await invoke<ProbeResult>('probe');
      let denied = false;
      try { await invoke('forbidden_probe'); } catch { denied = true; }
      await new Promise(requestAnimationFrame);
      await new Promise(requestAnimationFrame);
      const rendered = document.querySelectorAll('[data-slot]').length === 3;
      await invoke('finish', { ok: denied && rendered && result.edition === 'personal', denied, rendered });
    })().catch(error => { void invoke('finish', { ok: false, denied: false, rendered: false }); console.error(String(error)); });
  });
</script>
<main><h1>P0 技术验证</h1><p>本地资源 · Svelte → Rust · 权限校验</p>
{#if result}<section>{#each result.slots as slot}<article data-slot={slot}>{slot}</article>{/each}</section>{/if}
</main>
<style>
  :global(body){margin:0;background:#f5f3eb;color:#1f4037;font:16px system-ui}main{padding:36px}h1{font-size:26px}section{display:flex;gap:12px;margin-top:30px}article{padding:20px;border:1px solid #b5c4b9;border-radius:8px;font-size:13px}
</style>
