"""Offline checks in a disposable mirror; never starts the desktop product."""
from pathlib import Path
import hashlib,json,os,re,shutil,subprocess,tempfile,time

root=Path(__file__).resolve().parents[3];out=Path(__file__).resolve().parent
source=root/'apps/desktop'
cache=Path(json.loads((root/'output/review/R07-F09-整改-20261025/offline-runtime.json').read_text())['app'])/'node_modules'
paths=sorted([p for name in ('src','scripts','tests') for p in (source/name).rglob('*') if p.is_file()]+[p for pattern in ('package*.json','*config.*','index.html') for p in source.glob(pattern) if p.is_file()])
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def hashes():return {str(p.relative_to(root)):sha(p) for p in paths}
expected=hashes()
base=Path(tempfile.mkdtemp(prefix='claude-a03-frontend-final-'))
app=base/'workspace/apps/desktop';app.parent.mkdir(parents=True)
shutil.copytree(source,app,symlinks=True,ignore=shutil.ignore_patterns('node_modules','target','dist','.svelte-kit'))
subprocess.run(['/bin/cp','-cR',str(cache),str(app/'node_modules')],check=True)
evidence=root/'docs/acceptance/evidence/e06'
if evidence.exists():shutil.copytree(evidence,base/'workspace/docs/acceptance/evidence/e06',symlinks=True)
for path,digest in expected.items():assert sha(base/'workspace'/Path(path))==digest,path
for name in ('home','tmp','npm-cache'):(base/name).mkdir()
env=dict(os.environ)
for name in ('APP_SOURCE_PATH','REPAIR_BASELINE','REPAIR_REVISION2_BASELINE','NODE_OPTIONS','BASH_ENV','ENV'):
    env.pop(name,None)
env.update(HOME=str(base/'home'),TMPDIR=str(base/'tmp'),PATH='/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin',npm_config_cache=str(base/'npm-cache'),npm_config_offline='true',npm_config_update_notifier='false',npm_config_audit='false',npm_config_fund='false')
runtime=dict(base=str(base),app=str(app),cache=str(cache),no_install=True,source_hashes=expected,environment={k:env[k] for k in ('HOME','TMPDIR','PATH','npm_config_cache','npm_config_offline')},native_gui='NOT_RUN',native_ipc='NOT_RUN')
(out/'frontend-final-runtime.json').write_text(json.dumps(runtime,indent=2)+'\n')
checks=[('test',['/opt/homebrew/bin/npm','--offline','test'],{}),('check',['/opt/homebrew/bin/npm','--offline','run','check'],{}),('build-personal',['/opt/homebrew/bin/npm','--offline','run','build'],{'VITE_EDITION':'personal'}),('build-public',['/opt/homebrew/bin/npm','--offline','run','build'],{'VITE_EDITION':'public'})]
receipts=[]
for label,argv,extra in checks:
    before=hashes();assert before==expected
    started=time.monotonic();r=subprocess.run(argv,cwd=app,env=dict(env,**extra),capture_output=True,text=True,timeout=120)
    after=hashes()
    (out/('frontend-final-'+label+'.stdout')).write_text(r.stdout);(out/('frontend-final-'+label+'.stderr')).write_text(r.stderr)
    receipt=dict(label=label,argv=argv,cwd=str(app),exit=r.returncode,seconds=time.monotonic()-started,source_before=before,source_after=after,source_frozen=before==after==expected,environment=extra)
    if label=='test':
        match=re.search(r'tests (\d+)',r.stdout)
        if match:receipt['node_test_methods']=int(match.group(1))
    if label.startswith('build-') and r.returncode==0:
        target=base/(label+'-dist');shutil.copytree(app/'dist',target)
        receipt['artifact_directory']=str(target);receipt['artifacts']={str(p.relative_to(target)):sha(p) for p in target.rglob('*') if p.is_file()}
    receipts.append(receipt);(out/'frontend-final-receipts.json').write_text(json.dumps(receipts,indent=2)+'\n')
    print(json.dumps({k:receipt[k] for k in ('label','exit','seconds','source_frozen')}),flush=True)
    assert receipt['source_frozen']
    if r.returncode:
        print(r.stdout[-6000:]);print(r.stderr[-3000:]);raise SystemExit(r.returncode)
