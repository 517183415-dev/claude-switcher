from pathlib import Path
import argparse, hashlib, importlib.util, json, re, subprocess, time, sys
sys.dont_write_bytecode = True
ROOT=Path('/Users/gua/CodexWorkspaces/claude-switcher')
parser=argparse.ArgumentParser()
parser.add_argument("--attempt",required=True,choices=["current"])
ARGS=parser.parse_args()
OUT=Path(__file__).resolve().parent / ARGS.attempt
OUT.mkdir(exist_ok=False)
BASE=ROOT/'output/no-account-20261027/rust'
TOOL='/Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin/'
spec=importlib.util.spec_from_file_location('input_binding', ROOT/'output/no-account-20261027/native-ipc/build/build_native_v2.py'); binding=importlib.util.module_from_spec(spec);spec.loader.exec_module(binding)
prior_inputs=binding.inputs
def complete_inputs(generated=False):
    result=prior_inputs(generated)
    if not generated:
        for directory in [ROOT/'apps/desktop/tests/native-ipc-business', ROOT/'apps/desktop/tests/preferences-faults', ROOT/'tests/evidence_reconciliation', ROOT/'apps/desktop/tests/native-publisher', ROOT/'apps/desktop/tests/native-remote-config', ROOT/'apps/desktop/tests/native-remote-ui']:
            for path in sorted(directory.rglob('*')):
                if path.is_file() and not path.is_symlink() and '__pycache__' not in path.parts:
                    result[str(path.relative_to(ROOT))]=hashlib.sha256(path.read_bytes()).hexdigest()
    return result
binding.inputs=complete_inputs
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def write(p,x):p.write_text(json.dumps(x,ensure_ascii=False,indent=2)+'\n')
def main():
 assert not (OUT/'summary.json').exists()
 env=json.loads((BASE/'environment.json').read_text())
 for name in ['home','tmp']:(OUT/name).mkdir(mode=0o700,exist_ok=False)
 env.update(HOME=str(OUT/'home'),TMPDIR=str(OUT/'tmp')+'/',CARGO_TARGET_DIR=str(ROOT/'target'),RUSTUP_TOOLCHAIN='1.96.0-aarch64-apple-darwin',PYTHONDONTWRITEBYTECODE='1')
 profile=OUT/'account-free.sb';assert not profile.exists();profile.write_bytes((BASE/'account-free.sb').read_bytes())
 write(OUT/'environment.json',env)
 before=binding.inputs();summary={'head_at_start':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'environment':env,'source_before':before,'profile_sha256':sha(profile),'loopback_policy':'Retained solely for controlled local HTTP/TLS transport fixtures; all public network remains denied. No product launch.','commands':[]}
 skips=['unregistered_or_wrong_signer_service_never_authenticates','time::tests::real_macos_continuous_sample_is_available']
 base=[TOOL+'cargo'];test=base+['test','--offline','--locked','--workspace','--tests','--']
 for item in skips:test+=['--skip',item]
 plans=[('tests',test+['--test-threads=1'],None,False),('list',test+['--list'],None,False),('fmt',base+['fmt','--all','--','--check'],None,False),('clippy',base+['clippy','--offline','--locked','--workspace','--all-targets','--','-D','warnings'],None,False)]
 for label,argv,edition,expected_failure in plans:
  command_env=dict(env)
  source_before=binding.inputs();generated_before=binding.inputs(generated=True);start=time.monotonic();wrapped=['/usr/bin/sandbox-exec','-f',str(profile)]+argv
  print('START',label,flush=True)
  log=OUT/(label+'.log')
  with log.open('xb') as stream:result=subprocess.run(wrapped,cwd=ROOT,env=command_env,stdout=stream,stderr=subprocess.STDOUT)
  text=log.read_text(errors='replace');after=binding.inputs()
  item={'label':label,'argv':argv,'wrapped_argv':wrapped,'environment':command_env,'exit':result.returncode,'seconds':time.monotonic()-start,'log_sha256':sha(log),'head_before':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'source_before':source_before,'source_after':after,'source_unchanged':source_before==after,'generated_before':generated_before,'generated_after':binding.inputs(generated=True),'expected_failure':expected_failure}
  if label=='tests':item.update(executions=re.findall(r'^test (.+) \.\.\. (ok|FAILED|ignored)$',text,re.M),harness_results=re.findall(r'^test result: .+$',text,re.M))
  if label=='list':item['listed_names']=re.findall(r'^(.+): test$',text,re.M)
  item['passed']=(result.returncode!=0 and 'Select exactly one IPC test fixture' in text) if expected_failure else result.returncode==0
  item['passed']=item['passed'] and item['source_unchanged']
  write(OUT/(label+'-result.json'),item);summary['commands'].append({'label':label,'exit':result.returncode,'passed':item['passed'],'seconds':item['seconds'],'log_sha256':item['log_sha256']})
  print('DONE',label,result.returncode,'PASS' if item['passed'] else 'FAIL',flush=True)
  if not item['passed']:print(text[-6000:],flush=True)
  write(OUT/'summary.json',summary)
  if not item['source_unchanged']:break
 if not (OUT/'list-result.json').exists():
  summary['passed']=False;summary['status']='STOPPED_INPUT_CHANGE';write(OUT/'summary.json',summary);return
 run=json.loads((OUT/'tests-result.json').read_text());listed=json.loads((OUT/'list-result.json').read_text());executions=run['executions']; names=[x[0] for x in executions if x[1]=='ok']
 summary.update(head_at_end=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),source_after=binding.inputs(),source_unchanged=before==binding.inputs(),actual_execution_count=len(executions),passed_execution_count=len(names),distinct_passed_names=len(set(names)),listed_count=len(listed['listed_names']),distinct_listed_names=len(set(listed['listed_names'])),run_list_multiset_equal=sorted(names)==sorted(listed['listed_names']),native_skips=skips)
 summary['passed']=all(x['passed'] for x in summary['commands']) and summary['source_unchanged'] and summary['run_list_multiset_equal']
 write(OUT/'summary.json',summary);print(json.dumps({k:v for k,v in summary.items() if k not in ['environment','source_before','source_after']},indent=2),flush=True)
if __name__=='__main__':main()
