from pathlib import Path
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import time

root = Path(__file__).resolve().parents[3]
out = Path(__file__).resolve().parent
tool = Path('/Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin')
kind = sys.argv[1]
source_paths = [
    'apps/desktop/src-tauri/src/publisher/mod.rs',
    'apps/desktop/src-tauri/src/publisher/local_store.rs',
    'apps/desktop/src-tauri/src/publisher/cleanup.rs',
    'apps/desktop/src-tauri/src/publisher/cleanup_tests.rs',
    'apps/desktop/src-tauri/src/publisher/tests.rs',
    'apps/desktop/src-tauri/src/publisher/backup.rs',
    'apps/desktop/src-tauri/src/publisher/protocol.rs',
    'apps/desktop/src-tauri/src/publisher_commands.rs',
    'apps/desktop/src-tauri/src/main.rs',
    'apps/desktop/src-tauri/build.rs',
    'apps/desktop/src/pages/Operations.svelte',
    'crates/switcher-domain/src/desktop.rs',
    'crates/switcher-domain/src/lib.rs',
    'packaging/editions/personal.json',
    'apps/desktop/src/generated/PublisherCleanupPreview.ts',
    'apps/desktop/src/generated/PublisherGcPreview.ts',
    'apps/desktop/tests/publisher-limits.test.mjs',
    'apps/desktop/tests/publisher-resolution.test.mjs',
    'apps/desktop/tests/publisher-maintenance.test.mjs',
]

def hashes():
    return {p: hashlib.sha256((root / p).read_bytes()).hexdigest() for p in source_paths}

before = hashes()
frontend = json.loads((root / 'output/review/A03-F17-整改-20261025/frontend-final-runtime.json').read_text())
dist = frontend.get('build_personal_dist')
if not dist:
    dist = str(Path(frontend['base']) / 'build-personal-dist')
assert Path(dist).is_dir(), dist

with tempfile.TemporaryDirectory(prefix='claude-final-local-') as temporary:
    env = dict(os.environ)
    for key in ['BASH_ENV', 'ENV', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER']:
        env.pop(key, None)
    env.update(
        HOME=temporary, TMPDIR=temporary,
        PATH=str(tool) + ':/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin',
        CARGO_HOME='/Users/gua/.cargo', RUSTUP_HOME='/Users/gua/.rustup',
        RUSTUP_TOOLCHAIN='1.96.0-aarch64-apple-darwin', CARGO_NET_OFFLINE='true',
        RUSTC=str(tool / 'rustc'), RUSTDOC=str(tool / 'rustdoc'),
        TAURI_CONFIG=json.dumps({'bundle': {'resources': []}, 'build': {'frontendDist': dist}}),
    )
    cargo = [str(tool / 'cargo')]
    desktop = ['--offline', '--locked', '-p', 'switcher-desktop', '--bin', 'switcher-desktop']
    if kind == 'a01-compat':
        argv = cargo + ['test'] + desktop + ['publisher::tests::f15_']
    elif kind == 'a02-compat':
        argv = cargo + ['test'] + desktop + ['publisher::tests::', '--', '--skip', 'f15_']
    elif kind == 'a02-feedback':
        argv = cargo + ['test'] + desktop + ['publisher_commands::f16_feedback_tests::']
    elif kind == 'personal-clippy':
        argv = cargo + ['clippy'] + desktop + ['--', '-D', 'warnings']
    elif kind == 'public-check':
        argv = cargo + ['check'] + desktop + ['--no-default-features', '--features', 'public']
    elif kind == 'fmt-check':
        argv = [str(tool / 'rustfmt'), '--edition', '2024', '--config', 'skip_children=true', '--check']
        argv += [str(root / p) for p in source_paths if p.endswith('.rs')]
    elif kind == 'cli-build':
        argv = cargo + ['build', '--offline', '--locked', '-p', 'switcher-core', '--bin', 'claude-switcher']
    elif kind in ['cli-help-personal', 'cli-help-public']:
        edition = kind.removeprefix('cli-help-')
        argv = [str(root / 'target/debug/claude-switcher'), '--edition', edition, '--help']
    else:
        raise ValueError(kind)
    entries_before = sorted(str(p.relative_to(temporary)) for p in Path(temporary).rglob('*'))
    started = time.monotonic()
    result = subprocess.run(argv, cwd=root, env=env, text=True, capture_output=True)
    after = hashes()
    receipt = {
        'argv': argv, 'exit': result.returncode, 'seconds': time.monotonic() - started,
        'source_before': before, 'source_after': after, 'source_frozen': before == after,
        'environment': {k: env[k] for k in ['HOME', 'TMPDIR', 'PATH', 'CARGO_HOME', 'RUSTUP_HOME', 'RUSTUP_TOOLCHAIN', 'CARGO_NET_OFFLINE', 'RUSTC', 'RUSTDOC', 'TAURI_CONFIG']},
        'native_ipc': 'NOT_RUN', 'installation': 'NOT_RUN', 'real_accounts': 'NOT_RUN',
    }
    if kind.startswith('cli-') and (root / 'target/debug/claude-switcher').exists():
        binary = root / 'target/debug/claude-switcher'
        receipt['artifact'] = {'path': str(binary), 'sha256': hashlib.sha256(binary.read_bytes()).hexdigest()}
    if kind.startswith('cli-help-'):
        entries_after = sorted(str(p.relative_to(temporary)) for p in Path(temporary).rglob('*'))
        receipt['isolated_home_before'] = entries_before
        receipt['isolated_home_after'] = entries_after
        assert entries_before == entries_after, 'pure help changed isolated HOME'
    (out / (kind + '.stdout')).write_text(result.stdout)
    (out / (kind + '.stderr')).write_text(result.stderr)
    (out / (kind + '.json')).write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps({k: receipt[k] for k in ['exit', 'seconds', 'source_frozen']}), flush=True)
    print(result.stdout[-5500:], flush=True)
    print(result.stderr[-2500:], flush=True)
    assert before == after, 'source changed during verification'
    sys.exit(result.returncode)
