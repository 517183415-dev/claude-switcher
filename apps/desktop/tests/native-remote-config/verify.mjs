import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
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
  ['runtime', 'runtime_info'], ['key', 'create_ad_signing_key'], ['initial', 'publisher_remote_read'],
  ['preview', 'publisher_remote_save'], ['wrong-channel', 'publisher_remote_save'],
  ['shared-access', 'publisher_remote_save'], ['after-rejection', 'publisher_remote_read'],
  ['production', 'publisher_remote_save'], ['second-preview', 'publisher_remote_save'],
  ['wrong-retained-channel', 'publisher_remote_select'], ['restore-preview', 'publisher_remote_select'],
  ['select-local', 'publisher_remote_save'], ['final-read', 'publisher_remote_read']
];
assert.deepEqual(report.rows.map(r => [r.label, r.command]), expected);
const rows = Object.fromEntries(report.rows.map(r => [r.label, r]));
const rejected = ['wrong-channel', 'shared-access', 'wrong-retained-channel'];
for (const row of report.rows) assert.equal(row.ok, !rejected.includes(row.label));
assert.deepEqual(rows.initial.value, {preview: null, production: null, targets: [], unavailable_targets: []});
assert.deepEqual(rows['after-rejection'].value, rows.preview.value);
assert.equal(rows.production.value.production.bucket, 'ads-production');
assert.equal(rows['second-preview'].value.preview.bucket, 'ads-preview-two');
assert.equal(rows['restore-preview'].value.preview.bucket, 'ads-preview');
assert.equal(rows['final-read'].value.preview, null);
assert.equal(rows['final-read'].value.production.bucket, 'ads-production');
assert.equal(rows['final-read'].value.targets.length, 3);
assert.deepEqual(rows['final-read'].value, rows['select-local'].value);
const snapshots = Object.fromEntries(['baseline', 'preview', 'rejected', 'selected', 'local'].map(p => [p, read(p + '.json')]));
assert.deepEqual(snapshots.rejected, snapshots.preview);
for (const snapshot of Object.values(snapshots)) {
  assert.deepEqual(snapshot.public, snapshots.baseline.public);
  assert.equal(snapshot.active.generation, snapshots.baseline.active.generation);
}
const forbidden = ['synthetic-preview-access', 'synthetic-preview-secret', 'synthetic-production-access',
  'synthetic-production-secret', 'synthetic-preview-two-access', 'synthetic-preview-two-secret'];
const check = bytes => { for (const secret of forbidden) assert(!bytes.includes(secret), 'credential leaked'); };
check(readFileSync(join(out, 'webview.json')));
check(readFileSync(join(out, 'runtime.log')));
function checkTree(dir) {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name);
    const metadata = statSync(path);
    if (metadata.isDirectory()) checkTree(path);
    else if (metadata.isFile()) check(readFileSync(path));
  }
}
checkTree(join(out, 'tmp/business-root'));
assert.equal(read('process-result.json').pid, read('process.json').pid);
assert.equal(read('process-result.json').exit, -15);
assert(readFileSync(join(out, 'runtime.log'), 'utf8').includes('ORDINARY_PUBLISHER_FIXTURE synthetic-memory-vault-only'));
const receipt = {status: 'PASS_NATIVE_WEBVIEW_REMOTE_CONFIG_SYNTHETIC_VAULT', responses: expected.length,
  checkpoints: Object.keys(snapshots), credential_not_in_responses_or_disk: true,
  channel_separation_and_retained_selection: true, no_publication: true,
  binary_sha256: read('process.json').binary_sha256, inputs_sha256: sha(readFileSync(join(out, 'inputs.json'))),
  not_proven: ['Svelte user interface clicks', 'default Keychain persistence', 'R2/CDN or real credentials']};
writeFileSync(join(out, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
console.log(JSON.stringify(receipt));
