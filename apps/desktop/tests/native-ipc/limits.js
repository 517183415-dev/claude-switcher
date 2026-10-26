import { image } from './payloads.mjs';
import { verifyDenial } from './denials.mjs';
/* Compiled only into the ipc-limits fixture; no production page imports this file. */
'use strict';
(async () => {
  const invoke = window.__TAURI_INTERNALS__.invoke;
  const denials = [];
  const status = document.getElementById('status');
  function assert(condition, message) { if (!condition) throw new Error(message); }
  async function denied(command, args = {}, registered = false) {
    try { await invoke(command, args); }
    catch (error) {
      denials.push(verifyDenial(command, error, registered));
      return;
    }
    throw new Error(`unexpectedly allowed: ${command}`);
  }

  try {
    const info = await invoke('ipc_limits_info');
    assert(info.fixture === 'ipc-limits' && info.ordinary_runtime === false, 'wrong runtime');
    await denied('ipc_limits_forbidden', {}, true);
    await denied('workspace_snapshot');
    if (info.edition === 'public') {
      await denied('ipc_limits_image', { bytes: [0] });
      await denied('ipc_limits_backup', { bytes: [0], passphrase: 'synthetic' });
    } else if (!info.preflight) {
      for (const length of [1048575, 1048576, 1048577]) {
        status.textContent = `验证图片 ${length} 字节`;
        const receipt = await invoke('ipc_limits_image', { bytes: image(length) });
        assert(receipt.input_len === length, 'image transport length mismatch');
        if (length <= 1048576) {
          assert(receipt.entered === 1 && receipt.error === null && receipt.asset, `image import: ${JSON.stringify(receipt)}`);
        } else {
          assert(receipt.entered === 0 && /1 MiB/.test(receipt.error), 'image size guard');
        }
      }
      for (const length of [67108863, 67108864, 67108865]) {
        status.textContent = `验证备份 ${length} 字节`;
        const receipt = await invoke('ipc_limits_backup', { bytes: new Array(length).fill(0), passphrase: 'synthetic' });
        assert(receipt.input_len === length, 'backup transport length mismatch');
        if (length <= 67108864) {
          assert(receipt.entered === 1 && receipt.error === 'acceptance_restore_entered', 'backup restore seam');
        } else {
          assert(receipt.entered === 0 && /64 MiB/.test(receipt.error), 'backup size guard');
        }
        assert(receipt.decryption_or_vault === false, 'unexpected restore implementation');
      }
    }
    status.textContent = '验证通过';
    await invoke('ipc_limits_finish', { ok: true, denials, error: null });
  } catch (error) {
    status.textContent = String(error);
    await invoke('ipc_limits_finish', { ok: false, denials, error: String(error) });
  }
})();
