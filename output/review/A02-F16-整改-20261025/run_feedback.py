from pathlib import Path
import hashlib,json,os,shutil,subprocess,sys,tempfile,time

root=Path(__file__).resolve().parents[3]
out=Path(__file__).resolve().parent
label=sys.argv[1]
kind=sys.argv[2]
paths=['apps/desktop/src-tauri/src/publisher_commands.rs','apps/desktop/src/pages/Operations.svelte','apps/desktop/tests/publisher-resolution.test.mjs','apps/desktop/src-tauri/src/publisher/mod.rs','apps/desktop/src-tauri/src/publisher/tests.rs']
def hashes(): return {p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in paths}
before=hashes()
tool=Path('/Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin')
with tempfile.TemporaryDirectory(prefix='claude-a02-feedback-') as temporary:
    env=dict(os.environ,HOME=temporary,TMPDIR=temporary,PATH=str(tool)+':/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin',CARGO_HOME='/Users/gua/.cargo',RUSTUP_HOME='/Users/gua/.rustup',RUSTUP_TOOLCHAIN='1.96.0-aarch64-apple-darwin',CARGO_NET_OFFLINE='true',RUSTC=str(tool/'rustc'),RUSTDOC=str(tool/'rustdoc'),npm_config_offline='true',npm_config_update_notifier='false',TAURI_CONFIG=json.dumps({'bundle':{'resources':[]},'build':{'frontendDist':'/var/folders/gq/3mlm75990wg6j1lgrb4f80kc0000gn/T/claude-r07-final-v9bqk37d/workspace/apps/desktop/dist'}}))
    cwd=root
    if kind=='rust':
        argv=[str(tool/'cargo'),'test','--offline','--locked','-p','switcher-desktop','--bin','switcher-desktop','publisher_commands::f16_feedback_tests::']
    elif kind=='fmt':
        argv=[str(tool/'rustfmt'),'--edition','2024',str(root/paths[0])]
    elif kind=='fmtcheck':
        argv=[str(tool/'rustfmt'),'--edition','2024','--check',str(root/paths[0])]
    elif kind=='clippy':
        argv=[str(tool/'cargo'),'clippy','--offline','--locked','-p','switcher-desktop','--bin','switcher-desktop','--','-D','warnings']
    elif kind=='ui':
        modules=Path(json.loads((root/'output/review/R07-F09-整改-20261025/offline-runtime.json').read_text())['app'])/'node_modules'
        assert (modules/'typescript').exists()
        cwd=Path(temporary)/'app';cwd.mkdir();(cwd/'node_modules').symlink_to(modules,target_is_directory=True)
        for p in paths[1:3]:
            target=cwd/Path(p).relative_to('apps/desktop');target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(root/p,target)
        argv=['/opt/homebrew/bin/node','--test',str(cwd/'tests/publisher-resolution.test.mjs')]
    else:
        raise ValueError(kind)
    started=time.monotonic();r=subprocess.run(argv,cwd=cwd,env=env,text=True,capture_output=True)
    after=hashes()
    receipt=dict(argv=argv,exit=r.returncode,seconds=time.monotonic()-started,source_before=before,source_after=after,source_frozen=before==after,environment={k:env[k] for k in ('HOME','TMPDIR','PATH','CARGO_HOME','RUSTUP_HOME','RUSTUP_TOOLCHAIN','CARGO_NET_OFFLINE','RUSTC','RUSTDOC','TAURI_CONFIG')},native_ipc='NOT_RUN')
    (out/(label+'.stdout')).write_text(r.stdout);(out/(label+'.stderr')).write_text(r.stderr)
    (out/(label+'.json')).write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps({k:receipt[k] for k in ('argv','exit','seconds','source_frozen')},ensure_ascii=False));print(r.stdout[-5000:]);print(r.stderr[-2000:])
    assert kind=='fmt' or before==after
