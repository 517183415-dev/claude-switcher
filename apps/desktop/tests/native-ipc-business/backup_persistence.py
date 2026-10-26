"""Restore synthetic backup, exit, then read restored data through production IPC."""
import argparse
import base64
import hashlib
import json
import sys

import backup
import draft_persistence
import runner
from support import inputs, output_root


def validate_read(response, before, after, proof):
    assert isinstance(response, dict) and set(response) == {'draft', 'preview'}, 'read response schema'
    assert before == after, 'read process modified publisher tree'
    files = after['files']
    base = 'v2/personal/publisher/'
    generation = proof['new_generation']
    state = json.loads(files[base + generation + '/state.json'])
    assert response['draft'] == state['draft'], 'production draft differs from disk'
    campaigns = [c for slot in state['draft']['slots'] for c in slot['campaigns']]
    assert len(campaigns) == 1 and campaigns[0]['image']['sha256'] == proof['asset_sha256'], 'restored image reference mismatch'
    image = files[base + generation + '/assets/' + proof['asset_sha256']]
    assert hashlib.sha256(image).hexdigest() == proof['asset_sha256'], 'restored image digest mismatch'
    prefix = 'data:image/png;base64,'
    assert isinstance(response['preview'], str) and response['preview'].startswith(prefix), 'preview format mismatch'
    assert base64.b64decode(response['preview'][len(prefix):], validate=True) == image, 'preview bytes differ from disk'
    source_trust = files['v2/personal/backup-fixture/source/trust.json']
    assert json.loads(files[base + 'trust.json']) == json.loads(source_trust), 'restored trust differs from source'


def run(attempt, output):
    runner.OUT = output_root(str(output))
    runner.attempt_value(attempt)
    folder = runner.new_run('backup-persistence')
    root = folder / 'tmp/business-root'
    evidence = dict(status='RUNNING', fixture_app_launch='NOT_RUN', phases=[], root=str(root))
    try:
        binding = runner.build_binding('personal', attempt)
        current = inputs()
        assert binding[3] == current, 'source binding mismatch'
        profile_hash = runner.sha(folder / 'runtime.sb')
        evidence.update(source_hashes=current, profile_sha256=profile_hash,
                        binary_sha256=runner.sha(binding[2]), receipt_sha256=runner.sha(binding[0]))
        pids = set()
        proof = None
        for phase in ('restore', 'read'):
            assert runner.prepare(), 'isolation controls failed'
            assert inputs() == current and runner.sha(folder / 'runtime.sb') == profile_hash, 'input/profile changed'
            binary = binding[2]
            assert runner.sha(binary) == binding[1]['signed_sha256'], 'binary changed'
            before = draft_persistence.files(root)
            label = 'personal-' + phase
            execution = runner.execute([binary, '--backup-persistence-' + phase], folder, label=label)
            evidence['fixture_app_launch'] = 'RUN'
            row = dict(phase=phase, execution=execution)
            evidence['phases'].append(row)
            draft_persistence.validate_execution(execution, pids)
            after = draft_persistence.files(root)
            stdout = runner.bounded_text(folder / (label + '-stdout.log'))
            envelope = runner.envelope(stdout, execution['exit'], 'personal', False)
            if phase == 'restore':
                assert before == draft_persistence.empty(), 'restore must start with empty synthetic root'
                proof = backup.validate(stdout, envelope['responses'], 'personal', folder, [], persistent=True)
            else:
                assert set(envelope['responses']) == {'backup_persistence'}, 'unexpected read response'
                validate_read(envelope['responses']['backup_persistence'], before, after, proof)
            row.update(validation='PASS', file_sha256={name: hashlib.sha256(data).hexdigest() for name, data in after['files'].items()})
            assert inputs() == current and runner.sha(binary) == binding[1]['signed_sha256'] and runner.sha(folder / 'runtime.sb') == profile_hash, 'input changed during phase'
        assert len(pids) == 2, 'separate processes required'
        evidence.update(status='PASS', independent_disk='private bounded tree and byte-for-byte read-only snapshot',
                        production_read_handlers=['ad_draft_read', 'ad_image_preview'],
                        restore='shared restore body with synthetic MemoryVault',
                        original_backup_handler='NOT_RUN', real_keychain='NOT_ACCESSED',
                        vault_cross_process='NOT_TESTED')
    except (OSError, ValueError, KeyError, TypeError, AssertionError) as error:
        evidence.update(status='BLOCKED_OR_FAILED', error=str(error))
    runner.write(folder / 'result.json', evidence)
    print(folder)
    return evidence['status'] == 'PASS'


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=output_root)
    parser.add_argument('--build-attempt', required=True, type=runner.attempt_value)
    args = parser.parse_args()
    sys.exit(0 if run(args.build_attempt, args.output) else 1)
