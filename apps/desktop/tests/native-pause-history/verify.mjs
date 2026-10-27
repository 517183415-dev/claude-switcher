import assert from 'node:assert/strict';
import {createHash, createPublicKey, verify} from 'node:crypto';
import {readFileSync, readdirSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';

const out = resolve(process.argv[2]);
const read = name => JSON.parse(readFileSync(join(out, name)));
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const report = read('webview.json');
assert.equal(report.ok, true, report.error);
assert(['tauri://localhost', 'tauri://localhost/'].includes(report.url));
assert.match(report.userAgent, /AppleWebKit/);
const expected = [
  ['runtime', 'runtime_info'], ['initial', 'ad_draft_read'], ['key', 'create_ad_signing_key'],
  ['image', 'import_ad_image'], ['save', 'ad_draft_save'], ['publish-baseline', 'publish_ad'],
  ['single-pause', 'pause_ad'], ['publish-single', 'publish_ad'], ['all-pause', 'pause_ad'],
  ['publish-all', 'publish_ad'], ['unknown-history', 'restore_ad_history'],
  ['restore-history', 'restore_ad_history'], ['publish-history', 'publish_ad']
];
assert.deepEqual(report.rows.map(row => [row.label, row.command]), expected);
const rows = Object.fromEntries(report.rows.map(row => [row.label, row]));
for (const row of report.rows) assert.equal(row.ok, row.label !== 'unknown-history');
assert.match(rows['unknown-history'].error, /history_missing/);
for (const label of ['publish-baseline', 'publish-single', 'publish-all', 'publish-history'])
  assert.equal(rows[label].value.completed, true);
const phases = ['baseline', 'single-draft', 'single-published', 'all-draft',
  'all-published', 'rejected', 'history-draft', 'history-published'];
const s = Object.fromEntries(phases.map(phase => [phase, read(phase + '.json')]));
assert.deepEqual(s['single-draft'].public, s.baseline.public);
assert.deepEqual(s['all-draft'].public, s['single-published'].public);
assert.deepEqual(s.rejected, s['all-published']);
assert.deepEqual(s['history-draft'].public, s['all-published'].public);
const original = rows.save.value.slots;
assert.equal(original.length, 3);
for (const slot of original) {
  assert.equal(slot.enabled, true);
  assert.equal(slot.campaigns.length, 2);
}
const expectedSingle = structuredClone(original);
expectedSingle.find(slot => slot.slot === 'account_service').enabled = false;
assert.deepEqual(rows['single-pause'].value.slots, expectedSingle);
assert.deepEqual(s['single-draft'].state.draft.slots, expectedSingle);
assert.deepEqual(rows['all-pause'].value.slots.map(slot => [slot.enabled, slot.campaigns.length]),
  original.map(() => [false, 0]));
assert.deepEqual(rows['restore-history'].value.slots, original);
assert.deepEqual(s['history-draft'].state.draft.slots, original);

const publicRoot = join(out, 'tmp/business-root/v2/local-ads');
const trust = JSON.parse(readFileSync(join(publicRoot, 'trust.json')));
assert.equal(trust.key_id, 'ads-personal-v1');
const key = createPublicKey({key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'),
  Buffer.from(trust.public_key)]), format: 'der', type: 'spki'});
const manifests = new Map();
for (const name of readdirSync(join(publicRoot, 'production/versions'))) {
  const wire = readFileSync(join(publicRoot, 'production/versions', name));
  const envelope = JSON.parse(wire);
  assert(verify(null, Buffer.from(envelope.payload), key, Buffer.from(envelope.signature)));
  const manifest = JSON.parse(envelope.payload);
  assert.equal(manifest.channel, 'production');
  assert.equal(name, manifest.release_id + '.json');
  manifests.set(sha(wire), manifest);
}
const published = ['baseline', 'single-published', 'all-published', 'history-published']
  .map(phase => manifests.get(s[phase].public['production/current.json'].sha256));
assert(published.every(Boolean));
assert.deepEqual(published[0].slots, original);
assert.deepEqual(published[1].slots, expectedSingle);
assert(published[2].slots.every(slot => !slot.enabled && slot.campaigns.length === 0));
assert.deepEqual(published[3].slots, original);
for (let i = 1; i < published.length; i++)
  assert(published[i].content_revision > published[i - 1].content_revision);
assert.equal(rows['restore-history'].value.release_id, published[3].release_id);
assert.equal(sha(readFileSync(join(publicRoot, 'production/current.json'))),
  s['history-published'].public['production/current.json'].sha256);
assert.equal(read('process-result.json').pid, read('process.json').pid);
assert.equal(read('process-result.json').exit, -15);
assert(readFileSync(join(out, 'runtime.log'), 'utf8').includes('ORDINARY_PUBLISHER_FIXTURE synthetic-memory-vault-only'));
const receipt = {status: 'PASS_NATIVE_WEBVIEW_SLOT_PAUSE_AND_HISTORY_SYNTHETIC_VAULT',
  responses: expected.length, checkpoints: phases,
  single_slot_preserves_other_slots: true, unknown_history_preserves_state: true,
  draft_changes_do_not_publish: true, higher_revision_history_verified: true,
  signature_verifier: 'independent Node crypto Ed25519',
  binary_sha256: read('process.json').binary_sha256,
  source_inputs_sha256: sha(readFileSync(join(out, 'inputs.json'))),
  not_proven: ['unmodified product frontend/startup', 'OS restart', 'real R2/CDN',
    'default Keychain permission', 'whole AD-07 and exhaustive branches']};
writeFileSync(join(out, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
console.log(JSON.stringify(receipt));
