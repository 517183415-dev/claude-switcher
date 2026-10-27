from pathlib import Path
import hashlib
import json
import os
import plistlib
import posixpath
import re
import struct
import subprocess
import sys
import time

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
STATE = json.loads((OUT / 'runtime.json').read_text())
EDITION = sys.argv[1]
assert EDITION in ['personal', 'public']
assert json.loads((OUT / ('bundle-' + EDITION + '.json')).read_text())['exit'] == 0
WORK = Path(STATE['workspace'])
PRODUCT = 'Claude 切换器 ' + EDITION.title()
SOURCE = ROOT / 'target/aarch64-apple-darwin/release/bundle/macos' / (PRODUCT + '.app')
APP = OUT / 'artifacts' / EDITION / (PRODUCT + '.app')
assert SOURCE.is_dir(), str(SOURCE)
assert not APP.exists(), 'Preserve existing artifact; do not overwrite a prior validation'
APP.parent.mkdir(parents=True, exist_ok=True)
COMMANDS = []

def copy_manifest(directory):
    result = {}
    for path in sorted(directory.rglob('*')):
        name = str(path.relative_to(directory))
        mode = oct(path.lstat().st_mode & 0o7777)
        if path.is_symlink():
            result[name] = {'symlink': os.readlink(path), 'mode': mode}
        elif path.is_file():
            result[name] = {'bytes': path.stat().st_size, 'sha256': sha(path), 'mode': mode}
        elif path.is_dir():
            result[name] = {'directory': True, 'mode': mode}
    return result

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def command(label, argv):
    started = time.monotonic()
    result = subprocess.run(['/usr/bin/sandbox-exec', '-f', STATE['profile']] + argv, env=STATE['environment'], cwd=WORK, capture_output=True)
    prefix = OUT / (EDITION + '-' + label)
    prefix.with_suffix('.stdout').write_bytes(result.stdout)
    prefix.with_suffix('.stderr').write_bytes(result.stderr)
    COMMANDS.append({'label': label, 'argv': argv, 'exit': result.returncode, 'seconds': time.monotonic() - started, 'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(), 'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()})
    assert result.returncode == 0, label + ': ' + result.stderr.decode(errors='replace')[-1500:]
    return result.stdout.decode(errors='replace') + result.stderr.decode(errors='replace')

def embedded_plist(path):
    data = path.read_bytes()
    assert struct.unpack_from('<I', data)[0] == 0xFEEDFACF, 'Expected thin Mach-O 64'
    commands = struct.unpack_from('<I', data, 16)[0]
    cursor = 32
    for _ in range(commands):
        cmd, size = struct.unpack_from('<II', data, cursor)
        assert size >= 8 and cursor + size <= len(data)
        if cmd == 0x19:
            nsects = struct.unpack_from('<I', data, cursor + 64)[0]
            for index in range(nsects):
                section = cursor + 72 + 80 * index
                sectname = data[section:section + 16].rstrip(b'\0')
                segname = data[section + 16:section + 32].rstrip(b'\0')
                if sectname == b'__info_plist' and segname == b'__TEXT':
                    length = struct.unpack_from('<Q', data, section + 40)[0]
                    offset = struct.unpack_from('<I', data, section + 48)[0]
                    assert offset + length <= len(data)
                    return plistlib.loads(data[offset:offset + length].rstrip(b'\0'))
        cursor += size
    raise AssertionError('Missing embedded __info_plist: ' + str(path))

def allowed_path(value):
    normalized = posixpath.normpath(value)
    assert normalized.startswith('/'), value
    system = normalized.startswith(('/System/Library/', '/usr/lib/'))
    local = Path(normalized).resolve().is_relative_to(APP.resolve())
    assert system or local, 'External runtime dependency/search path: ' + value
    return normalized, system

def expand_path(value, binary):
    for token in ['@loader_path', '@executable_path']:
        if value == token or value.startswith(token + '/'):
            value = str(binary.parent) + value[len(token):]
            break
    assert '@' not in value, 'Unresolved loader token: ' + value
    return allowed_path(value)

def dependencies(label, binary, loads):
    rpaths = re.findall(r'cmd LC_RPATH\s+cmdsize \d+\s+path (.*?) \(offset \d+\)', loads)
    expanded = [expand_path(value, binary)[0] for value in rpaths]
    linked = command('dependencies-' + label, ['/usr/bin/otool', '-L', str(binary)])
    resolved = []
    for line in linked.splitlines()[1:]:
        dependency = line.strip().split(' (compatibility version', 1)[0]
        if not dependency:
            continue
        if dependency.startswith('@rpath/'):
            candidates = [allowed_path(base + '/' + dependency[len('@rpath/'):]) for base in expanded]
            valid = [(value, system) for value, system in candidates if system or Path(value).is_file()]
            assert valid, 'Unresolved dependency: ' + dependency
            value, system = valid[0]
        else:
            value, system = expand_path(dependency, binary)
        assert system or Path(value).is_file(), 'Missing bundled dependency: ' + value
        resolved.append({'install_name': dependency, 'resolved': value, 'system': system})
    return {'rpaths': rpaths, 'resolved_rpaths': expanded, 'dependencies': resolved}

source_manifest = copy_manifest(SOURCE)
command('copy', ['/usr/bin/ditto', str(SOURCE), str(APP)])
assert copy_manifest(APP) == source_manifest, 'Copied App differs in file bytes, modes, paths or symlinks'
(OUT / ('unsigned-copy-' + EDITION + '.json')).write_text(json.dumps(source_manifest, ensure_ascii=False, indent=2) + '\n')
info_path = APP / 'Contents/Info.plist'
info = plistlib.loads(info_path.read_bytes())
identifier = 'local.claudeswitcher.' + EDITION
assert info['CFBundleIdentifier'] == identifier
assert info['CFBundleShortVersionString'] == '0.2.0'
assert info['LSMinimumSystemVersion'] == '13.0'
executable_name = info['CFBundleExecutable']
assert Path(executable_name).name == executable_name
main = APP / 'Contents/MacOS' / executable_name
assert main.is_file() and os.access(main, os.X_OK)
assert sha(main) == sha(ROOT / 'target/aarch64-apple-darwin/release/switcher-desktop')
unsigned_main_hash = sha(main)
icon = info.get('CFBundleIconFile')
assert icon and (APP / 'Contents/Resources' / icon).is_file()
components = {}
for binary, role in [('claude-switcher', 'cli'), ('switcher-helper', 'helper'), ('switcher-guardian', 'guardian')]:
    path = APP / 'Contents/Resources/components' / binary
    prepared = WORK / 'apps/desktop/src-tauri/components' / binary
    assert path.is_file() and os.access(path, os.X_OK)
    assert sha(path) == sha(prepared), binary
    if role != 'cli':
        embedded = embedded_plist(path)
        assert embedded['CFBundleIdentifier'] == 'local.claudeswitcher.' + role
        assert embedded['CFBundleShortVersionString'] == '0.2.0'
    components[binary] = {'path': str(path), 'role': role, 'sha256': sha(path), 'bytes': path.stat().st_size}

# The bundler was explicitly run with --no-sign. Sign only this copied App,
# using an ad-hoc identity and no timestamp/Developer ID/Keychain credentials.
command('sign-app', ['/usr/bin/codesign', '--force', '--options', 'runtime', '--sign', '-', '--timestamp=none', '--identifier', identifier, str(APP)])
command('verify-app', ['/usr/bin/codesign', '--verify', '--strict', '--deep', str(APP)])
display = command('display-app', ['/usr/bin/codesign', '--display', '--verbose=4', str(APP)])
assert 'Identifier=' + identifier in display and 'Signature=adhoc' in display
assert 'runtime' in display and 'TeamIdentifier=not set' in display
native_checks = {}
for label, path in [('main', main)] + [(name, Path(item['path'])) for name, item in components.items()]:
    arch = command('arch-' + label, ['/usr/bin/lipo', '-archs', str(path)]).strip()
    assert arch == 'arm64', (label, arch)
    loads = command('loads-' + label, ['/usr/bin/otool', '-l', str(path)])
    minimum = re.search(r'\bminos\s+(\d+(?:\.\d+)+)', loads)
    if not minimum:
        minimum = re.search(r'LC_VERSION_MIN_MACOSX\s+cmdsize\s+\d+\s+version\s+(\d+(?:\.\d+)+)', loads)
    assert minimum and tuple(int(x) for x in minimum.group(1).split('.'))[:2] == (13, 0), label
    command('verify-' + label, ['/usr/bin/codesign', '--verify', '--strict', str(path)])
    signature = command('signature-' + label, ['/usr/bin/codesign', '--display', '--verbose=4', str(path)])
    role_identifier = identifier if label == 'main' else 'local.claudeswitcher.' + components[label]['role']
    assert 'Identifier=' + role_identifier in signature and 'Signature=adhoc' in signature
    assert 'runtime' in signature
    native_checks[label] = {'architecture': arch, 'minimum_macos': minimum.group(1), 'identifier': role_identifier, 'signature': 'adhoc', 'runtime': True, **dependencies(label, path, loads)}
for name, item in components.items():
    assert sha(Path(item['path'])) == item['sha256'], 'Outer signing changed component bytes: ' + name

files = {}
expected_native = {main.resolve()} | {Path(item['path']).resolve() for item in components.values()}
for path in sorted(APP.rglob('*')):
    relative = str(path.relative_to(APP))
    if path.is_symlink():
        assert path.resolve().is_relative_to(APP.resolve()), relative
        files[relative] = {'symlink': os.readlink(path)}
    elif path.is_file():
        with path.open('rb') as stream:
            magic = stream.read(4)
        if magic in [b'\xcf\xfa\xed\xfe', b'\xfe\xed\xfa\xcf', b'\xca\xfe\xba\xbe', b'\xbe\xba\xfe\xca', b'\xca\xfe\xba\xbf', b'\xbf\xba\xfe\xca']:
            assert path.resolve() in expected_native, 'Additional native payload requires validation: ' + relative
        files[relative] = {'sha256': sha(path), 'bytes': path.stat().st_size, 'mode': oct(path.stat().st_mode & 0o777)}
tree_hash = hashlib.sha256(json.dumps(files, sort_keys=True, ensure_ascii=False).encode()).hexdigest()
record = {'status': 'PASS_LOCAL_APP_BUNDLE_STATIC', 'edition': EDITION, 'commit': STATE['commit'], 'app': str(APP), 'bundle_identifier': identifier, 'version': info['CFBundleShortVersionString'], 'minimum_macos': info['LSMinimumSystemVersion'], 'architecture': 'arm64', 'signature': 'adhoc_without_timestamp', 'unsigned_main_sha256': unsigned_main_hash, 'signed_main_sha256': sha(main), 'components': components, 'native_checks': native_checks, 'files': files, 'tree_manifest_sha256': tree_hash, 'commands': COMMANDS, 'full_app_bundle': True, 'static_check_command_count': sum(c['label'] not in ['copy', 'sign-app'] for c in COMMANDS), 'mutation_command_count': sum(c['label'] in ['copy', 'sign-app'] for c in COMMANDS), 'mutation_labels': ['copy', 'sign-app'], 'all_entries_after_signing': copy_manifest(APP), 'copy_bytes_modes_verified': True, 'product_launch': 'NOT_RUN', 'native_gui_ipc': 'NOT_RUN', 'installation_services': 'NOT_RUN', 'real_accounts': 'NOT_ACCESSED', 'Developer_ID_notarization_Gatekeeper': 'NOT_RUN'}
(OUT / ('artifact-' + EDITION + '.json')).write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n')
print(json.dumps({k:record[k] for k in ['status', 'edition', 'app', 'bundle_identifier', 'architecture', 'signature', 'tree_manifest_sha256']}, ensure_ascii=False))
