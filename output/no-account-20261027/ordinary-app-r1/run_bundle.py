from pathlib import Path
import hashlib
import io
import json
import os
import re
import subprocess
import sys
import tarfile
import tempfile
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent
TOOLS = Path('/Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin')
if len(sys.argv) > 2:
    assert re.fullmatch(r'[0-9a-f]{40}', sys.argv[2]), 'Pass the exact 40-character product commit'
    COMMIT = subprocess.check_output(['git', 'rev-parse', '--verify', sys.argv[2] + '^{commit}'], cwd=ROOT).decode().strip()
    assert COMMIT == sys.argv[2], 'Archive must bind the exact supplied commit'
else:
    COMMIT = json.loads((OUT / 'runtime.json').read_text())['commit']

def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')

def bootstrap():
    assert not (OUT / 'runtime.json').exists(), 'Preserve earlier runtime evidence'
    assert len(sys.argv) > 2, 'preflight requires an explicit product commit'
    cache = Path(json.loads((ROOT / 'output/review/A03-F17-整改-20261025/frontend-final-runtime.json').read_text())['cache'])
    assert (cache / '.bin/tauri').exists(), 'Missing cached Tauri CLI'
    base = Path(tempfile.mkdtemp(prefix='claude-app-bundle-'))
    workspace = base / 'workspace'
    workspace.mkdir()
    tree = subprocess.check_output(['git', 'rev-parse', '--verify', COMMIT + '^{tree}'], cwd=ROOT, text=True).strip()
    assert re.fullmatch(r'[0-9a-f]{40}', tree), 'Product commit must resolve to an exact tree'
    archive = subprocess.check_output(['git', 'archive', COMMIT], cwd=ROOT)
    (OUT / 'source.tar').write_bytes(archive)
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        tar.extractall(workspace, filter='data')
    source_inputs = {str(path.relative_to(workspace)): {'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'bytes':path.stat().st_size,'mode':oct(path.stat().st_mode & 0o7777)} for path in sorted(workspace.rglob('*')) if path.is_file() and not path.is_symlink()}
    save(OUT / 'archived-source-inputs.json', source_inputs)
    from check_fixture_exclusion import source_contract
    save(OUT / 'archived-source-configuration.json', source_contract(workspace))
    (workspace / 'target').symlink_to(ROOT / 'target', target_is_directory=True)
    (workspace / 'apps/desktop/node_modules').symlink_to(cache, target_is_directory=True)
    for name in ['home', 'tmp', 'cargo-home', 'npm-cache', 'module-cache']:
        (base / name).mkdir()
    for name in ['registry', 'git']:
        original = Path('/Users/gua/.cargo') / name
        if original.exists():
            (base / 'cargo-home' / name).symlink_to(original, target_is_directory=True)
    protected = ['/Users/gua/Library', '/Users/gua/.claude', '/Users/gua/.claude.json', '/Users/gua/.claude-switcher', '/Users/gua/.ssh', '/Users/gua/.aws', '/Users/gua/.config', '/Users/gua/.cargo/credentials', '/Users/gua/.cargo/credentials.toml', '/Users/gua/.cargo/config', '/Users/gua/.cargo/config.toml', '/Library/Keychains', '/Library/PrivilegedHelperTools', '/Library/Application Support/claude-switcher', '/Applications/Claude.app']
    policy = '(version 1)\n(allow default)\n(deny network*)\n'
    policy += '(deny file-read* (subpath "/Users/gua"))\n'
    allowed_reads = [OUT, ROOT / 'target', TOOLS.parent, cache, Path('/Users/gua/.cargo/registry'), Path('/Users/gua/.cargo/git')]
    for path in allowed_reads:
        policy += '(allow file-read* (subpath ' + json.dumps(str(path)) + '))\n'
    ancestors = {parent for path in allowed_reads for parent in path.parents}
    policy += '(allow file-read-metadata ' + ' '.join('(literal ' + json.dumps(str(path)) + ')' for path in sorted(ancestors)) + ')\n'
    for path in protected:
        policy += '(deny file-read* file-write* (subpath ' + json.dumps(path) + '))\n'
    for path in ['/Applications', '/Library/LaunchDaemons', '/Library/LaunchAgents', '/Library/Application Support/claude-switcher', '/usr/local/bin', '/opt/homebrew/bin', str(ROOT / '.git'), str(ROOT / 'output/review')]:
        policy += '(deny file-write* (subpath ' + json.dumps(path) + '))\n'
    policy += '(deny mach-lookup (global-name-regex #"com.apple.securityd.*") (global-name-regex #".*[Cc]laude.*"))\n'
    for executable in ['/usr/bin/security', '/bin/launchctl', '/usr/bin/open', '/usr/bin/osascript', '/usr/sbin/systemsetup', '/usr/sbin/networksetup', '/sbin/route', '/sbin/ifconfig', '/usr/sbin/scutil', '/usr/bin/sudo']:
        policy += '(deny process-exec (literal ' + json.dumps(executable) + '))\n'
    for path in [str(cache), '/Users/gua/.cargo/registry', '/Users/gua/.cargo/git']:
        policy += '(deny file-write* (subpath ' + json.dumps(path) + '))\n'
    for executable in ['/usr/sbin/sysctl', '/usr/bin/defaults', '/usr/bin/pgrep', '/bin/ps', '/usr/bin/curl', '/usr/bin/killall']:
        policy += '(deny process-exec (literal ' + json.dumps(executable) + '))\n'
    profile = OUT / 'build.sb'
    profile.write_text(policy)
    env = {
        'HOME': str(base / 'home'), 'TMPDIR': str(base / 'tmp'),
        'PATH': str(TOOLS) + ':/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin',
        'LANG': 'en_US.UTF-8', 'LC_ALL': 'en_US.UTF-8', 'TERM': 'dumb',
        'USER': 'build-validation', 'LOGNAME': 'build-validation', 'SHELL': '/bin/bash',
        'CI': 'true', 'CARGO_HOME': str(base / 'cargo-home'),
        'RUSTUP_HOME': '/Users/gua/.rustup', 'RUSTUP_TOOLCHAIN': '1.96.0-aarch64-apple-darwin',
        'RUSTC': str(TOOLS / 'rustc'), 'RUSTDOC': str(TOOLS / 'rustdoc'),
        'CARGO_NET_OFFLINE': 'true', 'CARGO_TARGET_DIR': str(ROOT / 'target'),
        'MACOSX_DEPLOYMENT_TARGET': '13.0', 'CLANG_MODULE_CACHE_PATH': str(base / 'module-cache'),
        'npm_config_offline': 'true', 'npm_config_cache': str(base / 'npm-cache'),
        'npm_config_update_notifier': 'false', 'npm_config_audit': 'false', 'npm_config_fund': 'false',
        'COREPACK_ENABLE_NETWORK': '0', 'APPLE_SIGNING_IDENTITY': '-', 'VITE_IPC_SMOKE': '0',
    }
    state = {'repo_root': str(ROOT), 'target_dir': str(ROOT / 'target'), 'commit_tree': subprocess.check_output(['git', 'rev-parse', COMMIT + '^{tree}'], cwd=ROOT).decode().strip(), 'archive_sha256': hashlib.sha256(archive).hexdigest(), 'archived_source_inputs_sha256':hashlib.sha256((OUT/'archived-source-inputs.json').read_bytes()).hexdigest(), 'build_order_policy': 'sequential build; verify and copy each edition before next target mutation', 'commit': COMMIT, 'base': str(base), 'workspace': str(workspace), 'cache': str(cache), 'profile': str(profile), 'environment': env}
    save(OUT / 'runtime.json', state)
    return state

def run(state, label, argv, cwd=None, extra_env=None):
    env = state['environment'] | (extra_env or {})
    wrapped = ['/usr/bin/sandbox-exec', '-f', state['profile']] + argv
    start = time.monotonic()
    with (OUT / (label + '.stdout')).open('wb') as stdout, (OUT / (label + '.stderr')).open('wb') as stderr:
        result = subprocess.run(wrapped, cwd=cwd or state['workspace'], env=env, stdout=stdout, stderr=stderr)
    record = {'label': label, 'argv': argv, 'sandbox': state['profile'], 'exit': result.returncode, 'seconds': time.monotonic() - start, 'commit': COMMIT, 'HOME': env['HOME'], 'CARGO_HOME': env['CARGO_HOME'], 'network_policy': 'deny network*', 'installation': 'NOT_RUN', 'real_accounts': 'NOT_ACCESSED', 'product_launch': 'NOT_RUN'}
    for stream in ['stdout', 'stderr']:
        record[stream + '_sha256'] = hashlib.sha256((OUT / (label + '.' + stream)).read_bytes()).hexdigest()
    save(OUT / (label + '.json'), record)
    print(json.dumps(record, ensure_ascii=False), flush=True)
    if result.returncode:
        print((OUT / (label + '.stderr')).read_text(errors='replace')[-3500:], flush=True)
        sys.exit(result.returncode)

mode = sys.argv[1]
state = bootstrap() if mode == 'preflight' else json.loads((OUT / 'runtime.json').read_text())
app = Path(state['workspace']) / 'apps/desktop'
cli = str(Path(state['cache']) / '@tauri-apps/cli/tauri.js')
if mode == 'preflight':
    run(state, 'sandbox-smoke', ['/usr/bin/true'])
    run(state, 'tauri-build-help', ['/opt/homebrew/bin/node', cli, 'build', '--help'], cwd=app)
elif mode in ['personal', 'public']:
    assert not (OUT / ('bundle-' + mode + '.json')).exists(), 'Never overwrite an earlier build receipt'
    if mode == 'public':
        personal = json.loads((OUT / 'artifact-personal.json').read_text())
        assert personal['commit'] == COMMIT and personal['status'] == 'PASS_LOCAL_APP_BUNDLE_STATIC'
        assert (OUT / 'fixture-exclusion-personal.json').is_file(), 'Verify Personal exclusion before Public build'
    help_text = (OUT / 'tauri-build-help.stdout').read_text()
    assert '--no-sign' in help_text, 'Cached CLI lacks required no-sign option; stop before identity lookup'
    argv = ['/opt/homebrew/bin/node', cli, 'build', '--target', 'aarch64-apple-darwin', '--features', mode, '--config', '../../packaging/editions/' + mode + '.json', '--no-sign', '--', '--no-default-features', '--offline', '--locked']
    run(state, 'bundle-' + mode, argv, cwd=app, extra_env={'VITE_EDITION': mode})
else:
    raise ValueError(mode)
