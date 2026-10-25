from pathlib import Path
p=Path('apps/desktop/src/components/RegionSettings.svelte');s=p.read_text()
s=s.replace("let {fixture=false,environment=false,onoutcome}:{fixture?:boolean;environment?:boolean;onoutcome:(outcome:OperationOutcome)=>void}=$props();", "let {fixture=false,environment=false,parentBusy=false,onoutcome,onrepairbusy,onrepairsuccess}:{fixture?:boolean;environment?:boolean;parentBusy?:boolean;onoutcome:(outcome:OperationOutcome)=>void;onrepairbusy?:(active:boolean,owner:string)=>boolean|void;onrepairsuccess?:(outcome:OperationOutcome)=>Promise<void>}=$props();")
a=s.index('async function load()');b=s.index('const itemNames:',a)
s=s[:a]+'''let requestId:string|null=null;
let epoch=0;
let disposed=false;
let busyOwner:string|null=null;
let executionOwner:string|null=null;
const currentOperation=(current:number)=>current===epoch&&!disposed;
async function load(current=epoch){
  if(fixture||disposed)return;
  const result=await invoke<RegionView>('region_view');
  if(currentOperation(current))view=result;
}
async function reload(){
  if(busy||parentBusy||disposed)return;
  const current=epoch;
  try{await load(current);}catch(e){if(currentOperation(current))error=errorMessage(e);}
}
function begin(owner:string,label:string){
  if(busy||parentBusy||disposed)return false;
  if(onrepairbusy?.(true,owner)===false)return false;
  busyOwner=owner;busy=label;return true;
}
function finish(owner:string){
  if(busyOwner!==owner)return;
  busyOwner=null;busy='';onrepairbusy?.(false,owner);
}
async function cancelPlan(){
  const id=requestId;requestId=null;epoch++;preview=null;
  // A dispatched mutation retains the parent guard until its own promise settles.
  if(!executionOwner&&busyOwner)finish(busyOwner);
  if(id&&!fixture){try{await invoke('cancel_repair',{requestId:id});}catch{/* Local confirmation remains invalid even if disconnected. */}}
}
async function plan(){await previewPlan(false);}
async function fix(){await previewPlan(true);}
async function previewPlan(autoExecute:boolean){
  if(busy||parentBusy||disposed)return;
  void cancelPlan();
  const current=++epoch,id=crypto.randomUUID();
  if(!fixture&&!begin(id,'正在读取系统原值并预览地区修改…'))return;
  requestId=id;preview=null;error='';recoveryMessage='';
  if(fixture){preview=previewRegionRepair(selected);return;}
  try{
    const result=await invoke<RepairPreview>(environment?'preview_repair':'preview_region_change',environment?{requestId:id}:{region:selected,requestId:id});
    if(!currentOperation(current)){await invoke('cancel_repair',{requestId:id});return;}
    preview=result;
    if(autoExecute&&environment&&result.can_execute&&!result.recovery_required){
      requestId=null;preview=null;await apply(id,current);
      if(!currentOperation(current))return;
    }
  }catch(e){
    if(currentOperation(current)){preview=null;requestId=null;error=errorMessage(e);}
    try{await invoke('cancel_repair',{requestId:id});}catch{/* Retry requires a new preview. */}
  }finally{finish(id);}
}
async function apply(id:string,current:number){
  executionOwner=id;busy='正在核验并执行已确认的地区计划…';error='';
  try{
    const result=await invoke<OperationOutcome>('execute_repair',{requestId:id});
    if(!currentOperation(current))return;
    onoutcome(result);
    if(result.completed){
      finish(id);
      await onrepairsuccess?.(result);
      if(!currentOperation(current))return;
    }
    await load(current);
    if(!currentOperation(current))return;
  }catch(e){if(currentOperation(current))error=errorMessage(e);}
  finally{if(executionOwner===id)executionOwner=null;finish(id);}
}
async function execute(){
  if(busy||parentBusy||disposed||fixture||!requestId||!preview?.can_execute||preview.recovery_required)return;
  const id=requestId;
  if(!begin(id,'正在核验并执行已确认的地区计划…'))return;
  requestId=null;preview=null;const current=++epoch;
  await apply(id,current);
}
async function recover(){
  if(busy||parentBusy||disposed||!preview?.recovery_required)return;
  error='';recoveryMessage='';
  if(fixture){recoveryMessage='合成预览未查询系统结果，也未处理修复事务。真实处理后请核对地区目标、系统时区与 IPv6，重新检查环境。';return;}
  const current=++epoch,id=crypto.randomUUID();
  if(!begin(id,'正在查询系统结果并恢复修复事务…'))return;
  executionOwner=id;
  let recovered=false;
  try{
    const result=await invoke<OperationOutcome>('recover_repair');
    if(!currentOperation(current))return;
    onoutcome(result);
    if(result.completed){
      recoveryMessage='修复事务已处理，请核对地区目标、系统时区与 IPv6，重新检查环境。';
      preview=null;await load(current);
      if(!currentOperation(current))return;
      recovered=true;
    }
  }catch(e){if(currentOperation(current))error=errorMessage(e);}
  finally{if(executionOwner===id)executionOwner=null;finish(id);}
  if(recovered&&currentOperation(current))await plan();
}
''' + s[b:]
s=s.replace("onMount(()=>{void load().catch(e=>error=errorMessage(e));});", "onMount(()=>{void reload();});")
s=s.replace('disabled={!!busy}', 'disabled={!!busy||parentBusy}')
s=s.replace('disabled={!!busy||fixture', 'disabled={!!busy||parentBusy||fixture')
s=s.replace("onclick={plan}>{environment?'预览修复计划':'预览地区修改'}", "onclick={environment?fix:plan}>{environment?'修复环境':'预览地区修改'}")
s=s.replace("onclick={()=>load().catch(e=>error=errorMessage(e))}","onclick={reload}")
p.write_text(s)
p=Path('apps/desktop/src/App.svelte');s=p.read_text()
s=s.replace("let busy = $state('');", "let busy = $state('');\n  let repairBusy = $state(false);\n  let repairOwner:string|null=null;")
needle="  function announce(result: OperationOutcome) { outcome=result; error=''; }"
s=s.replace(needle,needle+'''
  function repairBusyChanged(active:boolean,owner:string){
    if(active){
      if(busy||repairBusy)return false;
      repairOwner=owner;repairBusy=true;checkEpoch++;clearReport();outcome=null;error='';
    }else if(repairOwner===owner){repairOwner=null;repairBusy=false;}
    return true;
  }
  async function repairSucceeded(result:OperationOutcome){
    if(!result.completed)return;
    announce(result);await runChecks();
  }''')
s=s.replace('if (busy) return;','if (busy||repairBusy) return;')
s=s.replace('if(busy)return;', 'if(busy||repairBusy)return;')
s=s.replace('if(busy||!snapshot)return;', 'if(busy||repairBusy||!snapshot)return;')
s=s.replace('if(!snapshot||busy)return;', 'if(!snapshot||busy||repairBusy)return;')
s=s.replace('if(!fixture&&!busy&&!loading)', 'if(!fixture&&!busy&&!repairBusy&&!loading)')
s=s.replace('onoutcome={announce}/>', 'onoutcome={announce}/>',1) # Only the two RegionSettings tags below gain repair props.
s=s.replace('<RegionSettings {fixture} environment onoutcome={announce}/>', '<RegionSettings {fixture} environment parentBusy={loading||!!busy||repairBusy} onrepairbusy={repairBusyChanged} onrepairsuccess={repairSucceeded} onoutcome={announce}/>')
s=s.replace('<RegionSettings {fixture} onoutcome={announce}/>', '<RegionSettings {fixture} parentBusy={loading||!!busy||repairBusy} onrepairbusy={repairBusyChanged} onrepairsuccess={repairSucceeded} onoutcome={announce}/>')
s=s.replace('disabled={!!busy', 'disabled={repairBusy||!!busy')
s=s.replace('disabled={loading||!!busy', 'disabled={loading||repairBusy||!!busy')
s=s.replace('aria-busy={loading||!!busy}', 'aria-busy={loading||repairBusy||!!busy}')
s=s.replace("'查看实际证据，修复会先显示计划。'", "'查看实际证据，一键修复环境后重新检查。'")
p.write_text(s)
