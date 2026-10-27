import assert from 'node:assert/strict';
import {createHash, createPublicKey, verify} from 'node:crypto';
import {readFileSync, readdirSync, statSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';

const out = resolve(process.argv[2]);
const read = name => JSON.parse(readFileSync(join(out, name)));
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const report = read('webview.json');
assert.equal(report.ok, true, report.error);
assert(['tauri://localhost', 'tauri://localhost/'].includes(report.url));
assert.match(report.userAgent, /AppleWebKit/);
const labels = ['runtime', 'initial', 'key', 'duplicate-key', 'image', 'save', 'preview', 'production',
  'backup', 'pause', 'paused-production', 'wrong-password', 'restore', 'restored-draft',
  'recovery-status', 'republished', 'ready-status'];
const commands = ['runtime_info', 'ad_draft_read', 'create_ad_signing_key', 'create_ad_signing_key',
  'import_ad_image', 'ad_draft_save', 'publish_ad', 'publish_ad', 'export_publisher_backup',
  'pause_ad', 'publish_ad', 'restore_publisher_backup', 'restore_publisher_backup', 'ad_draft_read',
  'publisher_status', 'publish_ad', 'publisher_status'];
assert.deepEqual(report.rows.map(r => r.label), labels);
assert.deepEqual(report.rows.map(r => r.command), commands);
const rows = Object.fromEntries(report.rows.map(r => [r.label, r]));
for (const row of report.rows) assert.equal(row.ok, !['duplicate-key', 'wrong-password'].includes(row.label));
assert.match(rows['duplicate-key'].error, /signing_key_already_configured/);
assert.match(rows['wrong-password'].error, /backup/);
assert.equal(rows.runtime.value.edition, 'personal');
assert.equal(rows.restore.value.completed, false);
assert.equal(rows['recovery-status'].value.status, 'recovery_required');
assert.equal(rows['ready-status'].value.status, 'ready_local');

const checkpoints = Object.fromEntries(['saved', 'paused', 'wrong-password', 'restored', 'republished'].map(p => [p, read(p + '.json')]));
const {saved, paused, restored, republished} = checkpoints;
assert.deepEqual(checkpoints['wrong-password'], paused);
assert.deepEqual(restored.public, paused.public);
assert.notEqual(restored.active.generation, paused.active.generation);
assert.equal(restored.states[paused.active.generation], paused.states[paused.active.generation]);
assert.equal(restored.state.recovery_required, true);
assert.equal(republished.state.recovery_required, false);
assert.deepEqual(restored.state.draft.slots, rows.save.value.slots);
assert.deepEqual(republished.state.draft.slots, rows.save.value.slots);
assert.deepEqual(rows['restored-draft'].value, restored.state.draft);
assert.equal(rows.save.value.slots.length, 3);
for (const slot of rows.save.value.slots) assert.equal(slot.campaigns.length, 2);
const root = join(out, 'tmp/business-root');
const publicRoot = join(root, 'v2/local-ads');
const trust = JSON.parse(readFileSync(join(publicRoot, 'trust.json')));
assert.equal(trust.key_id, 'ads-personal-v1');
assert.equal(trust.public_key.length, 32);
const key = createPublicKey({key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), Buffer.from(trust.public_key)]), format: 'der', type: 'spki'});
const wires = new Map();
for (const channel of ['preview', 'production']) {
  for (const name of readdirSync(join(publicRoot, channel, 'versions'))) {
    const wire = readFileSync(join(publicRoot, channel, 'versions', name));
    const envelope = JSON.parse(wire);
    assert.equal(envelope.signature.length, 64);
    assert(verify(null, Buffer.from(envelope.payload), key, Buffer.from(envelope.signature)));
    const manifest = JSON.parse(envelope.payload);
    assert.equal(manifest.channel, channel);
    assert.equal(manifest.key_id, trust.key_id);
    assert.equal(name, manifest.release_id + '.json');
    wires.set(sha(wire), manifest);
    for (const slot of manifest.slots) for (const campaign of slot.campaigns) {
      const image = campaign.image;
      const path = join(publicRoot, channel, 'assets', image.sha256 + '.png');
      const bytes = readFileSync(path);
      assert.equal(sha(bytes), image.sha256);
      assert.equal(statSync(path).mode & 0o777, 0o600);
      assert(!bytes.includes(Buffer.from('SYNTHETIC_PNG_METADATA')));
      assert.equal(bytes.readUInt32BE(16), image.width);
      assert.equal(bytes.readUInt32BE(20), image.height);
    }
  }
  assert(wires.has(sha(readFileSync(join(publicRoot, channel, 'current.json')))));
}
const manifest = (snapshot, channel) => {
  const value = wires.get(snapshot.public[channel + '/current.json'].sha256);
  assert(value, 'checkpoint current must match an independently verified immutable');
  return value;
};
assert.deepEqual(manifest(saved, 'production').slots, rows.save.value.slots);
assert.deepEqual(manifest(saved, 'preview').slots, rows.save.value.slots);
assert(manifest(paused, 'production').content_revision > manifest(saved, 'production').content_revision);
assert(manifest(republished, 'production').content_revision > manifest(paused, 'production').content_revision);
assert.deepEqual(manifest(republished, 'production').slots, manifest(saved, 'production').slots);
assert.equal(manifest(paused, 'production').slots.every(s => !s.enabled && !s.campaigns.length), true);
const backups = readdirSync(join(root, 'v2/personal/publisher/backups'));
assert.equal(backups.length, 1);
assert.equal(statSync(join(root, 'v2/personal/publisher/backups', backups[0])).mode & 0o777, 0o600);
const processReceipt = read('process-result.json');
assert.equal(processReceipt.pid, read('process.json').pid);
assert.equal(processReceipt.exit, -15);
assert(readFileSync(join(out, 'runtime.log'), 'utf8').includes('ORDINARY_PUBLISHER_FIXTURE synthetic-memory-vault-only'));
const receipt = {status: 'PASS_NATIVE_WEBVIEW_PRODUCT_HANDLERS_SYNTHETIC_VAULT', responses: labels.length,
  checkpoints: Object.keys(checkpoints), signature_verifier: 'independent Node crypto Ed25519',
  generations_preserved: true, wrong_password_preserves_state: true, restore_does_not_publish: true,
  resumed_above_paused_revision: true, binary_sha256: read('process.json').binary_sha256,
  source_inputs_sha256: sha(readFileSync(join(out, 'inputs.json'))),
  not_proven: ['user-default Keychain permission', 'unmodified product frontend/startup', 'real R2/CDN',
    'system-managed WebKit helper cleanup', 'whole AD-10 and exhaustive branches']};
writeFileSync(join(out, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
console.log(JSON.stringify(receipt));
