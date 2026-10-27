"""Bounded, isolated checks; each invocation records its own command and result."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
OUTPUT = Path(__file__).resolve().parent
TOOL = Path('/Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin')
ENV = os.environ | {
    'HOME': str(ROOT / 'output/no-account-20261027/ordinary-registry/home'),
    'TMPDIR': str(ROOT / 'output/no-account-20261027/ordinary-registry/tmp'),
    'CARGO_HOME': '/Users/gua/.cargo', 'RUSTUP_HOME': '/Users/gua/.rustup',
    'RUSTC': str(TOOL / 'rustc'), 'RUSTDOC': str(TOOL / 'rustdoc'),
    'PATH': f'{TOOL}:/Applications/Xcode.app/Contents/Developer/usr/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin',
    'CARGO_NET_OFFLINE': 'true',
}
name, edition, *arguments = sys.argv[1:]
ENV['TAURI_CONFIG'] = json.dumps({'identifier': f'local.claudeswitcher.{edition}'})
command = ['/usr/bin/sandbox-exec', '-f', str(ROOT / 'output/no-account-20261027/storage-directory-race/account-free.sb'), str(TOOL / 'cargo'), *arguments]
started = time.monotonic()
with (OUTPUT / f'{name}.log').open('wb') as log:
    result = subprocess.run(command, cwd=ROOT, env=ENV, stdout=log, stderr=subprocess.STDOUT)
receipt = {'command': command, 'edition': edition, 'exit': result.returncode,
           'duration_seconds': round(time.monotonic() - started, 3),
           'log_sha256': hashlib.sha256((OUTPUT / f'{name}.log').read_bytes()).hexdigest()}
(OUTPUT / f'{name}.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt))
print('\n'.join((OUTPUT / f'{name}.log').read_text(errors='replace').splitlines()[-14:]))
sys.exit(result.returncode)
