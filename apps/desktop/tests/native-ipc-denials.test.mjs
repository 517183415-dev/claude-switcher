import test from 'node:test';
import assert from 'node:assert/strict';
import { verifyDenial } from './native-ipc/denials.mjs';

test('a registered forbidden command must receive a permission denial', () => {
  assert.deepEqual(verifyDenial('forbidden', 'forbidden not allowed', true),
    { command: 'forbidden', kind: 'permission' });
  assert.throws(() => verifyDenial('forbidden', 'Command forbidden not found', true));
  assert.throws(() => verifyDenial('forbidden', 'handler panicked', true));
});

test('an absent command reports its distinct missing or permission outcome', () => {
  assert.deepEqual(verifyDenial('absent', 'Command absent not found'),
    { command: 'absent', kind: 'missing' });
  assert.deepEqual(verifyDenial('absent', 'absent not permitted'),
    { command: 'absent', kind: 'permission' });
  assert.throws(() => verifyDenial('absent', 'invalid payload'));
});
