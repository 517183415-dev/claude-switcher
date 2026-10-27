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
const expected = [
  ['runtime', 'runtime_info'], ['key', 'create_ad_signing_key'], ['preview', 'publish_ad'],
  ['prepare-cancel', 'publisher_recovery_prepare'], ['duplicate-prepare', 'publisher_recovery_prepare'],
  ['cancel', 'publisher_recovery_cancel'], ['confirm-canceled', 'publisher_recovery_confirm'],
  ['prepare-rejected', 'publisher_recovery_prepare'], ['confirm-invalid', 'publisher_recovery_confirm'],
  ['confirm-consumed', 'publisher_recovery_confirm'], ['prepare-confirmed', 'publisher_recovery_prepare'],
  ['confirm', 'publisher_recovery_confirm'], ['retained', 'publisher_retained_list'], ['republish', 'publish_ad']
];
assert.deepEqual(report.rows.map(r => [r.label, r.command]), expected);
const rows = Object.fromEntries(report.rows.map(r => [r.label, r]));
const rejected = ['duplicate-prepare', 'confirm-canceled', 'confirm-invalid', 'confirm-consumed'];
for (const row of report.rows) assert.equal(row.ok, !rejected.includes(row.label));
assert.match(rows['confirm-invalid'].error, /recovery_stale_confirmation/);
const snapshots = Object.fromEntries(['baseline', 'prepared', 'canceled', 'rejected', 'confirmed', 'republished'].map(p => [p, read(p + '.json')]));
const {baseline, prepared, canceled, confirmed, republished} = snapshots;
assert.deepEqual(prepared, baseline);
assert.deepEqual(canceled, baseline);
assert.deepEqual(snapshots.rejected, baseline);
assert.notEqual(confirmed.active.generation, baseline.active.generation);
assert.equal(confirmed.states[baseline.active.generation], baseline.states[baseline.active.generation]);
assert.deepEqual(confirmed.public, baseline.public);
assert.deepEqual(confirmed.state.draft.slots, baseline.state.draft.slots);
assert(confirmed.state.draft.content_revision > baseline.state.draft.content_revision);
assert.equal(republished.active.generation, confirmed.active.generation);
assert(rows.retained.value.some(item => item.generation === baseline.active.generation && item.state_verified));
const root = join(out, 'tmp/business-root');
const backupRoot = join(root, 'v2/personal/publisher/backups');
const backups = readdirSync(backupRoot).sort();
const previewNames = ['prepare-cancel', 'prepare-rejected', 'prepare-confirmed'].map(label => rows[label].value.backup_filename).sort();
assert.deepEqual(backups, previewNames);
assert.equal(new Set(backups).size, 3);
for (const name of backups) {
  const path = join(backupRoot, name);
  assert.equal(statSync(path).mode & 0o777, 0o600);
  assert(statSync(path).size > 0);
}
const publicRoot = join(root, 'v2/local-ads');
const trust = JSON.parse(readFileSync(join(publicRoot, 'trust.json')));
const key = createPublicKey({key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'), Buffer.from(trust.public_key)]), format: 'der', type: 'spki'});
const versions = new Map();
for (const name of readdirSync(join(publicRoot, 'preview/versions'))) {
  const wire = readFileSync(join(publicRoot, 'preview/versions', name));
  const envelope = JSON.parse(wire);
  assert(verify(null, Buffer.from(envelope.payload), key, Buffer.from(envelope.signature)));
  const manifest = JSON.parse(envelope.payload);
  assert.equal(name, manifest.release_id + '.json');
  assert.equal(manifest.channel, 'preview');
  versions.set(sha(wire), manifest);
}
const before = versions.get(baseline.public['preview/current.json'].sha256);
const after = versions.get(republished.public['preview/current.json'].sha256);
assert(before && after);
assert(after.content_revision > before.content_revision);
assert.deepEqual(after.slots, before.slots);
assert.equal(sha(readFileSync(join(publicRoot, 'preview/current.json'))), republished.public['preview/current.json'].sha256);
assert.equal(read('process-result.json').pid, read('process.json').pid);
assert.equal(read('process-result.json').exit, -15);
assert(readFileSync(join(out, 'runtime.log'), 'utf8').includes('ORDINARY_PUBLISHER_FIXTURE synthetic-memory-vault-only'));
const receipt = {status: 'PASS_NATIVE_WEBVIEW_RECOVERY_CONFIRM_CANCEL_SYNTHETIC_VAULT', responses: expected.length,
  checkpoints: Object.keys(snapshots), canceled_and_invalid_preserve_state: true, old_generation_preserved: true,
  confirmation_does_not_publish: true, republish_signature_verified: true,
  binary_sha256: read('process.json').binary_sha256, inputs_sha256: sha(readFileSync(join(out, 'inputs.json'))),
  not_proven: ['native product UI dialog', 'default Keychain permission', 'R2/CDN', 'whole AD-10']};
writeFileSync(join(out, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
console.log(JSON.stringify(receipt));
