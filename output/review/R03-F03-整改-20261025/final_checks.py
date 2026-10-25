import hashlib,json,re,shlex,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
BASE=Path(__file__).resolve().parent
FILES=['crates/switcher-core/src/accounts.rs','crates/switcher-core/src/accounts/legacy.rs','crates/switcher-core/src/migration.rs','crates/switcher-core/src/guardian_repair.rs','packaging/macos/local_install.py','packaging/macos/test_local_install.py']
def hashes():
    return {p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in FILES}
GROUPS={
 'python':[
  ('python-f03',['/opt/homebrew/bin/python3','-B','-m','unittest','discover','-s','packaging/macos','-p','test_local_install.py','-k','f03','-v']),
  ('python-module',['/opt/homebrew/bin/python3','-B','-m','unittest','discover','-s','packaging/macos','-p','test_local_install.py','-v'])],
 'rust':[
  ('rust-f03',['cargo','test','--offline','--locked','-p','switcher-core','--lib','f03_','--','--nocapture']),
  ('rust-guardian',['cargo','test','--offline','--locked','-p','switcher-core','--lib','guardian_repair::']),
  ('rust-regions',['cargo','test','--offline','--locked','-p','switcher-core','--lib','regions::']),
  ('clippy',['cargo','clippy','--offline','--locked','-p','switcher-core','--lib','--tests','--','-D','warnings']),
  ('rustfmt',['rustfmt','--check','--edition','2024','crates/switcher-core/src/guardian_repair.rs']),
  ('diff-check',['git','diff','--check'])]}
receipts=[]
for name,cmd in GROUPS[sys.argv[1]]:
    before=hashes(); start=time.monotonic(); log=BASE/('final-'+name+'.log')
    with log.open('wb') as out:
        proc=subprocess.run(cmd,cwd=ROOT,stdout=out,stderr=subprocess.STDOUT)
    body=log.read_text(errors='replace'); after=hashes()
    py=re.search(r'Ran (\d+) tests?',body); rust=re.search(r'test result: ok\. (\d+) passed;',body)
    count=int((py or rust).group(1)) if py or rust else None
    entry=dict(name=name,command=cmd,command_display=shlex.join(cmd),exit_code=proc.returncode,
               seconds=round(time.monotonic()-start,3),log=str(log.relative_to(ROOT)),
               matched_tests=count,source_hashes_before=before,source_hashes_after=after,
               unchanged=before==after)
    receipts.append(entry)
    (BASE/('final-'+sys.argv[1]+'-receipts.json')).write_text(json.dumps(receipts,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({k:entry[k] for k in ['name','exit_code','matched_tests','seconds','unchanged']}),flush=True)
    assert proc.returncode==0 and before==after
    if name in ['python-f03','python-module','rust-f03','rust-guardian','rust-regions']:
        assert count is not None and count>0
