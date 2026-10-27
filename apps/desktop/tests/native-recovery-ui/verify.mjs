import assert from 'node:assert/strict';
import {createHash, createPublicKey, verify} from 'node:crypto';
import {lstatSync, readFileSync, readdirSync, statSync, writeFileSync} from 'node:fs';
import {join, resolve} from 'node:path';

const out = resolve(process.argv[2]);
const read = name => JSON.parse(readFileSync(join(out, name), 'utf8'));
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const report = read('webview.json');
assert.equal(report.ok, true, report.error);
assert.match(report.url, /^tauri:\/\/localhost/);
assert.match(report.userAgent, /AppleWebKit/);
assert.equal(report.guard_during_reload, true);
assert.equal(report.failed_reload_retry, true);
const relevant = report.rows.filter(row => !row.simulated).map(row => [row.command, row.ok]);
assert.deepEqual(relevant, [['create_ad_signing_key', true], ['publish_ad', true],
  ['publisher_recovery_prepare', true], ['publisher_recovery_cancel', true],
  ['publisher_recovery_prepare', true], ['publisher_recovery_confirm', true], ['publish_ad', true]]);
assert(report.rows.some(row => row.command === 'workspace_snapshot' && row.simulated));
assert(report.rows.some(row => row.command === 'check_environment' && row.simulated));
const phases = ['baseline', 'prepared', 'canceled', 'confirmed', 'republished'];
const snapshots = Object.fromEntries(phases.map(phase => [phase, read(phase + '.json')]));
const {baseline, prepared, canceled, confirmed, republished} = snapshots;
assert.deepEqual(prepared, baseline);
assert.deepEqual(canceled, baseline);
assert.notEqual(confirmed.active.generation, baseline.active.generation);
assert.equal(confirmed.states[baseline.active.generation], baseline.states[baseline.active.generation]);
assert.deepEqual(confirmed.public, baseline.public);
assert.deepEqual(confirmed.state.draft.slots, baseline.state.draft.slots);
assert(confirmed.state.draft.content_revision > baseline.state.draft.content_revision);
assert.equal(republished.active.generation, confirmed.active.generation);
const root = join(out, 'tmp/business-root');
const backupRoot = join(root, 'v2/personal/publisher/backups');
const backups = readdirSync(backupRoot).sort();
assert.deepEqual(backups, [...report.backups].sort());
assert.equal(new Set(backups).size, 2);
for (const name of backups) {
  const metadata = statSync(join(backupRoot, name));
  assert.equal(metadata.mode & 0o777, 0o600);
  assert(metadata.size > 0);
}
const publicRoot = join(root, 'v2/local-ads');
const trust = JSON.parse(readFileSync(join(publicRoot, 'trust.json')));
const key = createPublicKey({key: Buffer.concat([Buffer.from('302a300506032b6570032100', 'hex'),
  Buffer.from(trust.public_key)]), format: 'der', type: 'spki'});
const versions = new Map();
for (const name of readdirSync(join(publicRoot, 'preview/versions'))) {
  const wire = readFileSync(join(publicRoot, 'preview/versions', name));
  const envelope = JSON.parse(wire);
  assert(verify(null, Buffer.from(envelope.payload), key, Buffer.from(envelope.signature)));
  const manifest = JSON.parse(envelope.payload);
  assert.equal(name, manifest.release_id + '.json');
  versions.set(sha(wire), manifest);
}
const before = versions.get(baseline.public['preview/current.json'].sha256);
const after = versions.get(republished.public['preview/current.json'].sha256);
assert(before && after);
assert(after.content_revision > before.content_revision);
assert.deepEqual(after.slots, before.slots);
assert.equal(read('process-result.json').pid, read('process.json').pid);
assert.equal(read('process-result.json').exit, -15);
const secret = Buffer.from('synthetic-recovery-ui-passphrase');
const scan = path => {
  const metadata = lstatSync(path, {throwIfNoEntry: true});
  assert(!metadata.isSymbolicLink(), `unexpected symlink: ${path}`);
  if (metadata.isDirectory()) {
    for (const name of readdirSync(path)) scan(join(path, name));
  } else assert(!readFileSync(path).includes(secret), `synthetic password leaked: ${path}`);
};
for (const path of [root, join(out, 'webview.json'), join(out, 'runtime.log'), ...phases.map(phase => join(out, phase + '.json'))]) scan(path);
const receipt = {status: 'PASS_PRODUCT_NATIVE_RECOVERY_UI_SYNTHETIC_VAULT', responses: relevant.length,
  checkpoints: phases, guard_during_delayed_reload: true, failed_reload_requires_retry: true,
  cancel_preserves_state: true, confirmation_does_not_publish: true,
  old_generation_preserved: true, explicit_republish_signature_verified: true,
  binary_sha256: read('process.json').binary_sha256, inputs_sha256: sha(readFileSync(join(out, 'inputs.json'))),
  not_proven: ['default Keychain permission', 'full desktop restart', 'R2/CDN', 'whole AD-10']};
writeFileSync(join(out, 'receipt.json'), JSON.stringify(receipt, null, 2) + '\n');
console.log(JSON.stringify(receipt));
