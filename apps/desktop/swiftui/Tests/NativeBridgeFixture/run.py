#!/usr/bin/env python3
"""Exercise the Swift bridge inside the existing isolated Personal Tauri fixture."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess

ROOT = Path(__file__).resolve().parents[5]
HERE = Path(__file__).resolve().parent
BRIDGE = ROOT / 'apps/desktop/swiftui/Sources/SwitcherUI/Bridge'
EXPECTED = {'runtime_info': True, 'draft_read': True, 'public_gate': True, 'backend_error': True,
            'unknown_command_denied': True}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True,
                        help='previous isolated Personal ordinary-publisher fixture directory')
    args = parser.parse_args()
    output = args.output.resolve()
    output.relative_to(ROOT / 'output')
    fixture = args.fixture.resolve()
    fixture.relative_to(ROOT / 'output')
    product = fixture / 'Publisher IPC.app/Contents/MacOS/switcher-desktop'
    if 'ORDINARY_PUBLISHER_FIXTURE synthetic-memory-vault-only' not in (fixture / 'runtime.log').read_text(errors='replace'):
        raise RuntimeError('the input is not a recorded isolated publisher fixture')
    output.mkdir(parents=True, mode=0o700, exist_ok=False)
    for name in ('home', 'tmp', 'tmp/business-root', 'cache'):
        (output / name).mkdir(mode=0o700)

    recorded = json.loads((fixture / 'process.json').read_text())['binary_sha256']
    if digest(product) != recorded:
        raise RuntimeError('isolated fixture executable differs from its recorded hash')
    profile = output / 'isolation.sb'
    profile.write_text((fixture / 'isolation.sb').read_text() + '\n(deny network*)\n')
    env = {'HOME': str(output / 'home'), 'TMPDIR': str(output / 'tmp') + '/',
           'CLANG_MODULE_CACHE_PATH': str(output / 'cache'), 'PATH': '/usr/bin:/bin:/usr/sbin:/sbin',
           'LANG': 'en_US.UTF-8', 'IPC_BUSINESS_ROOT': str(output / 'tmp/business-root'),
           'IPC_ORDINARY_PUBLISHER_OPT_IN': 'synthetic-memory-vault-only'}
    init = output / 'initialize.o'
    library = output / 'bridge-fixture.dylib'
    common = ['/usr/bin/sandbox-exec', '-f', str(profile)]
    commands = [
        ['/usr/bin/xcrun', 'clang', '-fobjc-arc', '-fblocks', '-c', str(HERE / 'initialize.m'), '-o', str(init)],
        ['/usr/bin/xcrun', 'swiftc', '-parse-as-library', '-emit-library', '-module-name', 'NativeBridgeFixture',
         *[str(BRIDGE / name) for name in ('DesktopCommand.swift', 'JSONValue.swift', 'DesktopBridge.swift', 'TauriWebViewTransport.swift')],
         str(HERE / 'Inject.swift'), str(init), '-framework', 'AppKit', '-framework', 'WebKit', '-o', str(library)],
        ['/usr/bin/codesign', '--force', '--sign', '-', '--timestamp=none', str(library)],
    ]
    with (output / 'build.log').open('wb') as log:
        for command in commands:
            subprocess.run(common + command, env=env, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, check=True)
    command = common + ['/usr/bin/env', 'DYLD_INSERT_LIBRARIES=' + str(library), str(product), '--ordinary-publisher']
    with (output / 'runtime.log').open('wb') as log:
        process = subprocess.Popen(command, env=env, cwd=output, stdout=log,
                                   stderr=subprocess.STDOUT, start_new_session=True)
        try:
            process.wait(timeout=35)
        finally:
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            if process.poll() is None:
                process.wait(timeout=5)
    results = [line.removeprefix('SWIFTUI_BRIDGE_RESULT ') for line in (output / 'runtime.log').read_text(errors='replace').splitlines()
               if line.startswith('SWIFTUI_BRIDGE_RESULT ')]
    if process.returncode != 0 or len(results) != 1 or json.loads(results[0]) != EXPECTED:
        raise RuntimeError('native bridge fixture failed; see runtime.log')
    evidence = {'status': 'PASS', 'fixture_binary_sha256': recorded,
                'fixture_library_sha256': digest(library), 'checks': EXPECTED,
                'scope': 'isolated Personal Tauri fixture, synthetic data, no network'}
    (output / 'result.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(evidence, ensure_ascii=False))


if __name__ == '__main__':
    main()
