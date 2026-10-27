import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import path from 'node:path';
const root = process.argv[2];
const read = name => JSON.parse(readFileSync(path.join(root, name), 'utf8'));
const result = read('webview.json');
assert.equal(result.ok, true, result.error);
assert.match(result.url, /^tauri:\/\/localhost/);
const relevant = result.rows.filter(row => !row.simulated).map(row => [row.command, row.channel]);
assert.deepEqual(relevant, [['create_ad_signing_key', null], ['publisher_remote_read', null],
  ['publisher_remote_save', 'preview'], ['publisher_remote_save', 'production'],
  ['publisher_remote_save', 'preview'], ['publisher_remote_select', 'preview']]);
assert.ok(result.rows.filter(row => !row.simulated).every(row => row.ok));
assert.ok(result.rows.some(row => row.command === 'workspace_snapshot' && row.simulated));
assert.ok(result.rows.some(row => row.command === 'check_environment' && row.simulated));
for (const name of ['webview.json', 'runtime.log', 'baseline.json', 'done.json']) {
  assert.doesNotMatch(readFileSync(path.join(root, name), 'utf8'), /synthetic-ui-(preview|production)-(access|secret)/);
}
console.log('PASS: native product page remote configuration; bootstrap checks simulated');
