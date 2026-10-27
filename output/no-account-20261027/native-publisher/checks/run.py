"""Explicit native acceptance; no real credentials, default Keychain settings unchanged."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
TOOL = Path('/Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin')
BASE = ROOT / 'output/no-account-20261027/storage-directory-race/account-free.sb'
ENV = {
    'HOME': str(OUT / 'home'), 'TMPDIR': str(OUT / 'tmp'),
    'CARGO_HOME': '/Users/gua/.cargo', 'RUSTUP_HOME': '/Users/gua/.rustup',
    'RUSTC': str(TOOL / 'rustc'), 'RUSTDOC': str(TOOL / 'rustdoc'),
    'PATH': f'{TOOL}:/Applications/Xcode.app/Contents/Developer/usr/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin',
    'CARGO_NET_OFFLINE': 'true', 'LANG': 'en_US.UTF-8',
    'TAURI_CONFIG': json.dumps({'identifier': 'local.claudeswitcher.personal'}),
}
for directory in ('home', 'tmp'):
    (OUT / directory).mkdir(exist_ok=True, mode=0o700)
name = sys.argv[1]
if '--features' in sys.argv and sys.argv[sys.argv.index('--features') + 1] .split(',')[0] == 'public':
    ENV['TAURI_CONFIG'] = json.dumps({'identifier': 'local.claudeswitcher.public'})
profile = BASE
arguments = [str(TOOL / 'cargo'), *sys.argv[2:]]
command = ['/usr/bin/sandbox-exec', '-f', str(profile), *arguments]
started = time.monotonic()
with (OUT / f'{name}.log').open('wb') as log:
    result = subprocess.run(command, cwd=ROOT, env=ENV, stdout=log, stderr=subprocess.STDOUT)
receipt = {'command': command, 'exit': result.returncode,
           'duration_seconds': round(time.monotonic() - started, 3),
           'log_sha256': hashlib.sha256((OUT / f'{name}.log').read_bytes()).hexdigest(),
           'policy_sha256': hashlib.sha256(profile.read_bytes()).hexdigest()}
(OUT / f'{name}.json').write_text(json.dumps(receipt, indent=2) + '\n')
print('\n'.join((OUT / f'{name}.log').read_text(errors='replace').splitlines()[-16:]))
sys.exit(result.returncode)
