from pathlib import Path
import subprocess,json,hashlib,sys,time
ROOT=Path.cwd();BASE=Path(__file__).resolve().parent
calls=[['python3','-B',str(BASE/'verify_current.py'),'--attempt','current'],['python3','-B',str(BASE/'feature_clippy_current.py')],['python3','-B',str(ROOT/'tests/evidence_reconciliation/reconcile.py'),str(BASE/'current'),'--output',str(BASE/'current/strict-result.json')]]
records=[]
for i,argv in enumerate(calls):
 start=time.monotonic();result=subprocess.run(argv,cwd=ROOT)
 records.append({'argv':argv,'exit':result.returncode,'seconds':time.monotonic()-start})
 (BASE/'driver-current-receipt.json').write_text(json.dumps({'commands':records},indent=2)+'\n')
 if result.returncode:raise SystemExit(result.returncode)
 if i==0:
  s=json.loads((BASE/'current/summary.json').read_text())
  if not s.get('source_unchanged') or len(s['commands'])!=4 or not all(c['passed'] for c in s['commands']):raise SystemExit('raw validation failed; original evidence retained')
