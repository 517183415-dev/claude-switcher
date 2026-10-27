from pathlib import Path
import hashlib,json,subprocess,time
ROOT=Path.cwd()
OUT=Path(__file__).resolve().parent/'current/ipc-business-clippy'
OUT.mkdir(exist_ok=False)
BASE=OUT.parent
baseline=json.loads((BASE/'tests-result.json').read_text())['source_before']
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def inputs():
 return {p:sha(ROOT/p) for p in baseline}
def write(p,x):p.write_text(json.dumps(x,ensure_ascii=False,indent=2)+'\n')
assert inputs()==baseline
results=[]
for edition in ['personal','public']:
 dest=OUT/edition;dest.mkdir()
 for name in ['home','tmp','module-cache']:(dest/name).mkdir(mode=0o700)
 env=json.loads((BASE/'environment.json').read_text())
 config_path=ROOT/f'packaging/editions/ipc-business-{edition}.json'
 config=json.loads(config_path.read_text())
 assert config['identifier']==f'local.switcher.ipcbusiness.{edition}' and config['build']['frontendDist']=='../tests/native-ipc-business' and config['bundle']['active'] is False
 env.update(HOME=str(dest/'home'),TMPDIR=str(dest/'tmp')+'/',CLANG_MODULE_CACHE_PATH=str(dest/'module-cache'),TAURI_CONFIG=json.dumps(config,ensure_ascii=False),APPLE_SIGNING_IDENTITY='-')
 argv=['/Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin/cargo','clippy','--offline','--locked','-p','switcher-desktop','--all-targets','--no-default-features','--features',edition+',ipc-business,custom-protocol','--','-D','warnings']
 wrapped=['/usr/bin/sandbox-exec','-f',str(BASE/'account-free.sb')]+argv
 before=inputs();start=time.monotonic();print('START ipc-business-clippy',edition,flush=True)
 log=dest/'clippy.log'
 with log.open('xb') as stream:run=subprocess.run(wrapped,cwd=ROOT,env=env,stdout=stream,stderr=subprocess.STDOUT)
 after=inputs()
 receipt={'edition':edition,'argv':argv,'wrapped_argv':wrapped,'environment':env,'config_sha256':sha(config_path),'exit':run.returncode,'seconds':time.monotonic()-start,'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'source_before':before,'source_after':after,'source_unchanged':before==after==baseline,'log_sha256':sha(log),'passed':run.returncode==0 and before==after==baseline}
 write(dest/'receipt.json',receipt);results.append({k:v for k,v in receipt.items() if k not in ['source_before','source_after','environment']});write(OUT/'summary.json',{'commands':results,'passed':len(results)==2 and all(r['passed'] for r in results)})
 print('DONE',edition,run.returncode,flush=True)
 if not receipt['passed']:
  print(log.read_text(errors='replace')[-8000:],flush=True)
  raise SystemExit(1)
