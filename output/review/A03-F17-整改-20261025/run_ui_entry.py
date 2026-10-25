from pathlib import Path
import hashlib,json,os,shutil,subprocess,sys,tempfile,time
root=Path(__file__).resolve().parents[3];out=Path(__file__).resolve().parent
label=sys.argv[1]
modules=Path(json.loads((root/'output/review/R07-F09-整改-20261025/offline-runtime.json').read_text())['app'])/'node_modules'
paths=['apps/desktop/src/pages/Operations.svelte','apps/desktop/tests/publisher-maintenance.test.mjs','apps/desktop/src/generated/PublisherCleanupPreview.ts','apps/desktop/src/generated/PublisherGcPreview.ts']
def hashes():return {p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in paths}
before=hashes()
with tempfile.TemporaryDirectory(prefix='claude-a03-ui-') as temporary:
    app=Path(temporary)/'app';app.mkdir();(app/'node_modules').symlink_to(modules,target_is_directory=True)
    for p in paths:
        target=app/Path(p).relative_to('apps/desktop');target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(root/p,target)
    env=dict(os.environ,HOME=temporary,TMPDIR=temporary,npm_config_offline='true',npm_config_update_notifier='false')
    argv=['/opt/homebrew/bin/node','--test',str(app/'tests/publisher-maintenance.test.mjs')]
    started=time.monotonic();r=subprocess.run(argv,cwd=app,env=env,text=True,capture_output=True)
    after=hashes();receipt=dict(argv=argv,exit=r.returncode,seconds=time.monotonic()-started,source_before=before,source_after=after,source_frozen=before==after,cache=str(modules),native_ipc='NOT_RUN')
    (out/(label+'.stdout')).write_text(r.stdout);(out/(label+'.stderr')).write_text(r.stderr);(out/(label+'.json')).write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps({k:receipt[k] for k in ('argv','exit','seconds','source_frozen')}));print(r.stdout);print(r.stderr)
    assert before==after
