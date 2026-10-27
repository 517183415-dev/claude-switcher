"""Explicit native acceptance; no real credentials, default Keychain settings unchanged."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
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
if '--features' in sys.argv and sys.argv[sys.argv.index('--features') + 1] == 'public':
    ENV['TAURI_CONFIG'] = json.dumps({'identifier': 'local.claudeswitcher.public'})
if name in ('native', 'default-skip', 'opt-in-guard'):
    profile = OUT / 'native.sb'
    policy = BASE.read_text().replace('(deny mach-lookup (regex ".*securityd.*") (regex ".*[Cc]laude.*"))', '(deny mach-lookup (regex ".*[Cc]laude.*"))')
    assert policy != BASE.read_text()
    profile.write_text(policy)
    executable = json.loads((OUT / 'executable.json').read_text())['executable']
    ENV['SWITCHER_NATIVE_KEYCHAIN_ROOT'] = str(OUT / ('synthetic-state' if name == 'native' else name + '-state'))
    if name == 'native':
        ENV['SWITCHER_NATIVE_KEYCHAIN_OPT_IN'] = 'synthetic-owned-items-only'
    arguments = [executable, '--exact', 'publisher::security::native_keychain_tests::process_tests::native_keychain_restore_survives_writer_exit', '--ignored', '--test-threads=1']
    if name == 'default-skip':
        arguments = [executable, 'publisher::security::native_keychain_tests::process_tests::', '--test-threads=1']
else:
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
if name == 'build' and result.returncode == 0:
    artifacts = []
    for line in (OUT / 'build.log').read_text().splitlines():
        try:
            message = json.loads(line)
        except ValueError:
            continue
        if message.get('reason') == 'compiler-artifact' and message.get('executable') and message.get('target', {}).get('name') == 'switcher-desktop' and message.get('profile', {}).get('test'):
            artifacts.append(message)
    assert len(artifacts) == 1
    executable = Path(artifacts[0]['executable'])
    (OUT / 'executable.json').write_text(json.dumps({'executable': str(executable), 'sha256': hashlib.sha256(executable.read_bytes()).hexdigest()}, indent=2) + '\n')
print(json.dumps(receipt))
if name != 'build':
    print('\n'.join((OUT / f'{name}.log').read_text(errors='replace').splitlines()[-18:]))
sys.exit(result.returncode)
