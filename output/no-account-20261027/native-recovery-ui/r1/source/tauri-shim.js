// Reuse the isolated startup shim; record only recovery and publication outcomes.
import {invoke as isolatedInvoke} from '../native-remote-ui/tauri-shim.js';

let holdNextReload = false;
export async function invoke(command, args = {}, options) {
  const observed = command.startsWith('publisher_recovery_') || command === 'publish_ad';
  try {
    const value = await isolatedInvoke(command, args, options);
    if (command === 'publisher_recovery_confirm') holdNextReload = true;
    if (command === 'ad_draft_read' && holdNextReload) {
      holdNextReload = false;
      window.__recoveryReloadWaiting = true;
      await new Promise(resolve => { window.__releaseRecoveryReload = resolve; });
    }
    if (observed) window.__remoteUiRows.push({command, ok: true});
    return value;
  } catch (error) {
    if (observed) window.__remoteUiRows.push({command, ok: false});
    throw error;
  }
}
