import test from 'node:test';
import assert from 'node:assert/strict';
import { crc32, inflateSync } from 'node:zlib';
import { image } from './native-ipc/payloads.mjs';

test('native IPC image payloads have exact lengths and valid PNG chunks', () => {
  for (const length of [1048575, 1048576, 1048577]) {
    const bytes = Buffer.from(image(length));
    assert.equal(bytes.length, length);
    assert.deepEqual(bytes.subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
    const chunks = [];
    let offset = 8;
    while (offset < bytes.length) {
      const size = bytes.readUInt32BE(offset);
      const type = bytes.subarray(offset + 4, offset + 8).toString('ascii');
      const end = offset + 8 + size;
      assert.ok(end + 4 <= bytes.length, `chunk length: ${type}`);
      assert.equal(crc32(bytes.subarray(offset + 4, end)), bytes.readUInt32BE(end), `CRC: ${type}`);
      if (type === 'IHDR') {
        assert.equal(bytes.readUInt32BE(offset + 8), 1);
        assert.equal(bytes.readUInt32BE(offset + 12), 1);
      }
      if (type === 'IDAT') assert.deepEqual(inflateSync(bytes.subarray(offset + 8, end)), Buffer.alloc(5));
      chunks.push(type);
      offset = end + 4;
    }
    assert.equal(offset, length);
    assert.deepEqual(chunks, ['IHDR', 'IDAT', 'paDd', 'IEND']);
  }
});
