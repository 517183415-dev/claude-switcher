"""Three native processes with a private, synthetic publication root."""
import hashlib
import json
import re
import sys
import draft_persistence
import runner
from support import inputs

PHASES = (('personal', 'write'), ('public', 'read'), ('personal', 'resume'), ('personal', 'read'))
COMMANDS = ('ipc_business_publish_init', 'ipc_business_publish', 'publisher_status', 'restore_ad_history',
            'preview_publisher_cleanup', 'commit_publisher_cleanup', 'preview_publisher_gc', 'commit_publisher_gc')


def assert_cleanup_finished(state):
    assert state.get('cleanup') is None, 'cleanup still pending'


def validate_write(value, snapshot):
    assert value['phase'] == 'write'
    assert value['staleError'].find('cleanup_stale_preview') >= 0
    assert value['before'] == value['after'] and value['beforeDisk']['files'] == value['afterDisk']['files']
    assert value['restored']['content_revision'] > value['saved']['content_revision']
    assert value['restored']['release_id'] != value['saved']['release_id']
    assert value['restored']['slots'] == value['saved']['slots']
    assert value['cleanup']['completed'] and value['committed']['completed']
    assert value['gc']['eligible'] == [value['image']['sha256']]
    files = snapshot['files']
    prefix = 'v2/personal/publisher/'
    generation = json.loads(files[prefix + 'active.json'])['generation']
    assert re.fullmatch('[0-9a-f]{32}', generation)
    base = prefix + generation + '/'
    state = json.loads(files[base + 'state.json'])
    assert state['draft'] == value['draft']
    assert_cleanup_finished(state)
    assert all(asset['sha256'] != value['image']['sha256'] for asset in state['assets'])
    assert base + 'assets/' + value['image']['sha256'] not in files
    assert len(value['status']['history']) == len(state['history'])
    for record in state['history']:
        channel = record['manifest']['channel']
        release = record['manifest']['release_id']
        # Locate the fixed local store independently of IPC output.
        matches = [data for name, data in files.items() if name.endswith('/' + channel + '/versions/' + release + '.json')]
        assert len(matches) == 1
        wire = json.loads(matches[0]); payload = json.loads(wire['payload'])
        assert payload == record['manifest'] and len(wire['signature']) == 64
        assert hashlib.sha256(wire['payload'].encode()).hexdigest() == record['payload_hash']
    return value['draft'], value['status']


def run(attempt):
    folder = runner.new_run('publish-persistence'); root = folder / 'tmp/business-root'
    evidence = {'status': 'RUNNING', 'fixture_app_launch': 'NOT_RUN', 'phases': []}
    try:
        bindings = {edition: runner.build_binding(edition, attempt) for edition in ('personal', 'public')}
        current = inputs(); assert all(binding[3] == current for binding in bindings.values())
        profile_hash = runner.sha(folder / 'runtime.sb'); pids = set(); expected = None
        evidence.update(source_hashes=current, profile_sha256=profile_hash,
                        bindings={edition: {'receipt_sha256': runner.sha(binding[0]), 'binary_sha256': runner.sha(binding[2])} for edition, binding in bindings.items()})
        for edition, phase in PHASES:
            assert runner.prepare(), 'sandbox negative controls failed'
            assert inputs() == current and runner.sha(folder / 'runtime.sb') == profile_hash
            before = draft_persistence.files(root); binary = bindings[edition][2]
            label = edition + '-' + phase
            execution = runner.execute([binary, '--publish-' + phase], folder, label=label)
            evidence['fixture_app_launch'] = 'RUN'
            record = {'edition': edition, 'phase': phase, 'execution': execution}; evidence['phases'].append(record)
            draft_persistence.validate_execution(execution, pids)
            after = draft_persistence.files(root)
            result = runner.envelope(runner.bounded_text(folder / (label + '-stdout.log')), execution['exit'], edition, False)
            assert set(result['responses']) == {'publish'}
            value = result['responses']['publish']
            if edition == 'public':
                assert before == after and value['phase'] == 'read'
                assert value['denials'] == [{'command': name, 'kind': 'permission'} for name in COMMANDS]
            elif phase == 'write':
                assert before == draft_persistence.empty()
                expected = validate_write(value, after)
            elif phase == 'resume':
                assert expected == (value['beforeDraft'], value['beforeStatus'])
                assert value['outcome']['completed'] and value['saved']['content_revision'] > value['beforeDraft']['content_revision']
                expected = (value['draft'], value['status'])
            else:
                assert before == after and expected == (value['draft'], value['status'])
            record['validation'] = {'status': 'PASS', 'unchanged': before == after,
                                    'file_sha256': {name: hashlib.sha256(data).hexdigest() for name, data in after['files'].items()}}
            assert inputs() == current and runner.sha(binary) == bindings[edition][1]['signed_sha256']
        evidence.update(status='PASS', ordinary_app='NOT_RUN', real_accounts='NOT_ACCESSED', keychain='NOT_RUN', network='NOT_RUN')
    except (OSError, ValueError, KeyError, TypeError, AssertionError) as error:
        evidence.update(status='BLOCKED_OR_FAILED', error=str(error))
    runner.write(folder / 'result.json', evidence); print(folder)
    return evidence['status'] == 'PASS'


if __name__ == '__main__':
    import argparse
    from support import output_root
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=output_root)
    parser.add_argument('--build-attempt', required=True, type=runner.attempt_value)
    args = parser.parse_args(); runner.OUT = args.output
    sys.exit(0 if run(args.build_attempt) else 1)
