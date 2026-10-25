#!/usr/bin/env python3
"""Preview and administer a fixed-path, source-local installation. No network use.

Install automatically replaces existing components and exits applications.
An optional plan digest supports explicit previews. Verify is read-only and runs
as the console user. Root is required only for component/service mutations.
"""
import argparse
import contextlib
import ctypes
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import pwd
import re
import secrets
import signal
import shlex
import time
import shutil
import stat
import subprocess
import uuid

SYSTEM = Path('/Library/Application Support/claude-switcher')
HELPER = Path('/Library/PrivilegedHelperTools/local.claudeswitcher.helper')
GUARDIAN = SYSTEM / 'switcher-guardian'
DAEMON = Path('/Library/LaunchDaemons/local.claudeswitcher.helper.plist')
AGENT = Path('/Library/LaunchAgents/local.claudeswitcher.guardian.plist')
RECORD = SYSTEM / 'install.json'
JOURNAL = SYSTEM / 'source-local-transaction.json'
BARRIER = SYSTEM / 'component-installer-active'
CHALLENGE = SYSTEM / 'source-local-health-challenge.json'
RECEIPT = SYSTEM / 'source-local-health-receipt.json'
LAST_RESULT = SYSTEM / 'source-local-last-result.json'
TEMPLATES = Path(__file__).parent / 'launchd'
WATCHDOG_PLIST = Path('/Library/LaunchDaemons/local.tz-watchdog.plist')
BODYGUARD_PLIST = Path('/Library/LaunchDaemons/local.claude-bodyguard.plist')
ZONEINFO = Path('/usr/share/zoneinfo')


def run(*args, check=True, inherit_proxy=False):
    environment = {'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'LC_ALL': 'C'}
    if inherit_proxy:
        if not args or str(args[0]) != '/usr/bin/curl':
            raise ValueError('proxy environment is restricted to discovery curl')
        for name in ('http_proxy','https_proxy','all_proxy','no_proxy'):
            for key in (name, name.upper()):
                if key in os.environ:
                    environment[key] = os.environ[key]
    return subprocess.run(list(map(str, args)), check=check, capture_output=True,
                          text=True, timeout=30, env=environment)


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def tree(path):
    """Reject links, devices and multiply-linked files; bind paths, modes and bytes."""
    path = Path(path)
    nodes = [path] + (sorted(path.rglob('*')) if path.is_dir() else [])
    result = []
    for node in nodes:
        m = node.lstat()
        if stat.S_ISLNK(m.st_mode) or not (stat.S_ISREG(m.st_mode) or stat.S_ISDIR(m.st_mode)):
            raise ValueError(f'unsafe artifact: {node}')
        if m.st_mode & 0o7022:
            raise ValueError(f'unsafe artifact permissions: {node}')
        if stat.S_ISREG(m.st_mode) and m.st_nlink != 1:
            raise ValueError(f'hard-linked artifact: {node}')
        result.append([str(node.relative_to(path)), stat.S_IMODE(m.st_mode),
                       hashlib.sha256(node.read_bytes()).hexdigest() if node.is_file() else None])
    return digest(result)


def identity(path, identifier):
    run('/usr/bin/codesign', '--verify', '--deep', '--strict', '--test-requirement',
        '=identifier "' + identifier + '"', path)
    info = run('/usr/bin/codesign', '--display', '--verbose=4', path).stderr
    cdhash = signing_cdhash(info)
    ent = run('/usr/bin/codesign', '--display', '--entitlements', ':-', path).stdout
    if ent.strip() and plistlib.loads(ent.encode()).get('com.apple.security.cs.disable-library-validation'):
        raise ValueError('library validation must remain enabled')
    executable = Path(path)
    if executable.is_dir():
        meta = plistlib.loads((executable/'Contents/Info.plist').read_bytes())
        executable = executable/'Contents/MacOS'/meta['CFBundleExecutable']
    arch = run('/usr/bin/lipo', '-archs', executable).stdout.strip()
    if arch != platform.machine():
        raise ValueError('native single architecture required')
    return cdhash


def signing_cdhash(info):
    hashes = re.findall(r'^CDHash=([a-f0-9]{40})$', info, re.M)
    flags = re.findall(r'^CodeDirectory .* flags=0x([a-fA-F0-9]+)\(([^)]*)\)', info, re.M)
    if (len(hashes) != 1 or len(flags) != 1 or not int(flags[0][0], 16) & 0x10000
            or "runtime" not in flags[0][1].split(",")):
        raise ValueError("single CDHash and Hardened Runtime required")
    return hashes[0]


def app(edition):
    return Path('/Applications') / ('Claude 切换器 ' + edition.title() + '.app')


def console_owner():
    owner = os.stat('/dev/console').st_uid
    if owner == 0:
        raise ValueError('an active non-root console user is required')
    return owner


def plan(edition, source, owner, operation='install'):
    if owner <= 0:
        raise ValueError('non-root owner required')
    payload = {}
    if operation != 'uninstall':
        source = Path(source).absolute()
        for key, path, identifier in [
            ('app', source, 'local.claudeswitcher.' + edition),
            ('helper', source/'Contents/Resources/components/switcher-helper', 'local.claudeswitcher.helper'),
            ('guardian', source/'Contents/Resources/components/switcher-guardian', 'local.claudeswitcher.guardian')]:
            payload[key] = dict(source=str(path), sha256=tree(path), cdhash=identity(path, identifier))
        cli = source/'Contents/Resources/components/claude-switcher'
        if not cli.is_file() or not cli.stat().st_mode & 0o111:
            raise ValueError('bundled Rust CLI missing or not executable')
        identity(cli, 'local.claudeswitcher.cli')
        meta = plistlib.loads((source/'Contents/Info.plist').read_bytes())
        version = meta['CFBundleShortVersionString']
        if not re.fullmatch(r'[0-9A-Za-z.+-]{1,48}', version):
            raise ValueError('invalid version')
    else:
        version = None
    entry = cli_snapshot(owner) if operation != 'uninstall' else None
    settings = installation_settings(owner, entry) if entry is not None else {}
    if entry is not None:
        entry['install_literals'] = settings
    result = dict(schema_version=1, operation=operation, edition=edition, owner_uid=owner,
                version=version, payload=payload,
                templates={p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in [TEMPLATES/DAEMON.name, TEMPLATES/AGENT.name]},
                destinations=list(map(str, [app(edition), HELPER, GUARDIAN, DAEMON, AGENT, RECORD])),
                previous=installed_state(edition),
                legacy=legacy_snapshot(owner, include_root=operation != 'uninstall'),
                timezone_target=settings.get('TZ_TARGET'),
                proxy_mode=settings.get('PROXY_MODE'),
                cli_entry=entry)
    result = edition_transition(result)
    return plan_uninstall(result) if operation == 'uninstall' else result


def edition_transition(p):
    if not RECORD.exists():
        return p
    record=private_json(RECORD,0o644)
    old=record['edition']
    if old not in ('personal','public'):
        raise ValueError('invalid previous edition')
    if record['owner_uid'] != p['owner_uid']:
        if p['operation'] == 'uninstall':
            raise ValueError('uninstall must name the current owner')
        if not isinstance(record['owner_uid'], int) or record['owner_uid'] <= 0:
            raise ValueError('invalid previous owner')
        p['previous_owner_uid'] = record['owner_uid']
        observed = installed_state(record['edition'])['app']
        if observed is None or observed['cdhash'] != record['trust']['gui_cdhash']:
            raise ValueError('previous owner App identity differs from installation record')
    if old == p['edition']:
        return p
    if p['operation'] == 'uninstall':
        raise ValueError('uninstall must name the installed edition')
    if present(app(p['edition'])):
        raise ValueError('destination edition already exists without matching record')
    observed=installed_state(old)['app']
    if observed is None or observed['cdhash'] != record['trust']['gui_cdhash']:
        raise ValueError('previous edition App identity differs from installation record')
    p['previous_edition']=old
    p['destinations'].append(str(app(old)))
    p['previous']['old_app']=observed
    return p


def plan_state(p):
    state=installed_state(p['edition'])
    if p.get('previous_edition'):
        state['old_app']=installed_state(p['previous_edition'])['app']
    return state


def uninstall_files(p):
    # The journal describes observations, never privileged destination paths.
    return dict(app=app(p['edition']), helper=HELPER, guardian=GUARDIAN,
                daemon=DAEMON, agent=AGENT, record=RECORD,
                cli=cli_path(p['owner_uid']), watchdog_script=SYSTEM/'tz-watchdog.sh',
                watchdog_plist=WATCHDOG_PLIST, bodyguard_script=SYSTEM/'claude-bodyguard.sh',
                bodyguard_plist=BODYGUARD_PLIST)


def uninstall_file(p, key, path=None):
    path = uninstall_files(p)[key] if path is None else path
    with as_owner(p['owner_uid']) if key == 'cli' else contextlib.nullcontext():
        try:
            metadata = path.lstat()
        except FileNotFoundError:
            return None
        mode = stat.S_IMODE(metadata.st_mode)
        if key == 'app':
            if not stat.S_ISDIR(metadata.st_mode):
                raise ValueError('unsafe uninstall App')
            content = tree(path)
        else:
            data, opened = small_file(path, limit=None if key in ('helper', 'guardian') else 1024*1024)
            if (metadata.st_dev, metadata.st_ino) != (opened.st_dev, opened.st_ino):
                raise ValueError('uninstall file changed while reading')
            content = hashlib.sha256(data).hexdigest()
        if mode & 0o7022:
            raise ValueError('unsafe uninstall permissions: '+key)
        return dict(sha256=content, uid=metadata.st_uid, gid=metadata.st_gid, mode=mode,
                    dev=metadata.st_dev, ino=metadata.st_ino)


def uninstall_service_specs(p):
    return [("system/local.claudeswitcher.helper", DAEMON, [str(HELPER)]),
            ('gui/'+str(p['owner_uid'])+'/local.claudeswitcher.guardian', AGENT, [str(GUARDIAN)]),
            ('system/local.tz-watchdog', WATCHDOG_PLIST, ['/bin/bash', str(SYSTEM/'tz-watchdog.sh')]),
            ('system/local.claude-bodyguard', BODYGUARD_PLIST, ['/bin/bash', str(SYSTEM/'claude-bodyguard.sh')])]


def uninstall_service(target, plist=None, arguments=None):
    observed = run('/bin/launchctl', 'print', target, check=False)
    if observed.returncode:
        if 'Could not find service' not in getattr(observed, 'stderr', ''):
            raise ValueError('cannot establish service absence: '+target)
        return False
    if plist is not None:
        # launchctl's loaded job is independent of the current plist on disk.
        def field(name):
            values = re.findall(r'^\s*'+name+r' = (.+)$', observed.stdout, re.M)
            return values[0].strip().strip('"') if len(values) == 1 else None
        blocks = re.findall(r'^\s*arguments = \{\n(.*?)^\s*\}', observed.stdout, re.M | re.S)
        loaded = []
        if len(blocks) == 1:
            for line in blocks[0].splitlines():
                line = re.sub(r'^\d+\s*=\s*', '', line.strip())
                if line:
                    loaded.append(line.strip('"'))
        if field('path') != str(plist) or field('program') != arguments[0] or loaded != arguments:
            raise ValueError('unproven loaded service: '+target)
    return True


def uninstall_snapshot(p):
    services = {}
    for target, plist, arguments in uninstall_service_specs(p):
        registered = uninstall_service(target, plist, arguments)
        domain, label = target.rsplit('/', 1)
        services[target] = dict(registered=registered, disabled=legacy_disabled(domain, label))
    return dict(files={key:uninstall_file(p, key) for key in uninstall_files(p)}, services=services)


def plan_uninstall(p):
    observed = uninstall_snapshot(p)
    if not present(RECORD):
        if (any(value is not None for value in observed['files'].values()) or
                any(value['registered'] for value in observed['services'].values()) or p['legacy']):
            raise ValueError('uninstall ownership is not recorded')
        p['uninstall_snapshot'] = observed
        return p
    record = private_json(RECORD, 0o644)
    trust = record.get('trust', {})
    generation = trust.get('generation', '')
    if trust.get('kind') != 'source_local' or not re.fullmatch('[a-f0-9]{32}', generation):
        raise ValueError('uninstall generation is not supported')
    stage = SYSTEM/('source-local-'+generation)
    safe_parents(stage)
    archived = private_json(stage/'result.json', 0o600)
    if (private_json(stage/'record', 0o644) != record or archived.get('stage') != str(stage) or
            archived.get('pending') != 'committed' or
            archived['plan']['owner_uid'] != p['owner_uid'] or archived['plan']['edition'] != p['edition']):
        raise ValueError('uninstall generation ownership differs')
    for key in component_keys(p):
        current = observed['files'][key]
        if current is not None:
            if current['uid'] != 0 or tree(uninstall_files(p)[key]) != tree(stage/key):
                raise ValueError('uninstall component ownership differs: '+key)
    entry = archived['plan'].get('cli_entry')
    if observed['files']['cli'] is not None:
        if entry is None or not archived.get('cli_changed'):
            raise ValueError('uninstall CLI ownership is not recorded')
        with as_owner(p['owner_uid']):
            data, _ = small_file(cli_path(p['owner_uid']))
        metadata = observed['files']['cli']
        if data != cli_shim(p['edition'], entry) or metadata['uid'] != p['owner_uid'] or metadata['mode'] != 0o755:
            raise ValueError('uninstall CLI ownership differs')
        account = pwd.getpwuid(p['owner_uid'])
        if metadata['gid'] not in os.getgrouplist(account.pw_name, account.pw_gid):
            raise ValueError('uninstall CLI group cannot be restored by its owner')
    p['cli_entry'] = cli_snapshot(p['owner_uid'])
    for kind, field in [('watchdog', 'timezone_target'), ('bodyguard', 'proxy_mode')]:
        state = archived.get(kind)
        target = archived['plan'].get(field)
        for part, mode in [('script', 0o755), ('plist', 0o644)]:
            key = kind+'_'+part
            current = observed['files'][key]
            if current is None:
                continue
            if state is None or not state.get('changed') or target is None:
                raise ValueError('uninstall root service ownership is not recorded: '+key)
            expected, _ = small_file(stage/(kind+'-new-'+part))
            if (current['uid'] != 0 or current['gid'] != 0 or current['mode'] != mode or
                    current['sha256'] != hashlib.sha256(expected).hexdigest()):
                raise ValueError('uninstall root service ownership differs: '+key)
        p[field] = target if state and state.get('changed') else None
        label = 'system/local.tz-watchdog' if kind == 'watchdog' else 'system/local.claude-bodyguard'
        if observed['services'][label]['registered'] and p[field] is None:
            raise ValueError('uninstall loaded service ownership is not recorded: '+kind)
    for (target, _, _), keys in zip(uninstall_service_specs(p),
            [('helper', 'daemon'), ('guardian', 'agent'),
             ('watchdog_script', 'watchdog_plist'), ('bodyguard_script', 'bodyguard_plist')]):
        if observed['services'][target]['registered'] and any(observed['files'][key] is None for key in keys):
            raise ValueError('uninstall registered service cannot be restored: '+target)
    p['uninstall_snapshot'] = observed
    return p


def recheck_uninstall(p):
    if 'uninstall_snapshot' in p and uninstall_snapshot(p) != p['uninstall_snapshot']:
        raise ValueError('uninstall installation changed since preview')


def uninstall_backup(j, key):
    if key == 'cli':
        return Path(j['stage'])/'legacy-cli.sh'
    if key.startswith(('watchdog_', 'bodyguard_')):
        kind, part = key.split('_', 1)
        return Path(j['stage'])/(kind+'-old-'+part)
    return Path(j['stage'])/('old-'+key)


def check_uninstall_backups(j, only=None):
    p = j['plan']
    snapshot = p.get('uninstall_snapshot')
    if snapshot is None:
        return
    for key, previous in snapshot['files'].items():
        if only is not None and key != only:
            continue
        if previous is not None:
            backup = uninstall_backup(j, key)
            if key == 'app':
                content = tree(backup)
                metadata = backup.lstat()
            else:
                data, metadata = small_file(backup, limit=None if key in ('helper', 'guardian') else 1024*1024)
                content = hashlib.sha256(data).hexdigest()
            mode = 0o600 if key == 'cli' else previous['mode']
            if (content != previous['sha256'] or metadata.st_uid != 0 or
                    stat.S_IMODE(metadata.st_mode) != mode):
                raise ValueError('uninstall backup differs: '+key)
    if only is None:
        for item in p['legacy']:
            data, metadata = small_file(Path(j['stage'])/('legacy-'+item['label']+'.plist'))
            if (hashlib.sha256(data).hexdigest() != item['sha256'] or
                    metadata.st_uid != 0 or stat.S_IMODE(metadata.st_mode) != 0o600):
                raise ValueError('uninstall legacy backup differs')


def uninstall_holding_path(j, key):
    target = uninstall_files(j['plan'])[key]
    generation = Path(j['stage']).name
    if not re.fullmatch(r'source-local-[a-f0-9]{32}', generation):
        raise ValueError('invalid uninstall holding generation')
    return target.parent/('.claude-switch-'+generation+'-'+key)


def check_uninstall_holdings(j, only=None):
    p = j['plan']
    for key, expected in j.get('uninstall_holding', {}).items():
        if only is not None and key != only:
            continue
        holding = uninstall_holding_path(j, key)
        with as_owner(p['owner_uid']) if key == 'cli' else contextlib.nullcontext():
            with directory_fd(holding) as directory:
                metadata = os.fstat(directory)
                observed = dict(path=str(holding), dev=metadata.st_dev, ino=metadata.st_ino,
                                uid=metadata.st_uid, gid=metadata.st_gid,
                                mode=stat.S_IMODE(metadata.st_mode))
                if observed != expected or metadata.st_uid != (p['owner_uid'] if key == 'cli' else 0) or observed['mode'] != 0o700:
                    raise ValueError('uninstall holding directory changed: '+key)
                held = uninstall_file(p, key, holding/'component')
                if held is not None and held != p['uninstall_snapshot']['files'][key]:
                    raise ValueError('uninstall moved component differs: '+key)
                if held is None and uninstall_file(p, key) is None:
                    raise ValueError('uninstall held component disappeared: '+key)
                if (holding.lstat().st_dev, holding.lstat().st_ino) != (metadata.st_dev, metadata.st_ino):
                    raise ValueError('uninstall holding directory changed: '+key)


def check_uninstall_recovery(j, only=None):
    p = j['plan']
    snapshot = p.get('uninstall_snapshot')
    if snapshot is None:
        return
    check_uninstall_backups(j, only)
    check_uninstall_holdings(j, only)
    for key, previous in snapshot['files'].items():
        if only is not None and key != only:
            continue
        current = uninstall_file(p, key)
        if current is None:
            if key in j.get('uninstall_restored', {}):
                raise ValueError('uninstall restored component disappeared: '+key)
            if previous is not None and key not in j.get('uninstall_removing', []):
                raise ValueError('uninstall component disappeared outside transaction: '+key)
        elif current != j.get('uninstall_restored', {}).get(key, previous):
            # Intent alone never proves that an externally created inode is ours.
            prepared = j.get('uninstall_prepared', {}).get(key)
            if key in j.get('uninstall_restored', {}) or prepared is None or current != prepared:
                raise ValueError('uninstall recovery destination changed: '+key)
    if only is not None:
        return current
    for target, plist, arguments in uninstall_service_specs(p):
        registered = uninstall_service(target, plist, arguments)
        domain, label = target.rsplit('/', 1)
        previous = snapshot['services'][target]
        if (registered and not previous['registered']) or legacy_disabled(domain, label) != previous['disabled']:
            raise ValueError('uninstall recovery service changed: '+target)


def remove_uninstall_file(j, key):
    p = j['plan']
    if uninstall_file(p, key) != p['uninstall_snapshot']['files'][key]:
        raise ValueError('uninstall destination changed before removal: '+key)
    if p['uninstall_snapshot']['files'][key] is None:
        return
    target = uninstall_files(p)[key]
    holding = uninstall_holding_path(j, key)
    with contextlib.ExitStack() as descriptors:
        with as_owner(p['owner_uid']) if key == 'cli' else contextlib.nullcontext():
            parent = descriptors.enter_context(directory_fd(target.parent))
            os.mkdir(holding.name, 0o700, dir_fd=parent)
            directory = descriptors.enter_context(directory_fd(holding))
            metadata = os.fstat(directory)
            os.fsync(parent)
        j.setdefault('uninstall_holding', {})[key] = dict(
            path=str(holding), dev=metadata.st_dev, ino=metadata.st_ino,
            uid=metadata.st_uid, gid=metadata.st_gid, mode=stat.S_IMODE(metadata.st_mode))
        j.setdefault('uninstall_removing', []).append(key)
        j['pending'] = 'remove-'+key; save(JOURNAL, j)
        with as_owner(p['owner_uid']) if key == 'cli' else contextlib.nullcontext():
            if uninstall_file(p, key) != p['uninstall_snapshot']['files'][key]:
                raise ValueError('uninstall destination changed before removal: '+key)
            rename_exclusive(parent, target.name, directory, 'component')
            os.fsync(parent); os.fsync(directory)
        # Keep the moved inode as a journal-addressed backup, including conflicts.
        check_uninstall_holdings(j, only=key)


def record_uninstall_prepared(j, key, candidate):
    previous = j['plan']['uninstall_snapshot']['files'][key]
    if candidate is None or any(candidate[field] != previous[field] for field in ('sha256', 'uid', 'gid', 'mode')):
        raise ValueError('uninstall prepared restoration differs: '+key)
    j.setdefault('uninstall_prepared', {})[key] = candidate
    save(JOURNAL, j)


def restore_uninstall_file(j, key):
    current = check_uninstall_recovery(j, only=key)
    p = j['plan']
    previous = p['uninstall_snapshot']['files'][key]
    if previous is None:
        if current is not None:
            raise ValueError('uninstall recovery found an unexpected file: '+key)
        return
    if current is None:
        if key not in j.setdefault('uninstall_restoring', []):
            j['uninstall_restoring'].append(key)
        save(JOURNAL, j)
        backup = uninstall_backup(j, key)
        if key == 'cli':
            data = small_file(backup)[0]
            def prepared(metadata):
                candidate = dict(sha256=hashlib.sha256(data).hexdigest(), uid=metadata.st_uid,
                                 gid=metadata.st_gid, mode=stat.S_IMODE(metadata.st_mode),
                                 dev=metadata.st_dev, ino=metadata.st_ino)
                record_uninstall_prepared(j, key, candidate)
            known_identity = write_owner_cli(p['owner_uid'], data, previous['mode'], exclusive=True,
                                            before_publish=prepared, gid=previous['gid'])
        else:
            # A failed copy stays private; it never leaves a partial live file.
            prepared = Path(j['stage'])/('restore-'+uuid.uuid4().hex)
            copy(backup, prepared)
            if key == 'app':
                content, metadata = tree(prepared), prepared.lstat()
            else:
                data, metadata = small_file(prepared, limit=None if key in ('helper', 'guardian') else 1024*1024)
                content = hashlib.sha256(data).hexdigest()
            if (content != previous['sha256'] or
                    stat.S_IMODE(metadata.st_mode) != previous['mode'] or metadata.st_uid != 0):
                raise ValueError('uninstall prepared restoration differs: '+key)
            known_identity = metadata.st_dev, metadata.st_ino
            with directory_fd(prepared.parent) as directory:
                fd = os.open(prepared.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=directory)
                try:
                    opened = os.fstat(fd)
                    if (opened.st_dev, opened.st_ino) != known_identity:
                        raise ValueError('uninstall prepared identity changed: '+key)
                    os.fchown(fd, previous['uid'], previous['gid']); os.fsync(fd)
                finally:
                    os.close(fd)
            record_uninstall_prepared(j, key, uninstall_file(p, key, prepared))
            target = uninstall_files(p)[key]
            with directory_fd(prepared.parent) as staged, directory_fd(target.parent) as parent:
                if uninstall_file(p, key) is not None:
                    raise ValueError('uninstall restoration destination appeared: '+key)
                rename_exclusive(staged, prepared.name, parent, target.name)
                os.fsync(parent); os.fsync(staged)
    else:
        known_identity = current['dev'], current['ino']
    path = uninstall_files(p)[key]
    with as_owner(p['owner_uid']) if key == 'cli' else contextlib.nullcontext():
        with directory_fd(path.parent) as parent:
            fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parent)
            try:
                metadata = os.fstat(fd)
                if ((metadata.st_dev, metadata.st_ino) != known_identity or
                        (key != 'app' and (not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1))):
                    raise ValueError('uninstall restoration identity changed: '+key)
                os.fchown(fd, previous['uid'], previous['gid']); os.fsync(fd)
            finally:
                os.close(fd)
    restored = uninstall_file(p, key)
    if (restored is None or (restored['dev'], restored['ino']) != known_identity or
            any(restored[k] != previous[k] for k in ('sha256', 'uid', 'gid', 'mode'))):
        raise ValueError('uninstall restored file differs: '+key)
    j.setdefault('uninstall_restored', {})[key] = restored
    if key in j.get('uninstall_restoring', []):
        j['uninstall_restoring'].remove(key)
    j.get('uninstall_prepared', {}).pop(key, None)
    save(JOURNAL, j)


def verify_uninstall_restored(j):
    p = j['plan']
    check_uninstall_recovery(j)
    for key, previous in p['uninstall_snapshot']['files'].items():
        if uninstall_file(p, key) != j.get('uninstall_restored', {}).get(key, previous):
            raise ValueError('uninstall restoration is incomplete: '+key)
    if uninstall_snapshot(p)['services'] != p['uninstall_snapshot']['services']:
        raise ValueError('uninstall restored services differ')
    if legacy_snapshot(p['owner_uid'], include_root=False) != p['legacy']:
        raise ValueError('uninstall restored legacy state differs')


def verify_uninstalled(p):
    for key, path in uninstall_files(p).items():
        with as_owner(p['owner_uid']) if key == 'cli' else contextlib.nullcontext():
            if present(path):
                raise ValueError('uninstall incomplete; component present: '+key)
    for target, _, _ in uninstall_service_specs(p):
        if uninstall_service(target):
            raise ValueError('uninstall incomplete; service registered: '+target)
    fixed = {spec['label']:spec for spec in legacy_specs(p['owner_uid'])}
    for item in p.get('legacy', []):
        if item['label'] != 'local.claude-update':
            continue
        spec = fixed[item['label']]
        with as_owner(p['owner_uid']):
            if present(Path(spec['path'])):
                raise ValueError('uninstall legacy scheduler remains')
        if uninstall_service(spec['domain']+'/'+spec['label']):
            raise ValueError('uninstall legacy scheduler is registered')


def component_keys(p):
    return ['app','helper','guardian','daemon','agent','record'] + (['old_app'] if p.get('previous_edition') else [])


def installed_state(edition):
    result = {}
    for key, path in zip(['app','helper','guardian','daemon','agent','record'],
                          [app(edition), HELPER, GUARDIAN, DAEMON, AGENT, RECORD]):
        if not path.exists() and not path.is_symlink():
            result[key] = None
            continue
        meta = path.lstat()
        item = dict(sha256=tree(path), uid=meta.st_uid, gid=meta.st_gid,
                    mode=stat.S_IMODE(meta.st_mode))
        if key in ('app','helper','guardian'):
            role = edition if key == 'app' else key
            item['cdhash'] = identity(path, 'local.claudeswitcher.'+role)
        result[key] = item
    return result


def private_json(path, mode):
    safe_parents(path.parent)
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        metadata = os.fstat(stream.fileno())
        if (not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != 0 or
                metadata.st_nlink != 1 or stat.S_IMODE(metadata.st_mode) != mode or metadata.st_size > 1024*1024):
            raise ValueError('unsafe root metadata: '+str(path))
        return json.load(stream)


def boot_uuid():
    value = run('/usr/sbin/sysctl','-n','kern.bootsessionuuid').stdout.strip()
    if not re.fullmatch(r'[a-fA-F0-9]{8}(?:-[a-fA-F0-9]{4}){3}-[a-fA-F0-9]{12}',value):
        raise ValueError('boot identity unavailable')
    return value


def finish(j, status, evidence):
    recheck_managed(j)
    if status == 'uninstalled':
        check_uninstall_holdings(j)
        verify_uninstalled(j['plan'])
    if status == 'restored' and 'uninstall_snapshot' in j['plan']:
        verify_uninstall_restored(j)
    # Persist terminal state before cleanup; retry after any cleanup crash is safe.
    j['pending'] = status
    j['evidence'] = evidence
    save(JOURNAL, j)
    archive = Path(j['stage'])/'result.json'
    save(archive, j)
    save(LAST_RESULT, dict(status=status, stage=j['stage'], owner_uid=j['plan']['owner_uid'],
                           edition=j['plan']['edition']))
    for path in [CHALLENGE, RECEIPT, BARRIER]:
        if path.exists():
            path.unlink()
            syncdir(SYSTEM)
    JOURNAL.unlink()
    syncdir(SYSTEM)
    return dict(status=status, evidence=str(archive), backup=j['stage'])


def resume_terminal(owner):
    if not JOURNAL.exists():
        result = private_json(LAST_RESULT,0o600)
        if result['owner_uid'] != owner:
            raise ValueError('console owner differs from completed transaction')
        if result['status'] == 'uninstalled':
            return verify(owner)
        return result
    j = load_journal()
    if j['plan']['owner_uid'] != owner:
        raise ValueError('console owner differs from journal')
    if j['pending'] in ('committed','restored','backup_aborted','uninstalled'):
        return finish(j,j['pending'],j.get('evidence',{}))
    return None


def safe_parents(path):
    for p in [Path(path), *Path(path).parents]:
        m = p.lstat()
        allowed_admin = p == Path('/Applications') and m.st_gid == 80 and not m.st_mode & 0o002
        if not stat.S_ISDIR(m.st_mode) or m.st_uid != 0 or (m.st_mode & 0o022 and not allowed_admin):
            raise ValueError(f'unsafe root directory: {p}')


def ensure_system_directories():
    # Only fixed system children may be created. Never alter /Applications mode.
    for directory in [SYSTEM, HELPER.parent, DAEMON.parent, AGENT.parent]:
        safe_parents(directory.parent)
        directory.mkdir(mode=0o755, exist_ok=True)
        safe_parents(directory)


def rename_exclusive(source_fd, source, destination_fd, destination):
    # macOS RENAME_EXCL prevents a group member from planting a replacement
    # between parking the old App and publishing the private staged App.
    libc = ctypes.CDLL(None, use_errno=True)
    call = libc.renameatx_np
    call.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    call.restype = ctypes.c_int
    if call(source_fd, os.fsencode(source), destination_fd, os.fsencode(destination), 4):
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code))


def replace_component(source, target, stage, is_app=False):
    if not is_app:
        remove(target)
        if source is not None:
            copy(source, target)
        return
    safe_parents(target.parent)
    parentfd = os.open(target.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    stagefd = os.open(stage, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        def parent_unchanged():
            before, now = os.fstat(parentfd), target.parent.lstat()
            if (before.st_dev, before.st_ino) != (now.st_dev, now.st_ino):
                raise ValueError('Applications parent changed')
        parent_unchanged()
        prepared = 'publish-' + uuid.uuid4().hex
        if source is not None:
            copy(source, stage/prepared)
            expected = tree(source)
            if tree(stage/prepared) != expected:
                raise ValueError('prepared App differs')
        try:
            metadata = os.stat(target.name, dir_fd=parentfd, follow_symlinks=False)
        except FileNotFoundError:
            metadata = None
        if metadata is not None:
            if stat.S_ISLNK(metadata.st_mode):
                raise ValueError('App destination symlink')
            # Move the directory entry itself into private storage. Never recurse
            # into a mutable /Applications entry as root.
            rename_exclusive(parentfd, target.name, stagefd, 'displaced-' + uuid.uuid4().hex)
            os.fsync(parentfd); os.fsync(stagefd)
        if source is not None:
            rename_exclusive(stagefd, prepared, parentfd, target.name)
            os.fsync(parentfd); os.fsync(stagefd)
            parent_unchanged()
            if tree(target) != expected:
                raise ValueError('published App changed')
        parent_unchanged()
    finally:
        os.close(stagefd)
        os.close(parentfd)


def syncdir(path):
    fd = os.open(path, os.O_RDONLY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def save(path, value, mode=0o600):
    temp = path.with_name(path.name + '.new-' + uuid.uuid4().hex)
    fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        with os.fdopen(fd, 'w') as f:
            os.fchmod(f.fileno(), mode)
            json.dump(value, f, sort_keys=True)
            f.flush()
            os.fsync(f.fileno())
        os.replace(temp, path)
        syncdir(path.parent)
    finally:
        if temp.exists():
            temp.unlink()


def copy(source, target):
    """Copy through no-follow descriptors; a source swap never follows a link."""
    def node(parentfd, name, destination):
        fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parentfd)
        try:
            m = os.fstat(fd)
            if stat.S_ISDIR(m.st_mode):
                destination.mkdir(mode=stat.S_IMODE(m.st_mode))
                destination.chmod(stat.S_IMODE(m.st_mode))
                for child in sorted(os.listdir(fd)):
                    node(fd, child, destination/child)
                syncdir(destination)
            elif stat.S_ISREG(m.st_mode) and m.st_nlink == 1:
                out = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                              stat.S_IMODE(m.st_mode))
                try:
                    while True:
                        chunk = os.read(fd, 1024 * 1024)
                        if not chunk:
                            break
                        view = memoryview(chunk)
                        while view:
                            view = view[os.write(out, view):]
                    os.fchmod(out, stat.S_IMODE(m.st_mode))
                    os.fsync(out)
                finally:
                    os.close(out)
            else:
                raise ValueError('unsafe source type/link count')
        finally:
            os.close(fd)
    # Resolve each ancestor via descriptor to reject parent symlink swaps too.
    fd = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
    try:
        for part in source.absolute().parts[1:-1]:
            nextfd = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            os.close(fd)
            fd = nextfd
        node(fd, source.name, target)
    finally:
        os.close(fd)
    syncdir(target.parent)


def remove(path):
    if path.is_symlink():
        raise ValueError(f'unsafe destination: {path}')
    if path.is_dir():
        shutil.rmtree(path)
    elif path.exists():
        path.unlink()
    syncdir(path.parent)


def stopped(owner):
    """Quit only the console owner's Claude/switcher processes before replacement."""
    def running():
        result = []
        for line in run('/bin/ps', '-ww', '-axo', 'pid=,uid=,comm=').stdout.splitlines():
            fields = line.strip().split(None, 2)
            if len(fields) != 3:
                continue
            pid, uid, command = fields
            if int(uid) == owner and ('/Claude.app/Contents/' in command or 'Claude 切换器' in command
                                     or command in ('Claude', 'switcher-desktop', 'ClaudeSwitcher')):
                result.append(int(pid))
        return result
    for pid in running():
        try:
            os.kill(pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
    deadline = time.monotonic() + 15
    while running():
        if time.monotonic() >= deadline:
            raise ValueError('application did not exit after termination request; installation unchanged')
        time.sleep(0.1)


def process_groups():
    # Darwin's unlimited getgroups variant (used by Python) returns directory
    # memberships, not the kernel list accepted by setgroups. Use the original
    # libc symbol to preserve the exact process list; never truncate it.
    libc=ctypes.CDLL(None,use_errno=True)
    getgroups=libc.getgroups
    getgroups.argtypes=[ctypes.c_int,ctypes.POINTER(ctypes.c_uint)]
    getgroups.restype=ctypes.c_int
    count=getgroups(0,None)
    if count < 0:
        error=ctypes.get_errno()
        raise OSError(error,os.strerror(error))
    if count > os.sysconf('SC_NGROUPS_MAX'):
        raise ValueError('kernel group list exceeds setgroups limit')
    groups=(ctypes.c_uint * count)()
    actual=getgroups(count,groups)
    if actual < 0:
        error=ctypes.get_errno()
        raise OSError(error,os.strerror(error))
    if actual > count:
        raise ValueError('kernel group list changed during snapshot')
    return list(groups[:actual])


@contextlib.contextmanager
def as_owner(owner):
    original_uid=os.geteuid()
    if original_uid != 0 and original_uid != owner:
        raise ValueError('console owner required')
    if original_uid == 0:
        account=pwd.getpwuid(owner)
        original_gid=os.getegid()
        original_groups=process_groups()
    try:
        if original_uid == 0:
            os.initgroups(account.pw_name,account.pw_gid)
            os.setegid(account.pw_gid)
            os.seteuid(owner)
        yield
    finally:
        if original_uid == 0:
            os.seteuid(0)
            try:
                os.setegid(original_gid)
            finally:
                os.setgroups(original_groups)


@contextlib.contextmanager
def directory_fd(path):
    fd=os.open('/',os.O_RDONLY|os.O_DIRECTORY)
    try:
        for part in Path(path).absolute().parts[1:]:
            nextfd=os.open(part,os.O_RDONLY|os.O_DIRECTORY|os.O_NOFOLLOW,dir_fd=fd)
            os.close(fd); fd=nextfd
        yield fd
    finally:
        os.close(fd)


def small_file(path, limit=1024*1024):
    with directory_fd(path.parent) as parent:
        fd=os.open(path.name,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK,dir_fd=parent)
        with os.fdopen(fd,'rb') as stream:
            meta=os.fstat(stream.fileno())
            if (not stat.S_ISREG(meta.st_mode) or meta.st_nlink!=1 or
                    (limit is not None and meta.st_size>limit)):
                raise ValueError('unsafe legacy file')
            return stream.read(),meta


def legacy_specs(owner):
    home=Path(pwd.getpwuid(owner).pw_dir)
    return [dict(label='local.claude-bodyguard',domain='system',
                 path='/Library/LaunchDaemons/local.claude-bodyguard.plist',uid=0,interval=10,run=True,
                 arguments=['/bin/bash',str(SYSTEM/'claude-bodyguard.sh')]),
            dict(label='local.tz-watchdog',domain='system',
                 path='/Library/LaunchDaemons/local.tz-watchdog.plist',uid=0,interval=3600,run=True,
                 arguments=['/bin/bash',str(SYSTEM/'tz-watchdog.sh')]),
            dict(label='local.claude-update',domain='gui/'+str(owner),uid=owner,interval=43200,run=False,
                 path=str(home/'Library/LaunchAgents/local.claude-update.plist'),
                 arguments=['/bin/bash',str(home/'.local/bin/claude-switch.sh'),'update','--auto'])]


CONFIG_KEYS = ('COUNTRY_TARGET','TZ_TARGET','LOCALE_TARGET','PROXY_MODE','PROXY_GROUP')


def config_literals(data):
    result = {}
    for line in data.decode('utf-8').splitlines():
        for key in CONFIG_KEYS:
            if not line.startswith(key+'='):
                continue
            value=line[len(key)+1:]
            if not value.startswith(('"', "'")):
                continue
            values=shlex.split(value, comments=True, posix=True)
            if len(values) != 1 or any(ord(c) < 32 for c in values[0]):
                raise ValueError('invalid legacy configuration literal')
            result[key]=values[0]
    return result


def cli_path(owner):
    return Path(pwd.getpwuid(owner).pw_dir)/'.local/bin/claude-switch.sh'


def cli_snapshot(owner):
    with as_owner(owner) if os.geteuid() == 0 else contextlib.nullcontext():
        try:
            data, metadata=small_file(cli_path(owner))
        except FileNotFoundError:
            return dict(sha256=None, mode=None, literals={})
        if metadata.st_uid != owner:
            raise ValueError('legacy CLI must belong to installation owner')
        return dict(sha256=hashlib.sha256(data).hexdigest(), mode=stat.S_IMODE(metadata.st_mode),
                    literals=config_literals(data))


def cli_shim(edition, entry):
    lines=['#!/bin/bash', '# Configuration literals retained for the Rust configuration reader.']
    # Force quotes even for simple values because both configuration readers
    # accept quoted assignments only.
    literals = entry.get('install_literals', entry['literals'])
    for key in CONFIG_KEYS:
        if key in literals:
            value=literals[key]
            lines.append(key+"='"+value.replace("'", "'\\''")+"'")
    executable=app(edition)/'Contents/Resources/components/claude-switcher'
    lines.append('exec '+shlex.quote(str(executable))+' --edition '+edition+' "$@"')
    return ('\n'.join(lines)+'\n').encode()


def cli_backup(j):
    entry=j['plan'].get('cli_entry')
    if entry is None:
        return
    owner=j['plan']['owner_uid']
    if cli_snapshot(owner) != {key: entry[key] for key in ('sha256', 'mode', 'literals')}:
        raise ValueError('legacy CLI changed since preview')
    if entry['sha256'] is not None:
        with as_owner(owner):
            data,_=small_file(cli_path(owner))
        if hashlib.sha256(data).hexdigest()!=entry['sha256']:
            raise ValueError('legacy CLI changed during backup')
        target=Path(j['stage'])/'legacy-cli.sh'
        with open(target,'xb') as stream:
            os.fchmod(stream.fileno(),0o600); stream.write(data); stream.flush(); os.fsync(stream.fileno())
    j['cli_changed']=False; save(JOURNAL,j)


def write_owner_cli(owner, data, mode, exclusive=False, before_publish=None, gid=None):
    published = None
    with contextlib.ExitStack() as descriptors:
        with as_owner(owner):
            home=Path(pwd.getpwuid(owner).pw_dir)
            # Create only the two conventional directories, via no-follow parents.
            parent=home
            for name in ('.local','bin'):
                with directory_fd(parent) as directory:
                    try: os.mkdir(name,0o755,dir_fd=directory)
                    except FileExistsError: pass
                parent=parent/name
            directory = descriptors.enter_context(directory_fd(parent))
            name='claude-switch.sh'
            if data is None:
                try: os.unlink(name,dir_fd=directory)
                except FileNotFoundError: pass
                os.fsync(directory)
                return None
            temporary='.claude-switch-'+uuid.uuid4().hex
        try:
            with as_owner(owner):
                fd=os.open(temporary,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,mode,dir_fd=directory)
                with os.fdopen(fd,'wb') as stream:
                    stream.write(data); stream.flush(); os.fchmod(stream.fileno(),mode)
                    if gid is not None:
                        os.fchown(stream.fileno(), owner, gid)
                    os.fsync(stream.fileno())
                    metadata = os.fstat(stream.fileno())
                    published = metadata.st_dev, metadata.st_ino
                os.fsync(directory)
            # Root-owned transaction evidence must not be saved as the owner.
            if before_publish is not None:
                before_publish(metadata)
            with as_owner(owner):
                if exclusive:
                    rename_exclusive(directory, temporary, directory, name)
                else:
                    os.replace(temporary,name,src_dir_fd=directory,dst_dir_fd=directory)
                os.fsync(directory)
        finally:
            with as_owner(owner):
                try: os.unlink(temporary,dir_fd=directory)
                except FileNotFoundError: pass
    return published


def cli_install(j):
    entry=j['plan'].get('cli_entry')
    if entry is None:
        return
    if 'uninstall_snapshot' in j['plan']:
        j['cli_changed'] = True; save(JOURNAL, j)
        remove_uninstall_file(j, 'cli')
        return
    owner=j['plan']['owner_uid']
    if cli_snapshot(owner) != {key: entry[key] for key in ('sha256', 'mode', 'literals')}:
        raise ValueError('legacy CLI changed before replacement')
    j['cli_changed']=True; j['pending']='cli-entry'; save(JOURNAL,j)
    write_owner_cli(owner,cli_shim(j['plan']['edition'],entry),0o755)


def cli_verify(j):
    entry=j['plan'].get('cli_entry')
    if entry is None:
        return
    owner=j['plan']['owner_uid']
    with as_owner(owner) if os.geteuid() == 0 else contextlib.nullcontext():
        data,metadata=small_file(cli_path(owner))
    if (data != cli_shim(j['plan']['edition'],entry) or metadata.st_uid != owner or
            stat.S_IMODE(metadata.st_mode) != 0o755):
        raise ValueError('Rust CLI compatibility entry differs')


def cli_restore(j):
    if not j.get('cli_changed'):
        return
    if 'uninstall_snapshot' in j['plan']:
        restore_uninstall_file(j, 'cli')
        j['cli_changed'] = False; save(JOURNAL, j)
        return
    entry=j['plan']['cli_entry']
    data=None
    if entry['sha256'] is not None:
        data,_=small_file(Path(j['stage'])/'legacy-cli.sh')
        if hashlib.sha256(data).hexdigest()!=entry['sha256']:
            raise ValueError('legacy CLI backup differs')
    owner=j['plan']['owner_uid']
    with as_owner(owner):
        try: current,_=small_file(cli_path(owner))
        except FileNotFoundError: current=None
    if current not in (None,data,cli_shim(j['plan']['edition'],entry)):
        raise ValueError('CLI entry changed after installation; preserve user changes')
    write_owner_cli(owner,data,entry['mode'] or 0o755)
    j['cli_changed']=False; save(JOURNAL,j)


LOCALES = dict(US='en_US', JP='ja_JP', TW='zh_TW', HK='zh_HK', SG='en_SG',
               GB='en_GB', DE='de_DE', FR='fr_FR', KR='ko_KR', AU='en_AU', CA='en_CA')


def detect_region():
    try:
        trace = run('/usr/bin/curl', '--fail', '--silent', '--show-error', '--max-time', '15',
                    'https://www.cloudflare.com/cdn-cgi/trace', inherit_proxy=True).stdout
    except subprocess.SubprocessError as error:
        raise ValueError('无法探测首次安装出口；请检查代理和网络后重试') from error
    match = re.search(r'^loc=([A-Z]{2})$', trace, re.M)
    if not match or match[1] == 'CN':
        raise ValueError('首次安装需要非 CN 出口；请先配置代理后重试')
    for endpoint in ('https://ipinfo.io/json', 'https://ipwho.is/', 'https://api.ip.sb/geoip'):
        try:
            info = json.loads(run('/usr/bin/curl', '--fail', '--silent', '--show-error', '--max-time', '15',
                                  endpoint, inherit_proxy=True).stdout)
            country = info.get('country') if endpoint == 'https://ipinfo.io/json' else info.get('country_code')
            zone = info.get('timezone')
            if isinstance(zone, dict):
                zone = zone.get('id')
            if country != match[1]:
                continue
            validate_timezone_target(zone)
            return dict(COUNTRY_TARGET=country, TZ_TARGET=zone, LOCALE_TARGET=LOCALES.get(country, 'en_US'))
        except (subprocess.SubprocessError, ValueError, AttributeError, TypeError):
            continue
    raise ValueError('无法取得与出口国家一致的完整地区数据；请检查代理后重试')


def detect_proxy(home):
    directory = home/'Library/Application Support/io.github.clash-verge-rev.clash-verge-rev'
    if directory.is_dir():
        group = 'GLOBAL'
        config = directory/'clash-verge.yaml'
        if config.exists():
            data, _ = small_file(config)
            text = data.decode('utf-8')
            # Read the first block-style select group, without evaluating YAML.
            section = re.search(r'(?ms)^proxy-groups:\s*\n(.*?)(?=^[^\s#-][^\n:]*:|\Z)', text)
            if section:
                for block in re.split(r'(?m)^\s*-\s+', section[1]):
                    name = re.search(r'(?m)^\s*name:\s*(.*?)\s*$', block)
                    kind = re.search(r'(?m)^\s*type:\s*[\'"]?select[\'"]?\s*$', block)
                    if name and kind:
                        raw = name[1]
                        group = shlex.split(raw, comments=True)[0] if raw.startswith(('"', "'")) else raw.split(' #',1)[0]
                        break
        return dict(PROXY_MODE='clash-verge', PROXY_GROUP=group)
    vpn = run('/usr/sbin/scutil', '--nc', 'list', check=False).stdout
    return dict(PROXY_MODE='shadowrocket' if 'com.liguangming.Shadowrocket' in vpn else 'auto',
                PROXY_GROUP='GLOBAL')


def installation_settings(owner, entry):
    home = Path(pwd.getpwuid(owner).pw_dir)
    values = dict(entry['literals'])
    with as_owner(owner) if os.geteuid() == 0 else contextlib.nullcontext():
        config = home/'.claude-switcher/v2/config.json'
        if config.exists():
            data, _ = small_file(config)
            current = json.loads(data)
            for key, field in [('COUNTRY_TARGET','country_target'), ('TZ_TARGET','timezone_target'),
                               ('LOCALE_TARGET','locale_target'), ('PROXY_MODE','proxy_mode')]:
                if current.get(field) is not None:
                    values[key] = current[field]
        if not all(values.get(key) for key in ('COUNTRY_TARGET','TZ_TARGET')):
            detected = detect_region()
            if values.get('COUNTRY_TARGET') and values['COUNTRY_TARGET'] != detected['COUNTRY_TARGET']:
                raise ValueError('已有目标国家与当前出口不一致；请先确认地区配置')
            for key, value in detected.items():
                if not values.get(key):
                    values[key] = value
        values.setdefault('LOCALE_TARGET', LOCALES.get(values['COUNTRY_TARGET'], 'en_US'))
        if not values.get('PROXY_MODE'):
            for key, value in detect_proxy(home).items():
                values.setdefault(key, value)
        values.setdefault('PROXY_GROUP', 'GLOBAL')
    if not isinstance(values['COUNTRY_TARGET'], str) or not re.fullmatch('[A-Z]{2}', values['COUNTRY_TARGET']):
        raise ValueError('configured country target is invalid')
    if not isinstance(values['LOCALE_TARGET'], str) or not re.fullmatch('[A-Za-z0-9_@.-]{1,80}', values['LOCALE_TARGET']):
        raise ValueError('configured locale target is invalid')
    if any(not isinstance(value,str) or any(ord(c)<32 for c in value) for value in values.values()):
        raise ValueError('invalid installation configuration literal')
    validate_timezone_target(values['TZ_TARGET'])
    validate_proxy_mode(values['PROXY_MODE'])
    return values


def read_timezone_target(owner):
    home = Path(pwd.getpwuid(owner).pw_dir)
    # Read owner-controlled data as that owner, never execute or source it.
    with as_owner(owner) if os.geteuid() == 0 else contextlib.nullcontext():
        config = home/'.claude-switcher/v2/config.json'
        if config.exists():
            data, _ = small_file(config)
            target = json.loads(data).get('timezone_target')
        else:
            data, _ = small_file(home/'.local/bin/claude-switch.sh')
            target = config_literals(data).get('TZ_TARGET')
    validate_timezone_target(target)
    return target


def validate_timezone_target(target):
    if (not isinstance(target, str) or not 0 < len(target) <= 80 or
            not re.fullmatch('[A-Za-z0-9/_+-]+', target) or
            any(part in ('', '.', '..') for part in target.split('/')) or
            not (ZONEINFO/target).is_file()):
        raise ValueError('configured timezone target is missing or invalid')


def watchdog_payload(target):
    validate_timezone_target(target)
    # Fixed reviewed equivalent of daemons/tz-watchdog.sh; only a validated
    # timezone literal is substituted. No user-controlled command or path.
    script = ('#!/bin/bash\nTARGET="' + target + '"\n'
              'current=$(readlink /etc/localtime 2>/dev/null)\n'
              'if [ "$current" != "/var/db/timezone/zoneinfo/$TARGET" ]; then\n'
              '    if /usr/sbin/systemsetup -settimezone "$TARGET"; then\n'
              '        logger -t local.tz-watchdog "时区异常: [$current] 已恢复为 $TARGET"\n'
              '    else\n'
              '        logger -t local.tz-watchdog "时区恢复失败: [$current] 目标为 $TARGET"\n'
              '        exit 1\n'
              '    fi\nfi\n').encode()
    plist = plistlib.dumps(dict(Label='local.tz-watchdog',
                               ProgramArguments=['/bin/bash', str(SYSTEM/'tz-watchdog.sh')],
                               StartInterval=3600, RunAtLoad=True))
    return [('script', SYSTEM/'tz-watchdog.sh', script, 0o755),
            ('plist', WATCHDOG_PLIST, plist, 0o644)]


def read_proxy_mode(owner):
    home = Path(pwd.getpwuid(owner).pw_dir)
    with as_owner(owner) if os.geteuid() == 0 else contextlib.nullcontext():
        config = home/'.claude-switcher/v2/config.json'
        if config.exists():
            data, _ = small_file(config)
            mode = json.loads(data).get('proxy_mode', 'auto')
        else:
            data, _ = small_file(home/'.local/bin/claude-switch.sh')
            mode = config_literals(data).get('PROXY_MODE', 'auto')
    validate_proxy_mode(mode)
    return mode


def validate_proxy_mode(mode):
    if mode not in ('auto', 'clash-verge', 'shadowrocket', 'sing-box', 'clashx', 'surge'):
        raise ValueError('configured proxy mode is invalid')


# Reviewed fixed legacy template; never load root-executed code from a user path.
BODYGUARD_SCRIPT = '#!/bin/bash\n# Claude 保镖 (参考模板) —— 安装程序会把 __HOME__ / __PROXY_MODE__ 替换为实际值后安装\n# 作用: 代理(内核或TUN)离线时,自动关闭 Claude,杜绝裸连重试\nMODE="__PROXY_MODE__"\n\nok=1\n\n# TUN 在线判定:存在 fake-ip 网段地址即视为 TUN 已接管\nifconfig 2>/dev/null | grep -q \'inet 198\\.18\\.\' || ok=0\n\n# 内核在线判定:按安装时识别的客户端分派\ncase "$MODE" in\n    clash-verge)\n        pgrep -f verge-mihomo >/dev/null 2>&1 || ok=0 ;;\n    shadowrocket)\n        # launchd 环境下 scutil 可能查不到 VPN 行,查不到时以 TUN 判定为准(避免误杀)\n        nc=$(scutil --nc list 2>/dev/null)\n        if echo "$nc" | grep -q \'com.liguangming.Shadowrocket\'; then\n            echo "$nc" | grep \'com.liguangming.Shadowrocket\' | grep -q \'(Connected)\' || ok=0\n        fi ;;\n    sing-box)\n        pgrep -f sing-box >/dev/null 2>&1 || ok=0 ;;\n    clashx)\n        { pgrep -x ClashX >/dev/null 2>&1 || pgrep -f clash >/dev/null 2>&1; } || ok=0 ;;\n    surge)\n        pgrep -f Surge >/dev/null 2>&1 || ok=0 ;;\n    auto)\n        : ;;  # auto 模式只查 TUN\n    *)\n        pgrep -f verge-mihomo >/dev/null 2>&1 || ok=0 ;;\nesac\n\nif [ "$ok" -eq 0 ]; then\n    if pgrep -f \'/Applications/Claude.app/Contents/MacOS/Claude\' >/dev/null 2>&1; then\n        pkill -f \'/Applications/Claude.app/Contents/MacOS/Claude\'\n        logger -t local.claude-bodyguard "代理离线(内核或TUN丢失),已自动关闭 Claude"\n    fi\nfi\n'


def bodyguard_payload(mode):
    validate_proxy_mode(mode)
    script = BODYGUARD_SCRIPT.replace('__PROXY_MODE__', mode).encode()
    plist = plistlib.dumps(dict(Label='local.claude-bodyguard',
                               ProgramArguments=['/bin/bash', str(SYSTEM/'claude-bodyguard.sh')],
                               StartInterval=10, RunAtLoad=True))
    return [('script', SYSTEM/'claude-bodyguard.sh', script, 0o755),
            ('plist', BODYGUARD_PLIST, plist, 0o644)]


def root_service_spec(j, kind):
    if kind == 'bodyguard':
        return j['plan'].get('proxy_mode'), bodyguard_payload, BODYGUARD_PLIST, 'local.claude-bodyguard'
    return j['plan'].get('timezone_target'), watchdog_payload, WATCHDOG_PLIST, 'local.tz-watchdog'


def watchdog_backup(j, kind="watchdog"):
    target, payload, service_plist, label = root_service_spec(j, kind)
    if target is None:
        return
    previous = {}
    for key, path, _, mode in payload(target):
        safe_parents(path.parent)
        if present(path):
            _, metadata = small_file(path)
            if metadata.st_uid != 0 or metadata.st_gid != 0 or stat.S_IMODE(metadata.st_mode) != mode:
                raise ValueError('unsafe existing timezone watchdog' if kind == 'watchdog' else 'unsafe existing root bodyguard')
            backup = Path(j['stage'])/(kind+'-old-'+key)
            copy(path, backup)
            previous[key] = tree(backup)
        else:
            previous[key] = None
    j[kind] = dict(previous=previous,
                         registered=run('/bin/launchctl','print','system/'+label,check=False).returncode == 0,
                         changed=False)
    save(JOURNAL,j)


def root_service_ownership(files):
    """Pin and validate all fixed service files before correcting inherited GIDs."""
    with contextlib.ExitStack() as stack:
        opened = []
        for path, expected, mode in files:
            safe_parents(path.parent)
            parent = stack.enter_context(directory_fd(path.parent))
            fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=parent)
            stream = stack.enter_context(os.fdopen(fd, 'rb'))
            metadata = os.fstat(fd)
            if (not stat.S_ISREG(metadata.st_mode) or metadata.st_nlink != 1 or
                    metadata.st_uid != 0 or stat.S_IMODE(metadata.st_mode) != mode or
                    metadata.st_size != len(expected) or stream.read() != expected):
                raise ValueError('root service ownership repair refused: file differs')
            current = os.stat(path.name, dir_fd=parent, follow_symlinks=False)
            if (current.st_dev, current.st_ino) != (metadata.st_dev, metadata.st_ino):
                raise ValueError('root service ownership repair refused: file replaced')
            opened.append((fd, metadata, parent, path.name))
        for fd, metadata, parent, name in opened:
            gid = metadata.st_gid
            if gid != 0:
                os.fchown(fd, 0, 0)
                os.fsync(fd)
            current = os.stat(name, dir_fd=parent, follow_symlinks=False)
            if (current.st_dev, current.st_ino) != (metadata.st_dev, metadata.st_ino):
                raise ValueError('root service ownership repair refused: file replaced')


def pending_service_ownership(j):
    files = []
    for kind in ('watchdog', 'bodyguard'):
        if kind in j:
            target, payload, _, _ = root_service_spec(j, kind)
            files.extend((path, data, mode) for _, path, data, mode in payload(target))
    root_service_ownership(files)


def watchdog_install(j, kind="watchdog"):
    target, payload, service_plist, label = root_service_spec(j, kind)
    if kind not in j:
        return
    if 'uninstall_snapshot' in j['plan']:
        for part in ('script', 'plist'):
            remove_uninstall_file(j, kind+'_'+part)
        return
    j[kind]['changed'] = True
    j['pending'] = label; save(JOURNAL,j)
    if run('/bin/launchctl','print','system/'+label,check=False).returncode == 0:
        run('/bin/launchctl','bootout','system/'+label)
    for key, path, data, mode in payload(target):
        safe_parents(path.parent)
        staged = Path(j['stage'])/(kind+'-new-'+key)
        with open(staged, 'xb') as stream:
            stream.write(data); stream.flush(); os.fchmod(stream.fileno(), mode); os.fsync(stream.fileno())
        replace_component(staged, path, Path(j['stage']))
        root_service_ownership([(path, data, mode)])
    run('/bin/launchctl','bootstrap','system',service_plist)


def watchdog_verify(j, kind="watchdog"):
    target, payload, service_plist, label = root_service_spec(j, kind)
    if kind not in j:
        return
    for _, path, expected, mode in payload(target):
        safe_parents(path.parent)
        data, metadata = small_file(path)
        if (data != expected or metadata.st_uid != 0 or metadata.st_gid != 0 or
                stat.S_IMODE(metadata.st_mode) != mode):
            raise ValueError(label + ' installation differs')
    if run('/bin/launchctl','print','system/'+label,check=False).returncode != 0:
        raise ValueError(label + ' service is not registered')


def watchdog_restore(j, kind="watchdog"):
    target, payload, service_plist, label = root_service_spec(j, kind)
    state = j.get(kind)
    if not state or not state['changed']:
        return
    if run('/bin/launchctl','print','system/'+label,check=False).returncode == 0:
        run('/bin/launchctl','bootout','system/'+label)
    for key, path, _, mode in payload(target):
        if 'uninstall_snapshot' in j['plan']:
            restore_uninstall_file(j, kind+'_'+key)
            continue
        safe_parents(path.parent)
        expected = state['previous'][key]
        backup = Path(j['stage'])/(kind+'-old-'+key)
        if expected is not None and tree(backup) != expected:
            raise ValueError(label + ' backup differs')
        data = small_file(backup)[0] if expected is not None else None
        replace_component(backup if expected is not None else None, path, Path(j['stage']))
        if data is not None:
            root_service_ownership([(path, data, mode)])
    if state['registered']:
        run('/bin/launchctl','bootstrap','system',service_plist)
    state['changed'] = False; save(JOURNAL,j)


def legacy_disabled(domain,label):
    output=run('/bin/launchctl','print-disabled',domain).stdout
    value=re.findall(r'"'+re.escape(label)+r'"\s*=>\s*(true|false)',output)
    if len(value)>1:
        raise ValueError('ambiguous legacy disabled state')
    return value[0]=='true' if value else None


def legacy_snapshot(owner, include_root=True):
    result=[]
    for spec in legacy_specs(owner):
        if not include_root and not spec['uid']:
            continue
        path=Path(spec['path'])
        with as_owner(owner) if spec['uid'] else contextlib.nullcontext():
            try:
                data,meta=small_file(path)
            except FileNotFoundError:
                if run('/bin/launchctl','print',spec['domain']+'/'+spec['label'],check=False).returncode==0:
                    raise ValueError('legacy service registered without its fixed plist')
                continue
        if meta.st_uid!=spec['uid'] or stat.S_IMODE(meta.st_mode)!=0o644:
            raise ValueError('legacy plist owner/mode differs: '+str(path))
        if spec['uid']:
            account=pwd.getpwuid(spec['uid'])
            if meta.st_gid not in os.getgrouplist(account.pw_name,account.pw_gid):
                raise ValueError('legacy plist group cannot be restored by its owner: '+str(path))
        expected=dict(Label=spec['label'],ProgramArguments=spec['arguments'],
                      RunAtLoad=spec['run'],StartInterval=spec['interval'])
        if plistlib.loads(data)!=expected:
            raise ValueError('legacy plist is not the fixed supported template: '+str(path))
        if not spec['uid']:
            safe_parents(path.parent)
            script=Path(spec['arguments'][1]); safe_parents(script.parent)
            script_data,script_meta=small_file(script)
            if script_meta.st_uid or script_meta.st_mode & 0o022:
                raise ValueError('unsafe legacy root script')
            script_hash=hashlib.sha256(script_data).hexdigest()
        else:
            script_hash=None
        result.append(dict(spec,sha256=hashlib.sha256(data).hexdigest(),gid=meta.st_gid,mode=0o644,
                           script_sha256=script_hash,
                           registered=run('/bin/launchctl','print',spec['domain']+'/'+spec['label'],check=False).returncode==0,
                           disabled=legacy_disabled(spec['domain'],spec['label'])))
    return result


def legacy_backup(j):
    for item in j['plan']['legacy']:
        with as_owner(item['uid']) if item['uid'] else contextlib.nullcontext():
            data,_=small_file(Path(item['path']))
        if hashlib.sha256(data).hexdigest()!=item['sha256']:
            raise ValueError('legacy changed after preview')
        target=Path(j['stage'])/('legacy-'+item['label']+'.plist')
        fd=os.open(target,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
        with os.fdopen(fd,'wb') as stream:
            stream.write(data); stream.flush(); os.fsync(stream.fileno())
        syncdir(target.parent)


def legacy_migrate(j):
    if legacy_snapshot(j['plan']['owner_uid'], include_root='uninstall_snapshot' not in j['plan'])!=j['plan']['legacy']:
        raise ValueError('legacy services changed after preview')
    for item in j['plan']['legacy']:
        if item['label'] in ('local.tz-watchdog', 'local.claude-bodyguard'):
            continue
        j['pending']='legacy-'+item['label']; save(JOURNAL,j)
        target=item['domain']+'/'+item['label']
        if item['registered']:
            run('/bin/launchctl','bootout',target)
        path=Path(item['path'])
        with as_owner(item['uid']) if item['uid'] else contextlib.nullcontext():
            data,_=small_file(path)
            if hashlib.sha256(data).hexdigest()!=item['sha256']:
                raise ValueError('legacy plist changed before removal')
            with directory_fd(path.parent) as parent:
                os.unlink(path.name,dir_fd=parent); os.fsync(parent)


def legacy_restore(j):
    for item in j['plan']['legacy']:
        if ('uninstall_snapshot' in j['plan'] and
                item['label'] in ('local.tz-watchdog', 'local.claude-bodyguard')):
            continue
        j['pending']='restore-legacy-'+item['label']; save(JOURNAL,j)
        data,_=small_file(Path(j['stage'])/('legacy-'+item['label']+'.plist'))
        if hashlib.sha256(data).hexdigest()!=item['sha256']:
            raise ValueError('legacy backup differs')
        if not item['uid']:
            script_data,meta=small_file(Path(item['arguments'][1]))
            if meta.st_uid or meta.st_mode & 0o022 or hashlib.sha256(script_data).hexdigest()!=item['script_sha256']:
                raise ValueError('legacy root script changed; cannot reactivate')
        path=Path(item['path'])
        with as_owner(item['uid']) if item['uid'] else contextlib.nullcontext():
            with directory_fd(path.parent) as parent:
                try:
                    present,_=small_file(path)
                except FileNotFoundError:
                    fd=os.open(path.name,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o644,dir_fd=parent)
                    with os.fdopen(fd,'wb') as stream:
                        os.fchown(stream.fileno(),item['uid'],item['gid'])
                        stream.write(data); stream.flush(); os.fchmod(stream.fileno(),item['mode']); os.fsync(stream.fileno())
                    os.fsync(parent)
                else:
                    if present!=data:
                        raise ValueError('legacy destination changed; refusing overwrite')
                    fd=os.open(path.name,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK,dir_fd=parent)
                    try:
                        metadata=os.fstat(fd)
                        if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid!=item['uid'] or metadata.st_nlink!=1:
                            raise ValueError('legacy destination metadata changed')
                        os.fchown(fd,item['uid'],item['gid'])
                        os.fchmod(fd,item['mode']); os.fsync(fd)
                    finally:
                        os.close(fd)
        if legacy_disabled(item['domain'],item['label'])!=item['disabled']:
            raise ValueError('legacy disabled override changed; refusing to modify it')
        target=item['domain']+'/'+item['label']
        registered=run('/bin/launchctl','print',target,check=False).returncode==0
        if item['registered'] and not registered:
            # Restoring RunAtLoad may immediately run the original scheduled task.
            run('/bin/launchctl','bootstrap',item['domain'],path)
        elif registered and not item['registered']:
            raise ValueError('unexpected legacy service registration')
    if legacy_snapshot(j['plan']['owner_uid'], include_root='uninstall_snapshot' not in j['plan'])!=j['plan']['legacy']:
        raise ValueError('legacy restoration differs from preview')


@contextlib.contextmanager
def lifecycle_lock(owner):
    home = Path(pwd.getpwuid(owner).pw_dir)
    directory = home/'.claude-switcher/v2'
    # Root never creates or traverses a user-owned directory. Drop effective UID
    # while opening the same lifecycle flock used by the GUI, then retain its FD.
    os.seteuid(owner)
    try:
        for parent in [home, home/'.claude-switcher', directory]:
            if parent.is_symlink():
                raise ValueError('unsafe lifecycle directory')
            if not parent.exists():
                parent.mkdir(mode=0o700)
            m = parent.stat()
            if m.st_uid != owner or m.st_mode & 0o022:
                raise ValueError('unsafe lifecycle owner/mode')
        fd = os.open(directory/'lifecycle.lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    finally:
        os.seteuid(0)
    try:
        m = os.fstat(fd)
        if not stat.S_ISREG(m.st_mode) or m.st_uid != owner or m.st_nlink != 1 or m.st_mode & 0o077:
            raise ValueError('unsafe lifecycle lock')
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        yield
    finally:
        os.close(fd)


@contextlib.contextmanager
def locks(owner, previous_owner=None):
    with contextlib.ExitStack() as stack:
        for uid in sorted({owner, previous_owner} - {None}):
            stack.enter_context(lifecycle_lock(uid))
        safe_parents(SYSTEM.parent)
        SYSTEM.mkdir(mode=0o755, exist_ok=True)
        safe_parents(SYSTEM)
        rootfd = os.open(SYSTEM/'root-operation.lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
        try:
            metadata = os.fstat(rootfd)
            if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid or metadata.st_nlink != 1 or metadata.st_mode & 0o022:
                raise ValueError('unsafe root lock')
            fcntl.flock(rootfd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            yield
        finally:
            os.close(rootfd)


def service(action, owner, check=True, only=None):
    for domain, label, plist in [('gui/'+str(owner), 'local.claudeswitcher.guardian', AGENT),
                                  ('system', 'local.claudeswitcher.helper', DAEMON)]:
        target = domain+'/'+label
        if only is not None and target not in only:
            continue
        if action == 'bootout':
            if run('/bin/launchctl', 'print', target, check=False).returncode == 0:
                run('/bin/launchctl', 'bootout', target, check=check)
        elif plist.exists():
            run('/bin/launchctl', 'bootstrap', domain, plist, check=check)
            if domain == 'system':
                run('/bin/launchctl', 'kickstart', target, check=check)


def transaction_owners(p):
    return sorted({p['owner_uid'], p.get('previous_owner_uid', p['owner_uid'])})


def transaction_registered(p):
    return sorted({target for uid in transaction_owners(p) for target in registered_services(uid)})


def transaction_service(action, p, only=None):
    for index, uid in enumerate(transaction_owners(p)):
        targets = ['gui/'+str(uid)+'/local.claudeswitcher.guardian']
        if index == 0:
            targets.append('system/local.claudeswitcher.helper')
        service(action, uid, only=[target for target in targets if only is None or target in only])


def load_journal():
    m = JOURNAL.lstat()
    if not stat.S_ISREG(m.st_mode) or m.st_uid or m.st_mode & 0o077 or m.st_nlink != 1:
        raise ValueError('unsafe journal')
    return json.loads(JOURNAL.read_text())


def present(path):
    # lstat also detects dangling links; pending repair state must never disappear.
    try:
        path.lstat()
        return True
    except FileNotFoundError:
        return False


def managed_json(path, mode):
    safe_parents(path.parent)
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError('duplicate managed state JSON key')
            result[key] = value
        return result
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        if (not stat.S_ISREG(before.st_mode) or before.st_uid != 0 or before.st_nlink != 1 or
                stat.S_IMODE(before.st_mode) != mode or before.st_size > 16384):
            raise ValueError('unsafe managed state metadata')
        data = stream.read(16385)
        after = os.fstat(stream.fileno())
        fields = ('st_dev','st_ino','st_uid','st_gid','st_mode','st_nlink','st_size','st_mtime_ns','st_ctime_ns')
        snapshot = {key:getattr(before,key) for key in fields}
        if len(data) > 16384 or snapshot != {key:getattr(after,key) for key in fields}:
            raise ValueError('managed state changed while reading')
        snapshot['sha256'] = hashlib.sha256(data).hexdigest()
        return json.loads(data, object_pairs_hook=unique), snapshot


def managed_results_directory():
    path = SYSTEM/'repair-results'
    safe_parents(path.parent)
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        metadata = os.fstat(fd)
        if not stat.S_ISDIR(metadata.st_mode) or metadata.st_uid != 0 or metadata.st_mode & 0o077:
            raise ValueError('unsafe managed repair directory')
        current = path.lstat()
        if (current.st_dev, current.st_ino) != (metadata.st_dev, metadata.st_ino):
            raise ValueError('managed repair directory identity changed')
        # Receipt creation can change directory timestamps; identity and access
        # permissions, rather than timestamps, define the directory snapshot.
        return {key:getattr(metadata,key) for key in ('st_dev','st_ino','st_uid','st_gid','st_mode')}
    finally:
        os.close(fd)


def recheck_managed(j):
    if 'managed_system' in j:
        # Components may already be replaced. Validate the immutable repair files
        # against the snapshot without treating candidate code as the old record.
        for name in ('repair-journal.json', '.repair-journal.next', '.system-original.next'):
            if present(SYSTEM/name):
                raise ValueError('managed state conflict; transaction retained')
        expected = j['managed_system']
        if managed_results_directory() != expected['results_directory']:
            raise ValueError('managed repair directory conflict; transaction retained')
        for key, path in [('original', SYSTEM/'system-original.json'),
                          ('receipt', SYSTEM/'repair-results'/(expected['plan_id']+'.json'))]:
            _, observed = managed_json(path, 0o600)
            if observed != expected[key]:
                raise ValueError('managed state conflict; transaction retained')
        if managed_results_directory() != expected['results_directory']:
            raise ValueError('managed repair directory conflict; transaction retained')


def transact(p, expected):
    if digest(p) != expected:
        raise ValueError('plan digest changed; preview again')
    if present(JOURNAL) or present(BARRIER):
        raise ValueError('recovery required; an installation is pending')
    if 'uninstall_snapshot' in p:
        expected_paths = list(map(str, list(uninstall_files(p).values())[:6]))
        if (p['operation'] != 'uninstall' or p['payload'] or p.get('previous_edition') or
                p['destinations'] != expected_paths or
                plan_uninstall(dict(p)) != p):
            raise ValueError('uninstall plan ownership changed; preview again')
    # Repair records belong to the helper. Preserve them without making their
    # semantic state an installer prerequisite. Old journals retain rechecks.
    managed = None
    for target in p['destinations']:
        safe_parents(Path(target).parent)
        if Path(target).is_symlink():
            raise ValueError('destination symlink')
    if RECORD.exists():
        old = private_json(RECORD,0o644)
        if old['owner_uid'] != p.get('previous_owner_uid',p['owner_uid']):
            raise ValueError('previous owner changed since preview')
        if old['edition'] != p.get('previous_edition',p['edition']):
            raise ValueError('previous edition changed since preview')
        if p.get('previous_edition') and old['trust']['gui_cdhash'] != p['previous']['old_app']['cdhash']:
            raise ValueError('previous edition identity changed since preview')
    if plan_state(p) != p['previous']:
        raise ValueError('installed components changed since preview')
    ensure_system_directories()
    stage = SYSTEM / ('source-local-' + uuid.uuid4().hex)
    stage.mkdir(mode=0o700)
    generation = stage.name.removeprefix('source-local-')
    for key, item in p['payload'].items():
        target = stage/key
        copy(Path(item['source']), target)
        identifier = 'local.claudeswitcher.' + (p['edition'] if key == 'app' else key)
        if tree(target) != item['sha256'] or identity(target, identifier) != item['cdhash']:
            raise ValueError('staged payload differs from approved plan')
    if p['operation'] != 'uninstall':
        for key, target in [('daemon', DAEMON), ('agent', AGENT)]:
            copy(TEMPLATES/target.name, stage/key)
            if hashlib.sha256((stage/key).read_bytes()).hexdigest() != p['templates'][target.name]:
                raise ValueError('template changed')
        save(stage/'record', dict(schema_version=2, owner_uid=p['owner_uid'], edition=p['edition'],
             trust=dict(kind='source_local', generation=generation,
                        **{('gui' if k == 'app' else k)+'_cdhash': v['cdhash'] for k,v in p['payload'].items()}),
             helper_version=p['version'], minimum_app_version=p['version']))
        (stage/'record').chmod(0o644)
    if plan_state(p) != p['previous']:
        raise ValueError('installed components changed since preview')
    keys = component_keys(p)
    active = transaction_registered(p)
    j = dict(plan=p, stage=str(stage), pending='backup', completed=[], old=[], status='pending', active=active)
    if managed is not None:
        j['managed_system'] = managed
    save(JOURNAL, j)
    for key, target in zip(keys, p['destinations']):
        if Path(target).exists():
            backup=stage/('old-'+key)
            copy(Path(target), backup)
            expected_old=p['previous'][key]
            if expected_old is None or tree(backup)!=expected_old['sha256']:
                raise ValueError('backup differs from approved previous component')
            if 'cdhash' in expected_old:
                role=p['previous_edition'] if key=='old_app' else (p['edition'] if key=='app' else key)
                if identity(backup,'local.claudeswitcher.'+role)!=expected_old['cdhash']:
                    raise ValueError('backup code identity differs')
            j['old'].append(key)
            save(JOURNAL, j)
    legacy_backup(j)
    cli_backup(j)
    watchdog_backup(j)
    watchdog_backup(j, "bodyguard")
    if plan_state(p)!=p['previous']:
        raise ValueError('installed components changed during backup')
    recheck_uninstall(p)
    check_uninstall_backups(j)
    j['pending'] = 'services'; save(JOURNAL, j)
    boot = boot_uuid()
    recheck_managed(j)
    fd = os.open(BARRIER, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o644)
    with os.fdopen(fd, 'w') as barrier:
        barrier.write(boot)
        barrier.flush()
        os.fsync(barrier.fileno())
    syncdir(SYSTEM)
    recheck_managed(j)
    check_uninstall_backups(j)
    for uid in transaction_owners(p):
        stopped(uid)
    recheck_uninstall(p)
    legacy_migrate(j)
    if 'uninstall_snapshot' in p:
        # Persist rollback responsibility before any of the four jobs is stopped.
        for kind in ('watchdog', 'bodyguard'):
            if kind in j:
                j[kind]['changed'] = True
        save(JOURNAL, j)
        for target, plist, arguments in uninstall_service_specs(p):
            registered = uninstall_service(target, plist, arguments)
            if registered != p['uninstall_snapshot']['services'][target]['registered']:
                raise ValueError('uninstall service changed before stopping: '+target)
            if registered:
                run('/bin/launchctl', 'bootout', target)
            if uninstall_service(target):
                raise ValueError('uninstall service remains registered: '+target)
    else:
        transaction_service('bootout', p)
    recheck_managed(j)
    pairs=list(zip(keys,p['destinations']))
    # Retire the old edition before publishing the new one; rollback removes
    # the new edition before restoring the old App.
    pairs.sort(key=lambda item:item[0] != 'old_app')
    for key, target in pairs:
        j['pending'] = key; save(JOURNAL, j)
        if 'uninstall_snapshot' in p:
            remove_uninstall_file(j, key)
        else:
            replace_component(stage/key if p['operation'] != 'uninstall' and key != 'old_app' else None,
                              Path(target), stage, is_app=key in ('app','old_app'))
        j['completed'].append(key); save(JOURNAL, j)
    cli_install(j)
    watchdog_install(j)
    watchdog_install(j, "bodyguard")
    j['pending'] = 'health'; save(JOURNAL, j)
    if p['operation'] != 'uninstall':
        service('bootstrap', p['owner_uid'])
    return dict(status='pending_component_verification', recovery='sudo python3 packaging/macos/local_install.py recover')


def recover(owner):
    terminal = resume_terminal(owner)
    if terminal is not None:
        return terminal
    j = load_journal()
    if j['plan']['owner_uid'] != owner:
        raise ValueError('console owner differs from journal')
    recheck_managed(j)
    stage = Path(j['stage'])
    if stage.parent != SYSTEM or not re.fullmatch('source-local-[a-f0-9]{32}', stage.name):
        raise ValueError('unsafe recovery stage')
    safe_parents(stage)
    # Backup never mutated destinations. Preserve this terminal fact across retries.
    if j['pending'] in ('backup', 'backup_aborted'):
        if (plan_state(j['plan']) != j['plan']['previous'] or
                legacy_snapshot(owner, include_root='uninstall_snapshot' not in j['plan'])!=j['plan']['legacy']):
            raise ValueError('untouched installation changed; manual recovery required')
        recheck_uninstall(j['plan'])
        return finish(j,'backup_aborted',dict(installed_unchanged=True))
    check_uninstall_recovery(j)
    if j['pending'] != 'restored_health':
        transaction_service('bootout', j['plan'])
        for key, target in zip(component_keys(j['plan']), j['plan']['destinations']):
            allowed=[app(j['plan']['edition']),HELPER,GUARDIAN,DAEMON,AGENT,RECORD]
            if j['plan'].get('previous_edition') in ('personal','public'):
                allowed.append(app(j['plan']['previous_edition']))
            if target not in list(map(str,allowed)):
                raise ValueError('unsafe recovery target')
            safe_parents(Path(target).parent)
            j['pending'] = 'restore-'+key; save(JOURNAL,j)
            if 'uninstall_snapshot' in j['plan']:
                restore_uninstall_file(j, key)
            else:
                replace_component(stage/('old-'+key) if key in j['old'] else None,
                                  Path(target), stage, is_app=key in ('app','old_app'))
        cli_restore(j)
        watchdog_restore(j)
        watchdog_restore(j, "bodyguard")
        legacy_restore(j)
        transaction_service('bootstrap', j['plan'], only=j['active'])
    j['pending'] = 'restored_health'; save(JOURNAL,j)
    observed = plan_state(j['plan'])
    # Files copied by root remain root-owned; byte/mode/signature restoration is exact.
    for key, previous in j['plan']['previous'].items():
        current = observed[key]
        if previous is None:
            if current is not None:
                raise ValueError('recovery left an unexpected component')
        elif current is None or any(current.get(field) != previous.get(field) for field in ['sha256','mode','cdhash']):
            raise ValueError('restored component differs: '+key)
    registered = transaction_registered(j['plan'])
    if sorted(registered) != sorted(j['active']):
        raise ValueError('restored service registration differs')
    if 'uninstall_snapshot' in j['plan']:
        check_uninstall_recovery(j)
        if uninstall_snapshot(j['plan'])['services'] != j['plan']['uninstall_snapshot']['services']:
            raise ValueError('restored uninstall service registration differs')
    return finish(j,'restored',dict(components=observed, registered=registered))


def registered_services(owner):
    return [target for target in ['system/local.claudeswitcher.helper',
            'gui/'+str(owner)+'/local.claudeswitcher.guardian']
            if run('/bin/launchctl','print',target,check=False).returncode == 0]


def verify_components(j):
    if j['plan'].get('previous_edition') and present(app(j['plan']['previous_edition'])):
        raise ValueError('previous edition App remains installed')
    record = private_json(RECORD,0o644)
    expected_record = private_json(Path(j['stage'])/'record',0o644)
    if record != expected_record:
        raise ValueError('installed record differs from staged record')
    for key, path in [('app',app(record['edition'])), ('helper',HELPER), ('guardian',GUARDIAN)]:
        expected = j['plan']['payload'][key]
        role = record['edition'] if key == 'app' else key
        if tree(path) != expected['sha256'] or identity(path,'local.claudeswitcher.'+role) != expected['cdhash']:
            raise ValueError('installed identity differs: '+key)
    for key,path in [('daemon',DAEMON),('agent',AGENT)]:
        if tree(path) != tree(Path(j['stage'])/key):
            raise ValueError('installed service template differs')
    return record


def verify_processes(owner):
    processes = []
    for domain, label, path, uid in [('system','local.claudeswitcher.helper',HELPER,0),
            ('gui/'+str(owner),'local.claudeswitcher.guardian',GUARDIAN,owner)]:
        state = run('/bin/launchctl','print',domain+'/'+label).stdout
        pid = re.findall(r'^\s*pid = (\d+)\s*$',state,re.M)
        program = re.findall(r'^\s*program = (.+)$',state,re.M)
        if len(pid)!=1 or program != [str(path)]:
            raise ValueError('registered live service identity unavailable: '+label)
        process = run('/bin/ps','-ww','-p',pid[0],'-o','uid=,comm=').stdout.strip().split(None,1)
        if len(process)!=2 or int(process[0])!=uid or process[1]!=str(path):
            raise ValueError('live process differs: '+label)
        processes.append(dict(label=label,pid=int(pid[0]),uid=uid,path=str(path)))
    return processes


def create_challenge(j):
    record = verify_components(j)
    challenge = dict(schema_version=1,record=record,generation=record['trust']['generation'],
                     nonce=secrets.token_hex(32),boot_session_uuid=boot_uuid(),expires_at=int(time.time())+110)
    if RECEIPT.exists():
        RECEIPT.unlink(); syncdir(SYSTEM)
    save(CHALLENGE,challenge,mode=0o644)
    return challenge


def valid_receipt(challenge, record):
    now=int(time.time())
    if (challenge.get('schema_version') != 1 or challenge.get('record') != record or
            challenge.get('generation') != record['trust']['generation'] or
            not re.fullmatch('[a-f0-9]{64}',challenge.get('nonce','')) or
            challenge.get('boot_session_uuid') != boot_uuid() or
            type(challenge.get('expires_at')) is not int or not now <= challenge['expires_at'] <= now+120):
        raise ValueError('expired or mismatched health challenge')
    receipt=private_json(RECEIPT,0o600)
    if (set(receipt) != {'schema_version','challenge','gui_verified','guardian_verified','guardian_loop_sequence','observed_at'} or
            receipt['schema_version'] != 1 or receipt['challenge'] != challenge or
            receipt['gui_verified'] is not True or receipt['guardian_verified'] is not True or
            type(receipt['guardian_loop_sequence']) is not int or receipt['guardian_loop_sequence'] <= 0 or
            type(receipt['observed_at']) is not int or
            not challenge['expires_at']-120 <= receipt['observed_at'] <= min(now,challenge['expires_at'])):
        raise ValueError('authenticated health receipt rejected')
    return receipt


def launch_health(j, challenge):
    owner=j['plan']['owner_uid']; account=pwd.getpwuid(owner)
    bundle=app(j['plan']['edition'])
    meta=plistlib.loads((bundle/'Contents/Info.plist').read_bytes())
    name=meta['CFBundleExecutable']
    if not isinstance(name,str) or '/' in name or name in ('','.','..'):
        raise ValueError('invalid GUI executable')
    executable=bundle/'Contents/MacOS'/name
    # launchctl establishes the owner's GUI namespace; sudo then drops uid/gid.
    command=['/bin/launchctl','asuser',str(owner),'/usr/bin/sudo','-n','-u','#'+str(owner),
             '-g','#'+str(account.pw_gid),'--','/usr/bin/env','-i','HOME='+account.pw_dir,
             'PATH=/usr/bin:/bin:/usr/sbin:/sbin',str(executable),'--installation-health',
             challenge['generation'],challenge['nonce']]
    return subprocess.Popen(command,stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,
                            stderr=subprocess.DEVNULL,env={'PATH':'/usr/bin:/bin:/usr/sbin:/sbin'})


def finalize(owner, launch=False, authenticated=False):
    terminal=resume_terminal(owner)
    if terminal is not None:
        return terminal
    j=load_journal()
    if j['plan']['owner_uid'] != owner or j['pending'] != 'health':
        raise ValueError('transaction requires recovery')
    if j['plan']['operation']=='uninstall':
        verify_uninstalled(j['plan'])
        return finish(j,'uninstalled',dict(components_absent=True,services_absent=True))
    recheck_managed(j)
    record=verify_components(j)
    previous_owner=j['plan'].get('previous_owner_uid')
    if previous_owner is not None and 'gui/'+str(previous_owner)+'/local.claudeswitcher.guardian' in registered_services(previous_owner):
        raise ValueError('previous owner guardian is still registered')
    pending_service_ownership(j)
    watchdog_verify(j)
    watchdog_verify(j, "bodyguard")
    cli_verify(j)
    if not authenticated:
        processes=verify_processes(owner)
        return finish(j,'committed',dict(components_verified=True, processes=processes,
                                        authenticated_health='not_run'))
    if launch:
        challenge=create_challenge(j)
        # Retry after reboot re-registers only missing services, never uses old sessions.
        for domain,label,plist in [('system','local.claudeswitcher.helper',DAEMON),
                                  ('gui/'+str(owner),'local.claudeswitcher.guardian',AGENT)]:
            if run('/bin/launchctl','print',domain+'/'+label,check=False).returncode != 0:
                run('/bin/launchctl','bootstrap',domain,plist)
        child=launch_health(j,challenge)
        deadline=time.monotonic()+110
        while time.monotonic() < deadline:
            if RECEIPT.exists():
                break
            if child.poll() not in (None,0):
                raise ValueError('installation-health GUI failed; use recover or finalize')
            time.sleep(0.25)
        # Do not kill other GUI processes. This particular health probe has its own
        # deadline; terminate only our child if it overstays after the bounded wait.
        if child.poll() is None:
            try:
                child.wait(timeout=max(0.1,min(2,deadline-time.monotonic())))
            except subprocess.TimeoutExpired:
                child.terminate()
        if not RECEIPT.exists():
            raise ValueError('health timed out; transaction retained for finalize or recover')
    challenge=private_json(CHALLENGE,0o644)
    receipt=valid_receipt(challenge,record)
    processes=verify_processes(owner)
    verify_components(j)
    return finish(j,'committed',dict(receipt=receipt,processes=processes))


def verify(owner):
    if not JOURNAL.exists():
        result=private_json(LAST_RESULT,0o600)
        if result['owner_uid'] != owner:
            raise ValueError('console owner differs from completed transaction')
        stage=Path(result['stage'])
        if stage.parent!=SYSTEM or not re.fullmatch('source-local-[a-f0-9]{32}',stage.name):
            raise ValueError('unsafe completed transaction stage')
        archived=private_json(stage/'result.json',0o600)
        if result['status']=='committed':
            verify_components(archived)
            result['processes']=verify_processes(owner)
        elif result['status']=='uninstalled':
            verify_uninstalled(archived['plan'])
        result['evidence']=str(stage/'result.json')
        return result
    j=load_journal()
    if j['plan']['owner_uid'] != owner:
        raise ValueError('console owner differs from journal')
    if j['pending']=='health' and j['plan']['operation']!='uninstall':
        verify_components(j)
        return dict(status='pending_authenticated_health',processes=verify_processes(owner))
    return dict(status='pending_recovery',phase=j['pending'])


def verify_public(owner, edition):
    record = private_json(RECORD, 0o644)
    if record['owner_uid'] != owner or record['edition'] != edition:
        raise ValueError('installation owner/edition differs')
    observed = installed_state(edition)
    if any(value is None for value in observed.values()):
        raise ValueError('installed component missing')
    if any(value['uid'] != 0 or value['mode'] & 0o022 for value in observed.values()):
        raise ValueError('installed component permissions differ')
    for key, pin in [('app','gui_cdhash'),('helper','helper_cdhash'),('guardian','guardian_cdhash')]:
        if observed[key]['cdhash'] != record['trust'][pin]:
            raise ValueError('installed component identity differs')
    watchdog_verify(dict(watchdog={}, plan=dict(timezone_target=read_timezone_target(owner))))
    watchdog_verify(dict(bodyguard={}, plan=dict(proxy_mode=read_proxy_mode(owner))), 'bodyguard')
    cli_verify(dict(plan=dict(owner_uid=owner,edition=edition,cli_entry=cli_snapshot(owner))))
    return dict(status='components_verified', processes=verify_processes(owner),
                timezone_watchdog='registered', authenticated_health='not_run')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=['plan','install','replace','uninstall','recover','verify','finalize','commit'])
    parser.add_argument('--edition', choices=['personal','public'], default='personal')
    parser.add_argument('--app', type=Path)
    parser.add_argument('--digest')
    parser.add_argument('--uninstall-plan', action='store_true')
    args = parser.parse_args()
    owner = console_owner()
    if args.operation == 'plan':
        p = plan(args.edition, args.app, owner, 'uninstall' if args.uninstall_plan else 'install')
        print(json.dumps(dict(plan=p, digest=digest(p),
            notes=['Legacy rollback restores scheduling; RunAtLoad may immediately start its original task.'],
            legacy_running_observations={item['label']:bool(re.search(r'^\s*pid = \d+',
                run('/bin/launchctl','print',item['domain']+'/'+item['label'],check=False).stdout,re.M))
                for item in p['legacy']}), indent=2)); return
    if args.operation == 'verify':
        print(json.dumps(verify_public(owner, args.edition), indent=2)); return
    if os.geteuid() != 0:
        raise ValueError('explicit administrator invocation required')
    p = None
    if args.operation in ('install','replace','uninstall'):
        # First-run discovery must fail before creating locks or stopping apps.
        p = plan(args.edition, args.app, owner, 'uninstall' if args.operation == 'uninstall' else 'install')
        if args.digest is not None and args.digest != digest(p):
            raise ValueError('plan digest changed')
    previous_owner = p.get('previous_owner_uid') if p else None
    if p is None and JOURNAL.exists():
        previous_owner = load_journal()['plan'].get('previous_owner_uid')
    if args.operation in ('install','replace','uninstall','recover'):
        for uid in sorted({owner, previous_owner} - {None}):
            stopped(uid)
    with locks(owner, previous_owner):
        if args.operation == 'recover':
            result = recover(owner)
        elif args.operation in ('finalize','commit'):
            result = finalize(owner,launch=args.operation=='finalize')
        else:
            if args.operation == 'replace':
                # Preview is identical; authorization to replace is the explicit command.
                expected = digest(p)
                if args.digest is not None and args.digest != expected:
                    raise ValueError('plan digest changed')
                p['operation'] = 'replace'
                result = transact(p, digest(p))
            else:
                result = transact(p, args.digest or digest(p))
            result = finalize(owner,launch=p['operation']!='uninstall')
        print(json.dumps(result, indent=2))


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        raise SystemExit(str(error))
