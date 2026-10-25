from pathlib import Path
import hashlib,json,os,subprocess,sys,tempfile,time
root=Path(__file__).resolve().parents[3]
out=Path(__file__).resolve().parent
label=sys.argv[1]; args=sys.argv[2:]
tool=Path('/Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin')
for name in ('cargo','rustc','rustdoc'):
    assert (tool/name).is_file() and (tool/name).resolve().name != 'rustup'
paths=list(json.loads((out/'baseline-hashes.json').read_text()))
def hashes(): return {p:(hashlib.sha256((root/p).read_bytes()).hexdigest() if (root/p).is_file() else None) for p in paths}
before=hashes()
with tempfile.TemporaryDirectory(prefix='claude-r10-'+label+'-') as d:
    env=dict(os.environ,HOME=d,TMPDIR=d,PATH=str(tool)+':/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin',CARGO_HOME='/Users/gua/.cargo',RUSTUP_HOME='/Users/gua/.rustup',RUSTUP_TOOLCHAIN='1.96.0-aarch64-apple-darwin',CARGO_NET_OFFLINE='true',RUSTC=str(tool/'rustc'),RUSTDOC=str(tool/'rustdoc'))
    argv=[str(tool/'cargo')]+args
    t=time.monotonic();r=subprocess.run(argv,cwd=root,env=env,text=True,capture_output=True)
    after=hashes()
    (out/(label+'.stdout')).write_text(r.stdout);(out/(label+'.stderr')).write_text(r.stderr)
    receipt=dict(argv=argv,exit=r.returncode,seconds=time.monotonic()-t,source_before=before,source_after=after,source_frozen=before==after,environment={k:env[k] for k in ['HOME','PATH','CARGO_HOME','RUSTUP_HOME','RUSTUP_TOOLCHAIN','CARGO_NET_OFFLINE','RUSTC','RUSTDOC']})
    (out/(label+'.json')).write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps({k:receipt[k] for k in ['argv','exit','seconds','source_frozen']},ensure_ascii=False));print(r.stdout[-7000:]);print(r.stderr[-2500:])
    assert before==after,'sources changed during verification'
