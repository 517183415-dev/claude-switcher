#!/usr/bin/env python3
"""Four separate native fixture processes; one newly created synthetic session root."""
import argparse
import hashlib
import json
import sys
import runner
from support import inputs, output_root


def expected(edition):
    return {'ads': {'slots_enabled': [True, False, True] if edition == 'personal' else [False, True, False],
                    'request_purpose_acknowledged': True},
            'updates_paused_until': 2000000001 if edition == 'personal' else 2000000002}


def files(root):
    runner.private(root, 0o700, True)
    result = {}
    if not list(root.iterdir()):
        return result
    assert set(path.name for path in root.iterdir()) == {'v2'}, 'unexpected shared root side effect'
    runner.private(root / 'v2', 0o700, True)
    for directory in (root / 'v2').iterdir():
        assert directory.name in ('personal', 'public'), 'unknown edition directory'
        runner.private(directory, 0o700, True)
        assert set(path.name for path in directory.iterdir()) == {'preferences.json', 'activity.json'}, 'unexpected edition files'
        for path in directory.iterdir():
            runner.private(path, 0o600)
            with path.open('rb') as stream:
                data = stream.read(65537)
            assert len(data) <= 65536, 'persistent file exceeds read bound'
            result[str(path.relative_to(root))] = data
    return result


def validate_phase(stdout, code, edition, action, before, after):
    result = runner.envelope(stdout, code, edition, False)
    assert result['responses'] == {'persistence': {'action': action, 'value': expected(edition)}}, 'persistence IPC value mismatch'
    # JSON encoding preserves Boolean/number distinctions that Python equality loses.
    assert json.dumps(result['responses']['persistence']['value'], sort_keys=True) == json.dumps(expected(edition), sort_keys=True)
    prefix = 'BUSINESS_IPC_PREFERENCES '
    snapshots = [json.loads(line[len(prefix):]) for line in stdout.splitlines() if line.startswith(prefix)]
    assert len(snapshots) == 2, 'persistence requires exactly two native snapshots'
    for moment, snapshot, disk in zip(('before', 'after'), snapshots, (before, after)):
        assert set(snapshot) == {'label', 'preferences', 'activity'} and snapshot['label'] == 'persistence-' + action + ':' + moment
        for name in ('preferences', 'activity'):
            raw = snapshot[name]
            if raw is not None:
                assert isinstance(raw, list) and len(raw) <= 65536 and all(type(byte) is int and 0 <= byte <= 255 for byte in raw)
                raw = bytes(raw)
            assert raw == disk.get('v2/' + edition + '/' + name + '.json'), 'native snapshot differs from independent disk read'
    own = {'v2/' + edition + '/' + name for name in ('preferences.json', 'activity.json')}
    if action == 'write':
        assert not own.intersection(before), 'writer must start with no edition state'
        assert set(after) == set(before) | own, 'write escaped edition files'
        assert all(after[name] == value for name, value in before.items()), 'writer changed other edition bytes'
        activity = json.loads(after['v2/' + edition + '/activity.json'])
        assert len(activity) == 1 and set(activity[0]) == {'observed_at', 'level', 'message'}
        assert type(activity[0]['observed_at']) is int and activity[0]['observed_at'] > 0
        assert activity[0]['level'] == 'info' and activity[0]['message'] == '已保存本机设置；全部广告关闭时不发送广告请求'
    else:
        assert action == 'read' and before == after, 'reader changed persistent bytes'
    saved = json.loads(after['v2/' + edition + '/preferences.json'])
    assert json.dumps(saved, sort_keys=True) == json.dumps(expected(edition), sort_keys=True), 'persistent preferences mismatch'
    return {'status': 'PASS', 'edition': edition, 'action': action, 'value': saved,
            'other_edition_unchanged': True, 'read_only_bytes_unchanged': action == 'read',
            'denials': result['denials'], 'native_snapshots': len(snapshots)}


def run(attempt):
    folder = runner.new_run('persistence')
    root = folder / 'tmp/business-root'
    evidence = {'status': 'RUNNING', 'fixture_app_launch': 'NOT_RUN', 'phases': [], 'root': str(root)}
    try:
        bindings = {edition: runner.build_binding(edition, attempt) for edition in ('personal', 'public')}
        current = inputs()
        assert all(binding[3] == current for binding in bindings.values())
        evidence['source_hashes'] = current
        evidence['profile_sha256'] = runner.sha(folder / 'runtime.sb')
        evidence['bindings'] = {edition: {'receipt_sha256': runner.sha(binding[0]), 'binary_sha256': runner.sha(binding[2])} for edition, binding in bindings.items()}
        pids = set()
        for edition, action in (('personal', 'write'), ('public', 'write'), ('personal', 'read'), ('public', 'read')):
            assert runner.prepare(), 'isolation controls failed'
            assert inputs() == current, 'source changed before launch'
            assert runner.sha(folder / 'runtime.sb') == evidence['profile_sha256'], 'profile changed'
            binary = bindings[edition][2]
            assert runner.sha(binary) == bindings[edition][1]['signed_sha256'], 'binary changed'
            before = files(root)
            label = edition + '-' + action
            execution = runner.execute([binary, '--persistence-' + action], folder, label=label)
            evidence['fixture_app_launch'] = 'RUN'
            assert not execution['timeout'] and execution['pid'] not in pids, 'fixture timeout or reused process'
            pids.add(execution['pid'])
            after = files(root)
            validation = validate_phase(runner.bounded_text(folder / (label + '-stdout.log')), execution['exit'], edition, action, before, after)
            evidence['phases'].append({'execution': execution, 'validation': validation,
                                       'before_file_sha256': {name: hashlib.sha256(value).hexdigest() for name, value in before.items()},
                                       'file_sha256': {name: hashlib.sha256(value).hexdigest() for name, value in after.items()}})
            assert inputs() == current, 'source changed during runtime'
            assert runner.sha(binary) == bindings[edition][1]['signed_sha256'], 'binary changed during runtime'
            assert runner.sha(folder / 'runtime.sb') == evidence['profile_sha256'], 'profile changed during runtime'
        assert len(pids) == 4 and len(evidence['phases']) == 4
        evidence['status'] = 'PASS'
    except (OSError, ValueError, KeyError, AssertionError) as error:
        evidence.update(status='BLOCKED_OR_FAILED', error=str(error))
    runner.write(folder / 'result.json', evidence)
    print(folder)
    return evidence['status'] == 'PASS'


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=output_root)
    parser.add_argument('--build-attempt', required=True, type=runner.attempt_value)
    args = parser.parse_args()
    runner.OUT = args.output
    sys.exit(0 if run(args.build_attempt) else 1)
