from pathlib import Path
import hashlib,json,os,shutil,subprocess,sys,tempfile,time
root=Path(__file__).resolve().parents[3];out=Path(__file__).resolve().parent
label=sys.argv[1]
cache=json.loads((root/'output/review/R07-F09-整改-20261025/offline-runtime.json').read_text())
modules=Path(cache['app'])/'node_modules'
assert (modules/'typescript').exists()
paths=['apps/desktop/src/pages/Operations.svelte','apps/desktop/tests/publisher-limits.test.mjs']
hashes={p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in paths}
with tempfile.TemporaryDirectory(prefix='claude-a01-ui-') as d:
 app=Path(d)/'app';app.mkdir();(app/'node_modules').symlink_to(modules,target_is_directory=True)
 for p in paths:
  target=app/Path(p).relative_to('apps/desktop');target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(root/p,target)
 env=dict(os.environ,HOME=d,TMPDIR=d,npm_config_offline='true',npm_config_update_notifier='false')
 argv=['/opt/homebrew/bin/node','--test',str(app/'tests/publisher-limits.test.mjs')]
 t=time.monotonic();r=subprocess.run(argv,cwd=app,env=env,capture_output=True,text=True)
 (out/(label+'.stdout')).write_text(r.stdout);(out/(label+'.stderr')).write_text(r.stderr)
 receipt=dict(argv=argv,exit=r.returncode,seconds=time.monotonic()-t,source_hashes=hashes,cache=str(modules),real_ipc='NOT_RUN')
 (out/(label+'.json')).write_text(json.dumps(receipt,indent=2)+'\n')
 print(json.dumps(receipt));print(r.stdout);print(r.stderr)
