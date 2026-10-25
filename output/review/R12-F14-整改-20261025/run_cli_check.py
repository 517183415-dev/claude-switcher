import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
TOOLCHAIN = Path('/Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin')
BOUND = json.loads((ROOT / 'output/review/R11-F18-整改-20261025/verification.json').read_text())['source_sha256']


def hashes():
    return {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in BOUND}


def run(name, argv, env):
    before = hashes()
    start = time.monotonic()
    result = subprocess.run(argv, cwd=ROOT, env=env, capture_output=True, timeout=900)
    after = hashes()
    (OUT / f'{name}.stdout').write_bytes(result.stdout)
    (OUT / f'{name}.stderr').write_bytes(result.stderr)
    receipt = {
        'argv': argv, 'exit_code': result.returncode,
        'elapsed_seconds': round(time.monotonic() - start, 3),
        'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(),
        'stderr_sha256': hashlib.sha256(result.stderr).hexdigest(),
        'source_sha256_before': before, 'source_sha256_after': after,
        'accepted_R11_sources_match': before == after == BOUND,
        'scope': 'offline compile or pure CLI help only; no product check, status, repair, install, guardian, account or network command',
    }
    (OUT / f'{name}.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps({'name': name, 'exit_code': result.returncode, 'elapsed_seconds': receipt['elapsed_seconds'], 'source_match': receipt['accepted_R11_sources_match']}), flush=True)
    assert result.returncode == 0 and receipt['accepted_R11_sources_match']


env = os.environ.copy()
env.update({
    'PATH': f'{TOOLCHAIN}:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin',
    'CARGO_HOME': '/Users/gua/.cargo', 'RUSTUP_HOME': '/Users/gua/.rustup',
    'RUSTUP_TOOLCHAIN': '1.96.0-aarch64-apple-darwin', 'CARGO_NET_OFFLINE': 'true',
    'RUSTC': str(TOOLCHAIN / 'rustc'), 'RUSTDOC': str(TOOLCHAIN / 'rustdoc'),
})
run('cli-build', [str(TOOLCHAIN / 'cargo'), 'build', '--offline', '--locked', '-p', 'switcher-core', '--bin', 'claude-switcher'], env)
binary = ROOT / 'target/debug/claude-switcher'
assert binary.is_file()
with tempfile.TemporaryDirectory(prefix='claude-r12-help-') as directory:
    isolated = env | {'HOME': directory, 'TMPDIR': directory}
    before = sorted(Path(directory).rglob('*'))
    for edition in ['personal', 'public']:
        run(f'cli-help-{edition}', [str(binary), '--edition', edition, '--help'], isolated)
    after = sorted(Path(directory).rglob('*'))
    assert before == after, 'pure help wrote into isolated HOME'
    (OUT / 'cli-help-home.json').write_text(json.dumps({'HOME': directory, 'entries_before': len(before), 'entries_after': len(after), 'product_commands': ['--edition personal --help', '--edition public --help']}, indent=2) + '\n')
(OUT / 'cli-artifact.json').write_text(json.dumps({'path': str(binary), 'bytes': binary.stat().st_size, 'sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'profile': 'debug', 'packaged_app': False}, indent=2) + '\n')
