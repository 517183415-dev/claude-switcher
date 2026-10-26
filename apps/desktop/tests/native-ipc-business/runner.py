#!/usr/bin/env python3
"""Isolated real activity-command IPC fixture. No ordinary startup or external network."""
import argparse
import errno
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import stat
import subprocess
import sys
import time
import uuid
import pwd

HERE = Path(__file__).resolve().parent
from support import ROOT, inputs, output_root, make_profile, private_directory, safe_file
OUT = None
DENIALS = ['ipc_business_forbidden', 'workspace_snapshot', 'account_action', 'software_update', 'open_date_settings']
EXPECTED_MESSAGE = '正常业务\nIP [网络地址已隐藏]\nIPv6 [网络地址已隐藏]\n[网络地址已隐藏]\n[敏感字段已隐藏]\n路径 [本机目录]/synthetic-profile'
RAW_PREFIX = '正常业务\nIP 203.0.113.9\nIPv6 2001:db8::1\nhttps://private.example/path\nCookie: BUSINESS_SYNTHETIC_ONLY\n路径 '


def sha(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def write(path, value):
    with Path(path).open('x') as handle:
        json.dump(value, handle, ensure_ascii=False, indent=2)
        handle.write('\n')


def bounded_text(path, limit=1024 * 1024):
    with Path(path).open('rb') as handle:
        content = handle.read(limit + 1)
    assert len(content) <= limit, 'evidence exceeds bounded read'
    return content.decode('utf-8')


def new_run(label):
    assert OUT is not None, 'explicit --output required'
    folder = OUT / ('run-' + time.strftime('%Y%m%d-%H%M%S') + '-' + label + '-' + uuid.uuid4().hex[:8])
    folder.mkdir(mode=0o700)
    for name in ['home', 'tmp']:
        (folder / name).mkdir(mode=0o700)
    (folder / 'tmp/business-root').mkdir(mode=0o700)
    make_profile(folder, OUT, build=False)
    return folder


def environment(folder):
    return {'HOME': str(folder / 'home'), 'TMPDIR': str(folder / 'tmp') + '/',
            'IPC_BUSINESS_ROOT': str(folder / 'tmp/business-root'),
            'PATH': '/usr/bin:/bin:/usr/sbin:/sbin', 'LANG': 'en_US.UTF-8',
            'LC_ALL': 'en_US.UTF-8', 'USER': 'ipc-fixture', 'LOGNAME': 'ipc-fixture',
            'PYTHONDONTWRITEBYTECODE': '1'}


def execute(argv, folder, timeout=120, label=None):
    profile = folder / 'runtime.sb'
    command = ['/usr/bin/sandbox-exec', '-f', str(profile), *map(str, argv)]
    if label is not None and re.fullmatch(r'[a-z0-9-]+', label) is None:
        raise ValueError('invalid process log label')
    stdout = folder / ((label + '-' if label else '') + 'stdout.log')
    stderr = folder / ((label + '-' if label else '') + 'stderr.log')
    start = time.monotonic()
    with stdout.open('xb') as out, stderr.open('xb') as err:
        process = subprocess.Popen(command, cwd=folder, env=environment(folder),
                                   stdin=subprocess.DEVNULL, stdout=out, stderr=err,
                                   start_new_session=True)
        timed_out = False
        try:
            code = process.wait(timeout=min(timeout, 120))
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGKILL)
            code = process.wait(timeout=10)
    return {'command': command, 'exit': code, 'timeout': timed_out,
            'termination_scope': 'fixture process group only; separately managed WebKit processes not proven terminated',
            'seconds': time.monotonic() - start, 'environment': environment(folder), 'pid': process.pid,
            'stdout_sha256': sha(stdout), 'stderr_sha256': sha(stderr)}


def private(path, mode, directory=False):
    metadata = path.lstat()
    assert not stat.S_ISLNK(metadata.st_mode), 'symlink forbidden'
    assert (stat.S_ISDIR(metadata.st_mode) if directory else stat.S_ISREG(metadata.st_mode)), 'unexpected file kind'
    assert stat.S_IMODE(metadata.st_mode) == mode, 'private mode mismatch'
    assert metadata.st_uid == os.geteuid(), 'fixture ownership mismatch'
    if not directory:
        assert metadata.st_nlink == 1, 'hardlinked evidence forbidden'
    return {'path': str(path), 'mode': oct(mode), 'uid': metadata.st_uid, 'nlink': metadata.st_nlink}


def preference_cases():
    """Independent protocol oracle; do not import the JavaScript matrix."""
    def pref(slots, ack, paused=None):
        return {'ads': {'slots_enabled': slots, 'request_purpose_acknowledged': ack}, 'updates_paused_until': paused}
    off = pref([False, False, False], False)
    cases = [('all_off_unconfirmed', {'preferences': off}, True)]
    for slot in range(3):
        slots = [False, False, False]; slots[slot] = True
        cases.append(('unconfirmed_' + str(slot), {'preferences': pref(slots, False)}, False))
    for slot in range(3):
        slots = [False, False, False]; slots[slot] = True
        cases.append(('enable_' + str(slot), {'preferences': pref(slots, True)}, True))
    cases.append(('all_off_again', {'preferences': off}, True))
    malformed = [
        ('unknown_outer', dict(off, unknown=True)),
        ('unknown_ads', dict(off, ads=dict(off['ads'], unknown=True))),
        ('missing_ads', {'updates_paused_until': None}),
        ('missing_slots', {'ads': {'request_purpose_acknowledged': False}, 'updates_paused_until': None}),
        ('missing_ack', {'ads': {'slots_enabled': [False, False, False]}, 'updates_paused_until': None}),
        ('short_slots', pref([False, False], False)),
        ('long_slots', pref([False, False, False, False], False)),
        ('wrong_preferences_type', []), ('wrong_ads_type', dict(off, ads=[])),
        ('wrong_slots_type', pref('false', False)), ('wrong_slot_type', pref([False, 1, False], False)),
        ('wrong_ack_type', pref([False, False, False], 'false')),
        ('wrong_updates_type', pref([False, False, False], False, -1)),
    ]
    for index, (name, value) in enumerate(malformed):
        cases.append((name, {'preferences': value}, False))
        if index == 1:
            cases.append(('missing_argument', {}, False))
    return cases


def validate_preferences(stdout, responses, edition, folder, filesystem):
    cases = preference_cases()
    assert set(responses) == {'preferences'}, 'preferences response schema mismatch'
    steps = responses['preferences']
    assert isinstance(steps, list) and len(steps) == len(cases), 'preferences matrix incomplete'
    prefix = 'BUSINESS_IPC_PREFERENCES '
    snapshots = [json.loads(line[len(prefix):]) for line in stdout.splitlines() if line.startswith(prefix)]
    assert len(snapshots) == 2 * len(cases), 'preferences snapshots incomplete'
    def decode(snapshot, label):
        assert set(snapshot) == {'label', 'preferences', 'activity'} and snapshot['label'] == label, 'snapshot schema/order mismatch'
        result = {}
        for name in ('preferences', 'activity'):
            value = snapshot[name]
            if value is None:
                result[name] = None
            else:
                assert isinstance(value, list) and len(value) <= 65536 and all(type(item) is int and 0 <= item <= 255 for item in value), 'invalid bounded byte snapshot'
                result[name] = bytes(value)
        return result
    same_json = lambda left, right: json.dumps(left, sort_keys=True) == json.dumps(right, sort_keys=True)
    previous = None
    successes = 0
    for index, ((name, request, accepted), step) in enumerate(zip(cases, steps)):
        assert set(step) == {'id', 'input', 'accepted', 'value', 'error'}, 'preference response fields mismatch'
        assert step['id'] == name and same_json(step['input'], request) and step['accepted'] is accepted, 'independent preference matrix mismatch'
        before = decode(snapshots[2 * index], name + ':before')
        after = decode(snapshots[2 * index + 1], name + ':after')
        if previous is None:
            assert before['preferences'] is None, 'fixture preferences must begin absent'
            initial = json.loads(before['activity'])
            assert len(initial) == 2 and initial[0]['message'] == EXPECTED_MESSAGE, 'initial activity control missing'
            assert initial[1]['message'] == RAW_PREFIX + str(folder / 'home') + '/synthetic-profile', 'initial raw positive control missing'
        else:
            assert before == previous, 'unobserved mutation between preference calls'
        if accepted:
            successes += 1
            assert step['error'] is None and same_json(step['value'], request['preferences']), 'preference return differs from independent request'
            assert same_json(json.loads(after['preferences']), request['preferences']), 'saved preferences differ from real IPC return'
            old_log, new_log = json.loads(before['activity']), json.loads(after['activity'])
            if successes == 1:
                old_log[1]['message'] = EXPECTED_MESSAGE
            assert len(new_log) == len(old_log) + 1 and new_log[:-1] == old_log, 'accepted preference activity append mismatch'
            entry = new_log[-1]
            assert set(entry) == {'observed_at', 'level', 'message'} and type(entry['observed_at']) is int and entry['observed_at'] > 0
            assert entry['level'] == 'info' and entry['message'] == '已保存本机设置；全部广告关闭时不发送广告请求', 'preference activity message mismatch'
        else:
            assert step['value'] is None and isinstance(step['error'], str) and step['error'], 'rejected preference did not return error'
            if name.startswith('unconfirmed_'):
                assert step['error'] == '请先确认广告请求用途，再开启广告位', 'business rejection mismatch'
            else:
                assert 'save_preferences' in step['error'], 'invalid DTO did not reach real command decoder'
            assert before == after, 'rejected preference changed preferences or activity bytes'
        previous = after
    directory = folder / 'tmp/business-root/v2' / edition
    for path in (directory.parent, directory):
        filesystem.append(private(path, 0o700, True))
    for name, snapshot in previous.items():
        path = directory / (name + '.json')
        filesystem.append(private(path, 0o600))
        with path.open('rb') as handle:
            content = handle.read(65537)
        assert len(content) <= 65536 and content == snapshot, 'final preference snapshot differs from independent disk read'
    assert set(path.name for path in directory.parent.iterdir()) == {edition}, 'edition directories escaped isolation'
    assert set(path.name for path in directory.iterdir()) == {'preferences.json', 'activity.json'}, 'unexpected preferences side effects'
    return {'status': 'PASS', 'edition': edition, 'mode': 'preferences', 'accepted': successes,
            'rejected': len(cases) - successes, 'snapshot_count': len(snapshots), 'filesystem': filesystem,
            'preferences_sha256': sha(directory / 'preferences.json'), 'activity_sha256': sha(directory / 'activity.json'),
            'rejected_bytes_unchanged': True, 'edition_isolation': True, 'ordinary_startup': 'NOT_RUN'}


def envelope(stdout, code, edition, preflight):
    assert code == 0, 'fixture exit must be zero'
    assert len(stdout.encode()) <= 1024 * 1024, 'stdout exceeds bound'
    results = [json.loads(line[len('BUSINESS_IPC_RESULT '):]) for line in stdout.splitlines()
               if line.startswith('BUSINESS_IPC_RESULT ')]
    assert len(results) == 1, 'exactly one result required'
    result = results[0]
    assert set(result) == {'fixture', 'ordinary_runtime', 'edition', 'preflight', 'ok', 'responses', 'denials', 'error'}, 'result schema mismatch'
    assert result['fixture'] == 'ipc-business' and result['ordinary_runtime'] is False
    assert result['edition'] == edition and result['preflight'] is preflight
    assert result['ok'] is True and result['error'] is None, 'JS failure'
    denials = result['denials']
    assert isinstance(denials, list) and len(denials) == len(DENIALS), 'denial count mismatch'
    assert [item.get('command') for item in denials] == DENIALS, 'denial commands mismatch'
    for item in denials:
        assert set(item) == {'command', 'kind'}, 'denial schema mismatch'
        allowed = ['permission'] if item['command'] == 'ipc_business_forbidden' else ['permission', 'missing']
        assert item['kind'] in allowed, 'denial classification mismatch'
    return result


def validate(stdout, code, edition, mode, folder):
    result = envelope(stdout, code, edition, mode == 'preflight')
    denials = result['denials']
    root = folder / 'tmp/business-root'
    filesystem = [private(root, 0o700, True)]
    responses = result['responses']
    if mode == 'preflight':
        assert responses == {}, 'preflight must not call business commands'
        assert list(root.iterdir()) == [], 'preflight business writes forbidden'
        return {'status': 'PASS', 'edition': edition, 'mode': mode, 'denials': denials, 'filesystem': filesystem}
    if mode == 'images':
        import images
        return dict(images.validate(stdout, responses, edition, folder, filesystem), denials=denials)
    if mode == 'draft':
        import draft
        return dict(draft.validate(stdout, responses, edition, folder, filesystem), denials=denials)
    if mode == 'backup':
        import backup
        return dict(backup.validate(stdout, responses, edition, folder, filesystem), denials=denials)
    if mode == 'preferences':
        return dict(validate_preferences(stdout, responses, edition, folder, filesystem), denials=denials)
    assert set(responses) == {'preview', 'exported', 'after'}, 'business response schema mismatch'
    preview = responses['preview']
    assert isinstance(preview, list) and len(preview) == 2, 'two activity entries required'
    for i, entry in enumerate(preview):
        assert set(entry) == {'observed_at', 'level', 'message'}, 'activity DTO schema mismatch'
        assert type(entry['observed_at']) is int and entry['observed_at'] > 0, 'invalid activity timestamp'
        assert entry['level'] == ['info', 'warning'][i], 'activity level mismatch'
        assert entry['message'] == EXPECTED_MESSAGE, 'independent redaction oracle mismatch'
    assert preview[1]['observed_at'] == 1, 'historical timestamp changed'
    exported = responses['exported']
    assert isinstance(exported, list) and len(exported) == 2, 'export tuple required'
    directory = root / 'v2' / edition
    path = directory / 'exports/activity-redacted.json'
    assert exported[0] == str(path), 'export path must be fixed per edition'
    assert exported[1] == preview == responses['after'], 'preview/export/after mismatch'
    for directory_path in [root / 'v2', directory, path.parent]:
        filesystem.append(private(directory_path, 0o700, True))
    for file_path in [directory / 'activity.json', path]:
        filesystem.append(private(file_path, 0o600))
    disk_export = json.loads(bounded_text(path, 65536))
    assert disk_export == preview, 'disk export differs from actual IPC snapshot'
    activity = json.loads(bounded_text(directory / 'activity.json', 65536))
    assert len(activity) == 2 and activity[0] == preview[0], 'normal persistence was not redacted'
    historical = dict(preview[1], message=RAW_PREFIX + str(folder / 'home') + '/synthetic-profile')
    assert activity[1] == historical, 'historical raw positive control missing or mutated'
    # Export path is a synthetic fixed path, so inspect message data independently of that path.
    messages = '\n'.join(entry['message'] for entry in preview + disk_export)
    for secret in ['203.0.113.9', '2001:db8::1', 'private.example', 'BUSINESS_SYNTHETIC_ONLY', str(folder / 'home')]:
        assert secret not in messages, 'raw synthetic marker escaped redaction'
    return {'status': 'PASS', 'edition': edition, 'mode': mode, 'denials': denials,
            'filesystem': filesystem, 'responses': responses,
            'export_sha256': sha(path), 'activity_sha256': sha(directory / 'activity.json'),
            'historical_raw_positive_control': True, 'ordinary_startup': 'NOT_RUN'}


NEGATIVE = r'''
import errno,json,os,socket,subprocess
results={}
try:
 fd=os.open(PRIVATE_LIBRARY,os.O_RDONLY|os.O_DIRECTORY)
 os.close(fd);results['private_file']={'unexpected_success':True}
except OSError as e:results['private_file']={'errno':e.errno}
try:
 subprocess.run(['/usr/bin/true'],check=True)
 results['exec']={'unexpected_success':True}
except OSError as e:results['exec']={'errno':e.errno}
for name,address,bind in [('public_network',('198.51.100.1',443),False),('localhost',('127.0.0.1',0),True)]:
 try:
  with socket.socket() as s:
   s.settimeout(2)
   if bind:s.bind(address)
   else:s.connect(address)
  results[name]={'unexpected_success':True}
 except OSError as e:results[name]={'errno':e.errno}
with open(os.path.join(os.environ['TMPDIR'],'allowed-marker'),'x') as f:f.write('synthetic')
results['fixture_write']=True
print(json.dumps(results))
'''


def prepare():
    folder = new_run('controls')
    execution = execute([sys.executable, '-I', '-c', 'PRIVATE_LIBRARY=' + repr(str(Path(pwd.getpwuid(os.geteuid()).pw_dir) / 'Library')) + '\n' + NEGATIVE], folder, 20)
    result = {'execution': execution, 'profile_sha256': sha(folder / 'runtime.sb'), 'runner_sha256': sha(__file__), 'fixture_app_launch': 'NOT_RUN'}
    try:
        controls = json.loads(bounded_text(folder / 'stdout.log'))
        result['controls'] = controls
        assert execution['exit'] == 0 and not execution['timeout']
        assert controls['fixture_write'] is True
        for name in ['private_file', 'exec', 'public_network', 'localhost']:
            assert controls[name].get('errno') in [errno.EPERM, errno.EACCES], (name, controls[name])
        result['status'] = 'PASS'
    except (ValueError, KeyError, AssertionError) as error:
        result.update(status='BLOCKED', error=str(error))
    write(folder / 'result.json', result)
    print(folder)
    return result['status'] == 'PASS'


def build_binding(edition, attempt):
    if edition not in ('personal', 'public'):
        raise ValueError('unsupported edition')
    attempt_value(attempt)
    receipt = OUT / 'build' / (edition + '-' + attempt) / 'result.json'
    private_directory(OUT / 'build')
    private_directory(receipt.parent)
    safe_file(receipt)
    data = json.loads(bounded_text(receipt))
    assert data['status'] == 'PASS_BUILD_AND_ADHOC_STATIC_VERIFY' and data['edition'] == edition
    binary_input = safe_file(Path(data['binary']))
    assert not binary_input.is_symlink(), 'binary symlink forbidden'
    binary = binary_input.resolve(strict=True)
    assert binary.is_file() and binary.is_relative_to(OUT / 'build')
    assert sha(binary) == data['signed_sha256'], 'binary hash mismatch'
    assert data['source_before'] == data['source_after'], 'build inputs changed'
    assert Path(data['sandbox']) == receipt.parent / 'build.sb', 'build sandbox must use fixed receipt path'
    assert sha(safe_file(Path(data['sandbox']))) == data['profile_sha256'], 'build sandbox changed'
    current = inputs()
    assert current == data['source_before'], 'current source differs from successful build inputs'
    assert data['harness_hashes'] == {name: current[name] for name in current if name.startswith('apps/desktop/tests/native-ipc-business/')}, 'harness differs from build'
    return receipt, data, binary, current


def run(edition, mode, attempt):
    folder = new_run(edition + '-' + mode)
    receipt = OUT / 'build' / (edition + '-' + attempt) / 'result.json'
    evidence = {'edition': edition, 'mode': mode, 'build_attempt': attempt, 'fixture_app_launch': 'NOT_RUN'}
    try:
        receipt, data, binary, current = build_binding(edition, attempt)
        evidence.update(source_hashes=current, binary={'path': str(binary), 'sha256': sha(binary)},
                        receipt_sha256=sha(receipt), profile_sha256=sha(folder / 'runtime.sb'), runner_sha256=sha(__file__))
        assert prepare(), 'sandbox controls failed'
        execution = execute([binary, '--' + mode], folder)
        evidence.update(execution=execution, fixture_app_launch='RUN')
        assert not execution['timeout'], '120-second timeout'
        evidence['validation'] = validate(bounded_text(folder / 'stdout.log'), execution['exit'], edition, mode, folder)
        assert inputs() == current, 'source changed during runtime'
        assert sha(binary) == data['signed_sha256'], 'binary changed during runtime'
        evidence['status'] = 'PASS'
    except (OSError, ValueError, KeyError, AssertionError) as error:
        evidence.update(status='BLOCKED_OR_FAILED', error=str(error))
    write(folder / 'result.json', evidence)
    print(folder)
    return evidence['status'] == 'PASS'


def attempt_value(value):
    if re.fullmatch(r'r[0-9]+', value) is None:
        raise argparse.ArgumentTypeError('attempt must match r[0-9]+')
    return value


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['prepare', 'run', 'draft-persistence'])
    parser.add_argument('--output', required=True, type=output_root)
    parser.add_argument('--edition', choices=['personal', 'public'])
    parser.add_argument('--mode', choices=['preflight', 'business', 'preferences', 'backup', 'draft', 'images'])
    parser.add_argument('--build-attempt', type=attempt_value)
    args = parser.parse_args(argv)
    if args.action == 'run' and (not args.edition or not args.mode or args.build_attempt is None):
        parser.error('run requires --edition, --mode and --build-attempt')
    if args.action == 'draft-persistence' and (args.build_attempt is None or args.edition is not None or args.mode is not None):
        parser.error('draft-persistence requires --build-attempt and determines its own editions/phases')
    return args


if __name__ == '__main__':
    args = parse_args()
    OUT = args.output
    if args.action == 'draft-persistence':
        import draft_persistence
        sys.exit(0 if draft_persistence.run(args.build_attempt, output=OUT) else 1)
    sys.exit(0 if (prepare() if args.action == 'prepare' else run(args.edition, args.mode, args.build_attempt)) else 1)
