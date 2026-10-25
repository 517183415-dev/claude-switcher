from pathlib import Path
import subprocess,json,hashlib,time,sys,ast
base=Path(__file__).resolve().parent
label=sys.argv[1]
command=['/opt/homebrew/bin/python3','-B','-m','unittest','discover','-s','packaging/macos','-p','test_local_install.py',*sys.argv[2:],'-v']
files=['packaging/macos/local_install.py','packaging/macos/test_local_install.py']
def hashes(): return {f:hashlib.sha256(Path(f).read_bytes()).hexdigest() for f in files}
before=hashes(); t=time.monotonic(); result=subprocess.run(command,capture_output=True)
(base/(label+'.stdout')).write_bytes(result.stdout); (base/(label+'.stderr')).write_bytes(result.stderr)
receipt=dict(command=command,exit=result.returncode,seconds=time.monotonic()-t,before=before,after=hashes())
(base/(label+'.json')).write_text(json.dumps(receipt,indent=2))
print(json.dumps(receipt)); print(result.stdout.decode()[-1000:]);print(result.stderr.decode()[-4000:])
sys.exit(result.returncode)
