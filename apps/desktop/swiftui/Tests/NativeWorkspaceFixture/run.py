#!/usr/bin/env python3
"""Build the native host into an isolated, synthetic-vault Tauri fixture."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import pwd
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[5]
TOOL = Path('/Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin')
COMMON = {'workspace': True, 'mounted': True, 'bridge_page': True, 'swift_gate': True, 'screenshot': True}


def isolation_profile():
    """Deny real credentials, system changes, and all network in the fixture."""
    home = Path(pwd.getpwuid(os.getuid()).pw_dir)
    private = [home / name for name in ('Library', '.claude', '.claude-switcher', '.ssh',
               '.aws', '.config', '.local/bin')]
    private += [Path('/Library/Keychains'), Path('/Library/Application Support/claude-switcher'),
                Path('/Library/PrivilegedHelperTools')]
    private_files = [home / name for name in ('.claude.json', '.cargo/credentials', '.cargo/credentials.toml')]
    protected = [Path(path) for path in ('/Applications', '/Library/LaunchDaemons', '/Library/LaunchAgents',
                 '/usr/local/bin', '/opt/homebrew/bin')]
    protected += [ROOT / path for path in ('.git', 'output/review', 'output/packaging-validation-20261026',
                  'docs/acceptance/evidence', 'docs/review-pack', 'docs/review-pack-gemini', 'docs/review-pack-jimmy')]
    commands = ['/usr/sbin/sysctl', '/usr/sbin/systemsetup', '/usr/sbin/networksetup', '/usr/sbin/scutil',
                '/sbin/route', '/sbin/ifconfig', '/usr/bin/defaults', '/usr/bin/pgrep', '/bin/ps',
                '/bin/launchctl', '/usr/bin/security', '/usr/bin/osascript', '/usr/bin/open',
                '/usr/bin/curl', '/usr/bin/sudo', '/usr/bin/killall']
    quote = lambda path: json.dumps(str(path), ensure_ascii=False)
    lines = ['(version 1)', '(allow default)', '(deny network*)']
    lines += [f'(deny file-read* file-write* (subpath {quote(path)}))' for path in private]
    lines += [f'(deny file-read* file-write* (literal {quote(path)}))' for path in private_files]
    lines += [f'(deny file-write* (subpath {quote(path)}))' for path in protected]
    lines.append('(deny mach-lookup (regex ".*securityd.*") (regex ".*[Cc]laude.*"))')
    lines += [f'(deny process-exec (literal {quote(path)}))' for path in commands]
    return '\n'.join(lines) + '\n'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--edition', choices=('personal', 'public'), required=True)
    parser.add_argument('--large-backup', action='store_true', help='exercise the 64 MiB native IPC boundary')
    args = parser.parse_args()
    if args.large_backup and args.edition != 'personal':
        parser.error('--large-backup requires the personal edition')
    output = args.output.absolute()
    if any(path.is_symlink() for path in (output, *output.parents)):
        raise ValueError('output path must not contain symlinks')
    output.relative_to(ROOT / 'output')
    output.mkdir(parents=True, mode=0o700, exist_ok=False)
    for name in ('home', 'tmp', 'tmp/business-root', 'dist', 'cache'):
        (output / name).mkdir(mode=0o700)
    profile = output / 'isolation.sb'
    policy = isolation_profile()
    if '(deny network*)' not in policy or 'securityd' not in policy:
        raise ValueError('isolation profile lacks required restrictions')
    profile.write_text(policy)
    shutil.copyfile(ROOT / 'apps/desktop/public/native-bridge.html', output / 'dist/native-bridge.html')
    config = json.loads((ROOT / f'packaging/editions/{args.edition}.json').read_text())
    config['build'] = {'frontendDist': str(output / 'dist')}
    (output / 'config.json').write_text(json.dumps(config, ensure_ascii=False, indent=2))
    env = {'HOME': str(output / 'home'), 'TMPDIR': str(output / 'tmp') + '/',
           'CLANG_MODULE_CACHE_PATH': str(output / 'cache'), 'CARGO_HOME': '/Users/gua/.cargo',
           'RUSTUP_HOME': '/Users/gua/.rustup', 'RUSTC': str(TOOL / 'rustc'), 'RUSTDOC': str(TOOL / 'rustdoc'),
           'PATH': str(TOOL) + ':/usr/bin:/bin:/usr/sbin:/sbin', 'LANG': 'en_US.UTF-8',
           'CARGO_NET_OFFLINE': 'true', 'TAURI_CONFIG': json.dumps(config), 'MACOSX_DEPLOYMENT_TARGET': '13.0',
           'IPC_BUSINESS_ROOT': str(output / 'tmp/business-root'),
           'IPC_ORDINARY_PUBLISHER_OPT_IN': 'synthetic-memory-vault-only',
           'NATIVE_UI_FIXTURE_IMAGE': str(output / 'workspace.png')}
    if args.large_backup:
        env['NATIVE_UI_FIXTURE_LARGE_BACKUP'] = '1'
    command = [str(TOOL / 'cargo'), 'build', '--release', '--offline', '--locked', '--target', 'aarch64-apple-darwin',
               '-p', 'switcher-desktop', '--bin', 'switcher-desktop', '--no-default-features',
               '--features', f'{args.edition},ipc-business,native-ui,custom-protocol']
    sandbox = ['/usr/bin/sandbox-exec', '-f', str(profile)]
    with (output / 'build.log').open('wb') as log:
        # Swift's macro plugin starts its own macOS sandbox, which cannot be
        # nested inside sandbox-exec. Compile offline with the isolated HOME;
        # enforce the stronger OS sandbox when the fixture itself is executed.
        result = subprocess.run(command, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
    if result.returncode:
        raise RuntimeError('fixture build failed; see build.log')
    source = ROOT / 'target/aarch64-apple-darwin/release/switcher-desktop'
    contents = output / 'Native Fixture.app/Contents'
    (contents / 'MacOS').mkdir(parents=True)
    product = contents / 'MacOS/switcher-desktop'
    shutil.copy2(source, product)
    (contents / 'Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier': 'local.claudeswitcher.fixture',
        'CFBundleExecutable': 'switcher-desktop', 'CFBundleName': 'Native Fixture', 'CFBundlePackageType': 'APPL'}))
    subprocess.run(['/usr/bin/codesign', '--force', '--sign', '-', '--timestamp=none', str(contents.parent)],
                   check=True, capture_output=True)
    with (output / 'runtime.log').open('wb') as log:
        result = subprocess.run(sandbox + [str(product), '--native-workspace'], cwd=output, env=env,
                                stdout=log, stderr=subprocess.STDOUT, timeout=150 if args.large_backup else 60)
    lines = [line.removeprefix('SWITCHER_NATIVE_RESULT ') for line in (output / 'runtime.log').read_text(errors='replace').splitlines()
             if line.startswith('SWITCHER_NATIVE_RESULT ')]
    expected = dict(COMMON, **({'publisher': True, 'binary_input': True} if args.edition == 'personal'
                               else {'publisher_hidden': True, 'backend_denied': True}))
    if args.large_backup:
        expected.update(backup_at_limit=True, backup_over_limit=True)
    if result.returncode != 0 or len(lines) != 1 or json.loads(lines[0]) != expected:
        raise RuntimeError('native workspace fixture failed; see runtime.log')
    inputs = sorted((ROOT / 'apps/desktop/swiftui/Sources/SwitcherUI').rglob('*.swift'))
    inputs += [ROOT / path for path in ('apps/desktop/src-tauri/src/native_host.rs', 'apps/desktop/src-tauri/build.rs',
               'apps/desktop/src-tauri/src/ipc_business/ordinary.rs', 'apps/desktop/src-tauri/src/publisher_commands.rs',
               'apps/desktop/src-tauri/src/publisher_recovery_commands.rs', 'apps/desktop/public/native-bridge.html',
               'apps/desktop/swiftui/Tests/NativeWorkspaceFixture/run.py')]
    evidence = {'status': 'PASS', 'scope': f'isolated {args.edition} window; no network or real account; synthetic vault only',
                'checks': expected, 'binary_sha256': sha(product),
                'source_sha256': {str(path.relative_to(ROOT)): sha(path) for path in inputs}}
    (output / 'result.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(evidence, ensure_ascii=False))


if __name__ == '__main__':
    main()
