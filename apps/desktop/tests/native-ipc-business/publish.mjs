import { verifyDenial } from './denials.mjs';
import { imageSources } from './image-fixtures.mjs';

const commands = ['ipc_business_publish_init', 'ipc_business_publish', 'publisher_status', 'restore_ad_history',
  'preview_publisher_cleanup', 'commit_publisher_cleanup', 'preview_publisher_gc', 'commit_publisher_gc'];
const options = { retain: 2, releaseResolution: false };
const demand = (condition, message) => { if (!condition) throw Error(message); };

export async function runPublish(invoke, edition, phase, responses) {
  if (edition === 'public') {
    const denials = [];
    for (const command of commands) {
      try { await invoke(command, {}); }
      catch (reason) { denials.push(verifyDenial(command, reason, true)); continue; }
      throw Error(`Public command unexpectedly allowed: ${command}`);
    }
    responses.publish = { phase, denials };
    return;
  }
  if (phase === 'read') {
    responses.publish = { phase, draft: await invoke('ad_draft_read'), status: await invoke('publisher_status') };
    return;
  }
  if (phase === 'resume') {
    const beforeDraft = await invoke('ad_draft_read');
    const beforeStatus = await invoke('publisher_status');
    await invoke('ipc_business_publish_init');
    const saved = await invoke('ad_draft_save', { manifest: beforeDraft });
    const outcome = await invoke('ipc_business_publish', { channel: 'production' });
    demand(outcome.completed && saved.content_revision > beforeDraft.content_revision, 'restart publication failed');
    responses.publish = { phase, beforeDraft, beforeStatus, saved, outcome,
      draft: await invoke('ad_draft_read'), status: await invoke('publisher_status') };
    return;
  }
  demand(phase === 'write', 'invalid publish phase');
  await invoke('ipc_business_publish_init');
  const png = Array.from(atob(imageSources['backup-source.png']), value => value.charCodeAt(0));
  const image = (await invoke('import_ad_image', { bytes: png })).asset;
  const draft = await invoke('ad_draft_read');
  draft.slots[0].enabled = true;
  draft.slots[0].campaigns = [{ campaign_id: 'synthetic-publish', merchant_id: 'synthetic-merchant',
    merchant_name: 'Synthetic merchant', title: 'Synthetic', description: 'Synthetic only', image,
    button_text: 'View', target_url: 'https://synthetic.example.com/', starts_at: 1000, ends_at: 2000, enabled: true }];
  const saved = await invoke('ad_draft_save', { manifest: draft });
  for (const channel of ['preview', 'production']) demand((await invoke('ipc_business_publish', { channel })).completed, 'publish failed');
  const firstStatus = await invoke('publisher_status');
  const first = firstStatus.history.find(item => item.channel === 'production');
  demand(first && first.revision === saved.content_revision, 'initial history mismatch');
  const restored = await invoke('restore_ad_history', { releaseId: first.release_id });
  demand(restored.content_revision > first.revision && restored.release_id !== first.release_id, 'history restore did not advance');
  await invoke('pause_ad', { slot: null });
  for (const channel of ['preview', 'production']) demand((await invoke('ipc_business_publish', { channel })).completed, 'pause publish failed');
  const stale = await invoke('preview_publisher_cleanup', options);
  await invoke('pause_ad', { slot: null });
  const before = await invoke('publisher_status');
  const beforeDisk = await invoke('ipc_business_draft_snapshot', { label: 'publish-stale-before' });
  let staleError = null;
  try { await invoke('commit_publisher_cleanup', { ...options, digest: stale.digest }); }
  catch (error) { staleError = String(error); }
  demand(staleError && staleError.includes('cleanup_stale_preview'), 'stale preview was not rejected');
  const after = await invoke('publisher_status');
  const afterDisk = await invoke('ipc_business_draft_snapshot', { label: 'publish-stale-after' });
  demand(JSON.stringify(beforeDisk.files) === JSON.stringify(afterDisk.files), 'stale commit changed disk');
  demand(JSON.stringify(before) === JSON.stringify(after), 'stale commit changed state');
  const preview = await invoke('preview_publisher_cleanup', options);
  demand(preview.candidate_assets.length === 1, 'expected one unreferenced asset');
  const cleanup = await invoke('commit_publisher_cleanup', { ...options, digest: preview.digest });
  const gc = await invoke('preview_publisher_gc');
  demand(gc.eligible.length === 1 && gc.eligible[0] === image.sha256, 'asset not eligible for GC');
  const committed = await invoke('commit_publisher_gc', { digest: gc.digest });
  demand(cleanup.completed && committed.completed, 'cleanup/GC failed');
  responses.publish = { phase, saved, firstStatus, restored, staleError, before, after, beforeDisk, afterDisk, preview, gc, cleanup, committed,
    draft: await invoke('ad_draft_read'), status: await invoke('publisher_status'), image };
}
