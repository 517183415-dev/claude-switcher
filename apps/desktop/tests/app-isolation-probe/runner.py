#!/usr/bin/env python3
"""Run a narrow parent/child sandbox probe and preserve a bounded receipt."""
import argparse
import hashlib
import importlib.util
import json
import os
import pwd
import signal
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
SUPPORT = HERE.parent / 'native-ipc-business' / 'support.py'
spec = importlib.util.spec_from_file_location('ipc_support', SUPPORT)
support = importlib.util.module_from_spec(spec)
spec.loader.exec_module(support)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_result(value):
    def object_field(obj, name):
        field = obj.get(name)
        if not isinstance(field, dict):
            raise ValueError(f'invalid {name} object')
        return field

    if not isinstance(value, dict) or value.get('direct_child_reaped') is not True:
        raise ValueError('direct child was not proven reaped')
    parent, child = object_field(value, 'parent'), object_field(value, 'child')
    if type(parent.get('pid')) is not int or type(child.get('pid')) is not int:
        raise ValueError('missing process identities')
    if child.get('ppid') != parent['pid'] or child['pid'] == parent['pid']:
        raise ValueError('unexpected child identity')
    securityd_proven = True
    for process in (parent, child):
        if process.get('synthetic_home_roundtrip') is not True:
            raise ValueError('synthetic HOME failed')
        if object_field(process, 'private_directory').get('denied') is not True:
            raise ValueError('private directory denial not demonstrated')
        net = object_field(process, 'network')
        if object_field(net, 'loopback_bind').get('denied') is not True:
            raise ValueError('loopback bind denial not demonstrated')
        outbound = object_field(net, 'outbound')
        if outbound.get('policy_denied') is not True:
            raise ValueError('outbound network policy denial not demonstrated')
        if outbound.get('actual_nonlocal_connection') != 'NOT_RUN':
            raise ValueError('unexpected nonlocal test')
        lookup = object_field(process, 'securityd').get('lookup')
        if lookup == 'unexpected_allowed':
            raise ValueError('securityd lookup unexpectedly allowed')
        if lookup != 'denied':
            securityd_proven = False
    if value.get('webkit_process_isolation') != 'NOT_PROVEN' or value.get('system_managed_process_cleanup') != 'NOT_PROVEN':
        raise ValueError('system managed process boundary must remain unproven')
    return 'DIRECT_PROCESSES_PASS' if securityd_proven else 'PARTIAL_SECURITYD_NOT_PROVEN'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True, help='private output directory under project output/')
    args = parser.parse_args()
    out = support.output_root(args.output)
    if not out.is_relative_to(support.ROOT / 'output'):
        raise ValueError('output must be below project output/')
    run = Path(tempfile.mkdtemp(prefix='direct-process-', dir=out))
    os.chmod(run, 0o700)
    for name in ('home', 'tmp'):
        (run / name).mkdir(mode=0o700)
    source = HERE / 'probe.py'
    copied = run / 'probe.py'
    copied.write_bytes(source.read_bytes())
    profile = support.make_profile(run, out, build=False)
    profile_text = profile.read_text()
    if '(deny network*)' not in profile_text or '.*securityd.*' not in profile_text:
        raise ValueError('profile lacks mandatory deny rules')
    real_home = Path(pwd.getpwuid(os.geteuid()).pw_dir)
    environment = {'HOME': str(run / 'home'), 'TMPDIR': str(run / 'tmp') + '/',
                   'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'LANG': 'C',
                   'PYTHONDONTWRITEBYTECODE': '1', 'PRIVATE_PROBE_PATH': str(real_home / 'Library')}
    command = ['/usr/bin/sandbox-exec', '-f', str(profile), sys.executable, str(copied)]
    result = {'status': 'NOT_PROVEN', 'profile_sha256': digest(profile),
              'probe_sha256': digest(source), 'runner_sha256': digest(Path(__file__)),
              'support_sha256': digest(SUPPORT), 'command': command,
              'real_private_path': 'HOME/Library (open only; no content read)',
              'ordinary_app': 'NOT_RUN', 'real_keychain_api': 'NOT_RUN',
              'nonlocal_connection': 'NOT_RUN', 'webkit_isolation': 'NOT_PROVEN'}
    with (run / 'stdout.log').open('xb') as stdout, (run / 'stderr.log').open('xb') as stderr:
        proc = subprocess.Popen(command, cwd=run, env=environment, stdin=subprocess.DEVNULL,
                                stdout=stdout, stderr=stderr, start_new_session=True)
        result['sandbox_process_pid'] = proc.pid
        try:
            result['exit'] = proc.wait(timeout=30)
            result['timeout'] = False
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid, signal.SIGKILL)
            result['exit'] = proc.wait(timeout=10)
            result['timeout'] = True
    result['stdout_sha256'] = digest(run / 'stdout.log')
    result['stderr_sha256'] = digest(run / 'stderr.log')
    try:
        if result['timeout'] or result['exit'] != 0:
            raise ValueError('sandbox process failed or timed out')
        value = json.loads((run / 'stdout.log').read_text())
        result['observations'] = value
        result['status'] = check_result(value)
    except (ValueError, json.JSONDecodeError) as error:
        result['error'] = str(error)
    (run / 'receipt.json').write_text(json.dumps(result, indent=2, sort_keys=True) + '\n')
    print(json.dumps({'receipt': str(run / 'receipt.json'), 'status': result['status'],
                      'sha256': digest(run / 'receipt.json')}))
    return 0 if result['status'] == 'DIRECT_PROCESSES_PASS' else 1


if __name__ == '__main__':
    sys.exit(main())
