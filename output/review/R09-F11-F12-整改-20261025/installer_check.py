from pathlib import Path
import hashlib,json,os,subprocess,tempfile,time
root=Path(__file__).resolve().parents[3];out=Path(__file__).resolve().parent
baseline=json.loads((out/'baseline-hashes.json').read_text())
owned={'crates/switcher-helper/src/platform.rs','crates/switcher-helper/src/main.rs','crates/switcher-helper/src/policy.rs'}
def hashes(): return {p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in baseline}
before=hashes()
for p in baseline:
    if p not in owned: assert before[p]==baseline[p],p
tool=Path('/Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin')
for name in ['cargo','rustc','rustdoc']:assert (tool/name).is_file() and (tool/name).resolve().name!='rustup'
with tempfile.TemporaryDirectory(prefix='claude-r09-installer-') as d:
    temp=Path(d);(temp/'home').mkdir();(temp/'tmp').mkdir();(temp/'guard').mkdir()
    (temp/'guard/sitecustomize.py').write_text("""import os,sys
blocked={'launchctl','sudo','open','curl','scutil','systemsetup','networksetup','codesign','lipo','dig','nslookup','ps','kill','pkill','systemctl','rustup'}
def audit(event,args):
    if event=='subprocess.Popen':
        executable=os.fsdecode(args[0])
        if os.path.basename(executable) in blocked or os.path.basename(os.path.realpath(executable))=='rustup':raise RuntimeError('R09 blocked native command: '+os.path.basename(executable))
    if event in ('socket.connect','socket.bind','socket.getaddrinfo'):raise RuntimeError('R09 blocked network')
sys.addaudithook(audit)
""")
    env=dict(os.environ,HOME=str(temp/'home'),TMPDIR=str(temp/'tmp'),PYTHONPATH=str(temp/'guard'),PYTHONDONTWRITEBYTECODE='1',PATH=str(tool)+':/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin',CARGO_HOME='/Users/gua/.cargo',RUSTUP_HOME='/Users/gua/.rustup',RUSTUP_TOOLCHAIN='1.96.0-aarch64-apple-darwin',CARGO_NET_OFFLINE='true',RUSTC=str(tool/'rustc'),RUSTDOC=str(tool/'rustdoc'))
    argv=['/opt/homebrew/bin/python3','-B','-m','unittest','discover','-s','packaging/macos','-p','test_local_install.py','-v']
    t=time.monotonic();r=subprocess.run(argv,cwd=root,env=env,text=True,capture_output=True)
    after=hashes();(out/'installer-final.stdout').write_text(r.stdout);(out/'installer-final.stderr').write_text(r.stderr)
    receipt=dict(argv=argv,exit=r.returncode,seconds=time.monotonic()-t,source_before=before,source_after=after,source_frozen=before==after,environment={k:env[k] for k in ['HOME','TMPDIR','PYTHONPATH','PATH','CARGO_HOME','RUSTUP_HOME','RUSTUP_TOOLCHAIN','CARGO_NET_OFFLINE','RUSTC','RUSTDOC']},guard='Python native/socket/rustup audit hook; not an OS-wide sandbox')
    (out/'installer-final.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps({k:receipt[k] for k in ['exit','seconds','source_frozen']},ensure_ascii=False));print(r.stderr[-2500:]);assert before==after
