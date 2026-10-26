import { verifyDenial } from './denials.mjs';
const clone = value => JSON.parse(JSON.stringify(value));
const commands = ['ad_draft_read', 'ad_draft_save', 'pause_ad'];

export async function runDraft(invoke, edition, responses) {
  if (edition === 'public') {
    responses.draft = [];
    for (const command of commands) {
      try { await invoke(command, {}); }
      catch (reason) { responses.draft.push(verifyDenial(command, reason, true)); continue; }
      throw new Error('public draft command unexpectedly allowed');
    }
    return;
  }
  const initial = await invoke('ad_draft_read');
  responses.initial = initial;
  const slots = ['residential_broadband', 'account_service', 'kyc_service'];
  const filled = () => {
    const manifest = clone(initial);
    manifest.slots.forEach((slot, i) => {
      slot.enabled = true;
      slot.campaigns = [0, 1].map(j => ({
        campaign_id: `synthetic-${i}-${j}`, merchant_id: 'synthetic-merchant',
        merchant_name: 'Synthetic merchant', title: `Synthetic ${i}/${j}`, description: 'Synthetic draft only',
        image: { sha256: 'a'.repeat(64), mime: 'image/png', bytes: 1, width: 1, height: 1 },
        button_text: 'View', target_url: 'https://synthetic.example.com/',
        starts_at: 1000 + j * 1000, ends_at: 2000 + j * 1000, enabled: true
      }));
    });
    return manifest;
  };
  const current = filled();
  const cases = [['save_six', 'ad_draft_save', { manifest: clone(current) }, true]];
  for (let i = 0; i < 3; i++) {
    current.slots[i].campaigns[1].title = `Edited ${i}`;
    cases.push([`edit_${i}`, 'ad_draft_save', { manifest: clone(current) }, true]);
  }
  slots.forEach((slot, i) => cases.push([`pause_${i}`, 'pause_ad', { slot }, true]));
  cases.push(['resume_six', 'ad_draft_save', { manifest: filled() }, true]);
  cases.push(['pause_all', 'pause_ad', { slot: null }, true]);
  const bad = (name, mutate) => {
    const manifest = filled(); mutate(manifest);
    cases.push([name, 'ad_draft_save', { manifest }, false]);
  };
  bad('overlap', m => { m.slots[0].campaigns[1].starts_at = 1500; });
  bad('duplicate_slot', m => { m.slots[1].slot = slots[0]; });
  bad('invalid_url', m => { m.slots[0].campaigns[0].target_url = 'http://synthetic.example/'; });
  bad('reversed_time', m => { m.slots[0].campaigns[0].ends_at = 999; });
  bad('unknown_field', m => { m.unknown = true; });
  bad('missing_slots', m => { delete m.slots; });
  bad('wrong_slots_type', m => { m.slots = 'wrong'; });
  cases.push(['missing_manifest', 'ad_draft_save', {}, false]);
  cases.push(['invalid_pause', 'pause_ad', { slot: 'unknown' }, false]);
  responses.draft = [];
  for (const [id, command, input, accepted] of cases) {
    const before = await invoke('ipc_business_draft_snapshot', { label: `${id}:before` });
    let value = null, error = null;
    try { value = await invoke(command, input); } catch (reason) { error = String(reason); }
    const after = await invoke('ipc_business_draft_snapshot', { label: `${id}:after` });
    const readback = await invoke('ad_draft_read');
    responses.draft.push({ id, command, input, accepted: error === null, value, readback, error, before, after });
    if ((error === null) !== accepted) throw new Error(`unexpected draft result: ${id}`);
  }
}
