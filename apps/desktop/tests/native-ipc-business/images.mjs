import { verifyDenial } from './denials.mjs';
import { imageSources } from './image-fixtures.mjs';
const commands = ['import_ad_image', 'ad_image_preview'];
async function localBytes(path) {
  const encoded = imageSources[path];
  if (!encoded) throw new Error('embedded synthetic image unavailable');
  return Array.from(atob(encoded), value => value.charCodeAt(0));
}
function exactPng(source) {
  const length = 1048576 - source.length - 12;
  const chunk = new Uint8Array(length + 12);
  new DataView(chunk.buffer).setUint32(0, length);
  chunk.set([110,112,65,68], 4);
  let crc = 0xffffffff;
  for (const byte of chunk.subarray(4, chunk.length - 4)) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
  }
  new DataView(chunk.buffer).setUint32(chunk.length - 4, (crc ^ 0xffffffff) >>> 0);
  return [...source.slice(0, -12), ...chunk, ...source.slice(-12)];
}
export async function runImages(invoke, edition, responses) {
  responses.images = [];
  if (edition === 'public') {
    for (const command of commands) {
      try { await invoke(command, {}); }
      catch (reason) { responses.images.push(verifyDenial(command, reason, true)); continue; }
      throw new Error('Public image command unexpectedly allowed');
    }
    return;
  }
  responses.initial = await invoke('ad_draft_read');
  const png = await localBytes('backup-source.png'), jpeg = await localBytes('image-source.jpg');
  const cases = [['png',png,true],['jpeg',jpeg,true],['exact_png',exactPng(png),true],
    ['corrupt_png',[137,80,78,71,13,10,26,10],false],['corrupt_jpeg',[255,216,255],false],
    ['unsupported',[71,73,70,56,57,97],false],['oversize',new Array(1048577).fill(0),false]];
  for (const [id, bytes, accepted] of cases) {
    const before = await invoke('ipc_business_draft_snapshot',{label:`${id}:before`});
    let value=null, error=null, preview=null;
    try { value=await invoke('import_ad_image',{bytes}); preview=await invoke('ad_image_preview',{sha256:value.asset.sha256}); }
    catch (reason) { error=String(reason); }
    const after = await invoke('ipc_business_draft_snapshot',{label:`${id}:after`});
    const draft = await invoke('ad_draft_read');
    responses.images.push({id,size:bytes.length,accepted:error===null,value,preview,error,before,after,draft});
    if ((error===null)!==accepted) throw new Error(`unexpected image result ${id}`);
  }
  const before=await invoke('ipc_business_draft_snapshot',{label:'missing_preview:before'});
  let error=null;
  try { await invoke('ad_image_preview',{sha256:'f'.repeat(64)}); } catch(reason) { error=String(reason); }
  const after=await invoke('ipc_business_draft_snapshot',{label:'missing_preview:after'});
  responses.images.push({id:'missing_preview',size:0,accepted:false,value:null,preview:null,error,before,after,draft:await invoke('ad_draft_read')});
  if(error===null) throw new Error('missing image preview allowed');
}
