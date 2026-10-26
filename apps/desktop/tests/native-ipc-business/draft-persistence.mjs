import { verifyDenial } from './denials.mjs';
const commands=['ad_draft_read','ad_draft_save','pause_ad'];

function sixCampaigns(initial) {
  const manifest=JSON.parse(JSON.stringify(initial));
  manifest.slots.forEach((slot,i)=>{
    slot.enabled=true;
    slot.campaigns=[0,1].map(j=>({
      campaign_id:`synthetic-${i}-${j}`,merchant_id:'synthetic-merchant',
      merchant_name:'Synthetic merchant',title:`Synthetic ${i}/${j}`,description:'Synthetic draft only',
      image:{sha256:'a'.repeat(64),mime:'image/png',bytes:1,width:1,height:1},
      button_text:'View',target_url:'https://synthetic.example.com/',
      starts_at:1000+j*1000,ends_at:2000+j*1000,enabled:true
    }));
  });
  return manifest;
}

export async function runDraftPersistence(invoke,edition,phase,responses) {
  if(phase==='probe') {
    if(edition!=='public')throw Error('draft probe edition mismatch');
    const denials=[];
    for(const command of commands){
      try{await invoke(command,{});}
      catch(reason){denials.push(verifyDenial(command,reason,true));continue;}
      throw Error('Public draft command unexpectedly allowed');
    }
    responses.draft_persistence={phase,denials};
    return;
  }
  if(edition!=='personal')throw Error('draft writer edition mismatch');
  if(phase==='save') {
    const initial=await invoke('ad_draft_read');
    const saved=await invoke('ad_draft_save',{manifest:sixCampaigns(initial)});
    responses.draft_persistence={phase,initial,saved};
  }else if(phase==='read'||phase==='verify'){
    responses.draft_persistence={phase,value:await invoke('ad_draft_read')};
  }else if(phase==='pause'){
    const before=await invoke('ad_draft_read');
    const single=await invoke('pause_ad',{slot:'residential_broadband'});
    const single_read=await invoke('ad_draft_read');
    const all=await invoke('pause_ad',{slot:null});
    const all_read=await invoke('ad_draft_read');
    responses.draft_persistence={phase,before,single,single_read,all,all_read};
  }else throw Error('unknown draft persistence phase');
}
