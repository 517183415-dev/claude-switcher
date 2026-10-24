#!/usr/bin/env python3
"""Build signed, owner-bound macOS component and recovery packages. Never install."""
import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
SYSTEM = Path('Library/Application Support/claude-switcher')
HELPER = Path('Library/PrivilegedHelperTools/local.claudeswitcher.helper')
DAEMON = Path('Library/LaunchDaemons/local.claudeswitcher.helper.plist')
AGENT = Path('Library/LaunchAgents/local.claudeswitcher.guardian.plist')
GUARDIAN = SYSTEM / 'switcher-guardian'
RECORD = SYSTEM / 'install.json'
FILES = [HELPER, DAEMON, AGENT, GUARDIAN, RECORD]


def parser():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--team', required=True)
    p.add_argument('--application-identity', required=True)
    p.add_argument('--installer-identity', required=True)
    p.add_argument('--owner-uid', required=True, type=int)
    p.add_argument('--edition', required=True, choices=['personal', 'public'])
    p.add_argument('--version', default='0.2.0')
    p.add_argument('--minimum-app-version', help='Oldest signed GUI permitted by this helper; defaults to version')
    p.add_argument('--operation', choices=['install', 'uninstall'], default='install')
    p.add_argument('--helper', type=Path, required=True)
    p.add_argument('--guardian', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--plan', action='store_true', help='Print a plan without signing or building')
    return p


def run(argv):
    subprocess.run([str(v) for v in argv], check=True)


def scripts(owner, team, restore=False, uninstall=False):
    # Paths are compile-time constants. The root-owned backup never contains caller paths.
    prolog = '''#!/bin/sh
set -eu
[ "$(/usr/bin/id -u)" = 0 ] || exit 70
[ "$3" = / ] || exit 71
base='/Library/Application Support/claude-switcher'
for p in '/Library' '/Library/Application Support' "$base"; do
  [ ! -L "$p" ] || exit 72
  if [ -e "$p" ]; then
    [ "$(/usr/bin/stat -f %u "$p")" = 0 ] || exit 73
    /usr/bin/find "$p" -maxdepth 0 -perm +022 | /usr/bin/grep -q . && exit 74
  fi
done
/bin/mkdir -p "$base"
/usr/sbin/chown root:wheel "$base"
/bin/chmod 755 "$base"
backup="$base/component-backup"
[ -f "$base/component-installer-active" ] && [ ! -L "$base/component-installer-active" ] || exit 84
[ "$(/usr/bin/stat -f %u "$base/component-installer-active")" = 0 ] || exit 85
[ "$(/bin/cat "$base/component-installer-active")" = "$(/usr/sbin/sysctl -n kern.bootsessionuuid)" ] || exit 86
'''
    # The package targets only the active, OS-reported user. No environment UID is accepted.
    owner_check = f'[ "$(/usr/bin/stat -f %u /dev/console)" = {owner} ] || exit 75\n'
    stop = '''/bin/launchctl bootout system/local.claudeswitcher.helper 2>/dev/null || true
/bin/launchctl bootout "gui/OWNER/local.claudeswitcher.guardian" 2>/dev/null || true
'''.replace('OWNER', str(owner))
    paths = '\n'.join('/' + str(path) for path in FILES)
    if restore:
        body = '''[ -d "$backup" ] && [ ! -L "$backup" ] || exit 76
[ "$(/usr/bin/stat -f %u "$backup")" = 0 ] || exit 77
'''+stop+f'''while IFS= read -r path; do
  name=$(/usr/bin/basename "$path")
  if [ -f "$backup/$name" ] && [ ! -L "$backup/$name" ]; then
    /bin/cp -p "$backup/$name" "$path"
  else
    /bin/rm -f "$path"
  fi
done <<'FIXED_PATHS'
{paths}
FIXED_PATHS
'''
        body += '''if [ -f '/Library/PrivilegedHelperTools/local.claudeswitcher.helper' ]; then
/usr/bin/codesign --verify --strict --test-requirement '=anchor apple generic and certificate leaf[subject.OU] = "TEAM" and identifier "local.claudeswitcher.helper"' '/Library/PrivilegedHelperTools/local.claudeswitcher.helper'
/bin/launchctl bootstrap system '/Library/LaunchDaemons/local.claudeswitcher.helper.plist'
/bin/launchctl bootstrap 'gui/OWNER' '/Library/LaunchAgents/local.claudeswitcher.guardian.plist'
fi
'''.replace('TEAM', team).replace('OWNER', str(owner))
        body += f'''while IFS= read -r path; do
  /bin/rm -f "$backup/$(/usr/bin/basename "$path")"
done <<'FIXED_PATHS'
{paths}
FIXED_PATHS
/bin/rmdir "$backup"
/bin/rm -f "$base/component-pending"
'''
        return {'postinstall': prolog + owner_check + body + '\n/bin/rm -f \"$base/component-installer-active\"\n/bin/sync\n'}
    before = prolog + owner_check + '''if [ -e "$backup" ]; then
  [ -f "$base/component-committed.json" ] && [ ! -L "$base/component-committed.json" ] || exit 78
  [ "$(/usr/bin/stat -f %u "$base/component-committed.json")" = 0 ] || exit 78
  [ ! -L "$backup" ] && [ "$(/usr/bin/stat -f %u "$backup")" = 0 ] || exit 78
  for name in local.claudeswitcher.helper local.claudeswitcher.helper.plist local.claudeswitcher.guardian.plist switcher-guardian install.json; do
    /bin/rm -f "$backup/$name"
  done
  /bin/rmdir "$backup"
fi
/bin/rm -f "$base/component-committed.json"
/bin/mkdir "$backup"
/bin/chmod 700 "$backup"
'''+f'''while IFS= read -r path; do
  [ ! -L "$path" ] || exit 79
  if [ -e "$path" ]; then
    [ "$(/usr/bin/stat -f %u "$path")" = 0 ] || exit 80
    /bin/cp -p "$path" "$backup/$(/usr/bin/basename "$path")"
  fi
done <<'FIXED_PATHS'
{paths}
FIXED_PATHS
'''+'''/usr/bin/touch "$base/component-pending"
/bin/chmod 600 "$base/component-pending"
'''+'''/bin/sync
'''+stop
    after = prolog + owner_check + '''/usr/sbin/chown root:wheel '/Library/PrivilegedHelperTools/local.claudeswitcher.helper' "$base/switcher-guardian" "$base/install.json" '/Library/LaunchDaemons/local.claudeswitcher.helper.plist' '/Library/LaunchAgents/local.claudeswitcher.guardian.plist'
/bin/chmod 755 '/Library/PrivilegedHelperTools/local.claudeswitcher.helper' "$base/switcher-guardian"
/bin/chmod 644 "$base/install.json" '/Library/LaunchDaemons/local.claudeswitcher.helper.plist' '/Library/LaunchAgents/local.claudeswitcher.guardian.plist'
/bin/launchctl bootstrap system '/Library/LaunchDaemons/local.claudeswitcher.helper.plist'
/bin/launchctl bootstrap 'gui/OWNER' '/Library/LaunchAgents/local.claudeswitcher.guardian.plist'
'''.replace('OWNER', str(owner))
    if uninstall:
        after = prolog + owner_check + f'''[ ! -e "$base/repair-journal.json" ] || exit 81
[ ! -e "$base/system-original.json" ] || exit 82
while IFS= read -r path; do
  [ ! -L "$path" ] || exit 83
  /bin/rm -f "$path"
done <<'FIXED_PATHS'
{paths}
FIXED_PATHS
'''
    return {'preinstall': before, 'postinstall': after + '\n/bin/rm -f \"$base/component-installer-active\"\n/bin/sync\n'}


def main():
    args = parser().parse_args()
    if not re.fullmatch(r'[A-Z0-9]{10}', args.team) or args.owner_uid <= 0:
        raise SystemExit('invalid team or owner UID')
    args.minimum_app_version = args.minimum_app_version or args.version
    if not re.fullmatch(r'\d+\.\d+\.\d+', args.minimum_app_version):
        raise SystemExit('invalid minimum app version')
    if not re.fullmatch(r'\d+\.\d+\.\d+', args.version):
        raise SystemExit('version must contain three numeric components')
    for identity, prefix in [(args.application_identity, 'Developer ID Application:'), (args.installer_identity, 'Developer ID Installer:')]:
        if not identity.startswith(prefix) or not identity.endswith(f'({args.team})'):
            raise SystemExit('identity must match the pinned team and Developer ID role')
    plan = {'operation': 'build_only', 'system_modified': False, 'owner_uid': args.owner_uid,
            'edition': args.edition, 'version': args.version, 'minimum_app_version': args.minimum_app_version, 'team': args.team, 'intent': args.operation,
            'payload_paths': ['/' + str(p) for p in FILES],
            'outputs': ['components.pkg', 'recovery.pkg'],
            'notarization': 'REQUIRED_BEFORE_INSTALL'}
    if args.plan:
        print(json.dumps(plan, ensure_ascii=False, indent=2))
        return
    identities = subprocess.check_output(['/usr/bin/security', 'find-identity', '-v'], text=True)
    for identity in [args.application_identity, args.installer_identity]:
        if identity not in identities:
            raise SystemExit('signing_identity_unavailable')
    if not args.helper.is_file() or not args.guardian.is_file():
        raise SystemExit('component_binary_missing')
    args.output.mkdir(parents=True, exist_ok=False)
    with tempfile.TemporaryDirectory(prefix='switcher-pkg-') as temp:
        staging = Path(temp)
        payload = staging / 'payload'
        for source, destination in [(args.helper, HELPER), (args.guardian, GUARDIAN),
                                    (ROOT/'launchd/local.claudeswitcher.helper.plist', DAEMON),
                                    (ROOT/'launchd/local.claudeswitcher.guardian.plist', AGENT)]:
            target = payload / destination
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
        for path, identifier in [(HELPER, 'local.claudeswitcher.helper'), (GUARDIAN, 'local.claudeswitcher.guardian')]:
            os.chmod(payload/path, 0o755)
            run(['/usr/bin/codesign', '--force', '--options', 'runtime', '--timestamp', '--sign', args.application_identity, '--identifier', identifier, payload/path])
            run(['/usr/bin/codesign', '--verify', '--strict', payload/path])
        record = dict(schema_version=2, owner_uid=args.owner_uid, edition=args.edition,
                      trust=dict(kind="developer_id", team_id=args.team), helper_version=args.version, minimum_app_version=args.minimum_app_version)
        (payload/RECORD).write_text(json.dumps(record), encoding='utf-8')
        for operation in ['components', 'recovery']:
            script_dir = staging / operation
            script_dir.mkdir()
            for name, text in scripts(args.owner_uid, args.team, operation == 'recovery', args.operation == 'uninstall').items():
                path = script_dir/name
                path.write_text(text, encoding='utf-8')
                path.chmod(0o755)
                run(['/bin/sh', '-n', path])
            command = ['/usr/bin/pkgbuild', '--identifier', f'local.claudeswitcher.{operation}', '--version', args.version,
                       '--scripts', script_dir, '--ownership', 'recommended', '--sign', args.installer_identity]
            command += ['--nopayload'] if operation == 'recovery' or args.operation == 'uninstall' else ['--root', payload, '--install-location', '/']
            run(command + [args.output / f'{operation}.pkg'])
        (args.output/'build-plan.json').write_text(json.dumps(plan, indent=2), encoding='utf-8')
    print('Signed packages built. Notarization and installation are not performed.')


if __name__ == '__main__':
    main()
