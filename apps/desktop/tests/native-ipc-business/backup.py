"""Accept only bounded non-secret backup receipts and independently verify private disk state."""
import json
import re

CASES = ['wrong_password', 'tampered', 'truncated', 'vault_failure', 'success']
COMMANDS = ['ipc_business_backup_export', 'ipc_business_backup_restore']
ERRORS = ['password_or_integrity', 'password_or_integrity', 'header', 'vault_unavailable', None]
FLAGS = ['old_generation_preserved', 'trust_unchanged', 'old_vault_preserved', 'reopened_credentials_match', 'source_unchanged']
FIELDS = {'case', 'entered', 'accepted', 'error_code', 'old_generation', 'new_generation', 'active_unchanged',
          'vault_items', 'reopened_slots', 'recovery_required', *FLAGS}


def validate_receipts(rows, events):
    assert isinstance(rows, list) and len(rows) == len(CASES), 'five backup cases required'
    assert json.dumps(events, sort_keys=True) == json.dumps(rows, sort_keys=True), 'native/WebView receipt mismatch'
    original = None
    for row, name, error in zip(rows, CASES, ERRORS):
        assert isinstance(row, dict) and set(row) == FIELDS, 'backup receipt schema/non-disclosure mismatch'
        assert row['case'] == name, 'backup order mismatch'
        assert type(row['entered']) is int and row['entered'] == 1, 'real restore must be entered once'
        success = name == 'success'
        assert row['accepted'] is success and row['error_code'] == error, 'crypto failure classification mismatch'
        assert all(row[flag] is True for flag in FLAGS), 'backup preservation/reopen invariant failed'
        assert row['active_unchanged'] is (not success) and row['recovery_required'] is success, 'active/recovery state mismatch'
        assert type(row['vault_items']) is int and row['vault_items'] == (2 if success else 1), 'vault count mismatch'
        for field in ['old_generation', 'new_generation']:
            assert type(row[field]) is str and re.fullmatch('[0-9a-f]{32}', row[field]), 'invalid generation'
        if original is None: original = row['old_generation']
        assert row['old_generation'] == original, 'old generation continuity lost'
        assert (row['new_generation'] != original) is success, 'wrong generation selected'
        expected = [True, False, True] if success else [False, False, False]
        assert json.dumps(row['reopened_slots']) == json.dumps(expected), 'reopened draft differs from expected backup'
    return {'accepted': 1, 'rejected': 4, 'reopened': 5, 'old_generation': original, 'new_generation': rows[-1]['new_generation']}


def validate_public(rows):
    assert rows == [{'command': name, 'kind': 'permission'} for name in COMMANDS], 'Public backup commands must be permission denied'


def validate(stdout, responses, edition, folder, filesystem):
    import runner
    assert set(responses) == {'backup'}, 'backup response schema mismatch'
    prefix = 'BUSINESS_IPC_BACKUP '
    events = [json.loads(line[len(prefix):]) for line in stdout.splitlines() if line.startswith(prefix)]
    root = folder / 'tmp/business-root'
    if edition == 'public':
        validate_public(responses['backup'])
        assert not events and list(root.iterdir()) == [], 'Public backup wrote state or emitted native receipt'
        return {'status': 'PASS', 'edition': edition, 'mode': 'backup', 'commands_acl_denied': COMMANDS, 'runtime_denial_kind': 'permission', 'handler_registration': 'static cfg-gated exclusion; runtime ACL denial does not prove lookup absence', 'no_writes': True}
    proof = validate_receipts(responses['backup'], events)
    # Metadata and the current public publisher state only; no backup or Vault buffers are persisted/read.
    base = root / 'v2/personal/backup-fixture'
    assert set(path.name for path in root.iterdir()) == {'v2'}
    assert set(path.name for path in (root / 'v2').iterdir()) == {'personal'}, 'edition isolation failure'
    assert set(path.name for path in base.iterdir()) == {'source', 'target'}, 'unexpected backup artifacts'
    assert set(path.name for path in base.parent.iterdir()) == {'backup-fixture'}, 'unexpected business writes'
    for directory in [root / 'v2', base.parent, base]: filesystem.append(runner.private(directory, 0o700, True))
    # Do not follow any directory or file links. Bounded traversal rejects a runaway fixture tree.
    count = 0
    def walk(directory):
        nonlocal count
        filesystem.append(runner.private(directory, 0o700, True))
        for path in directory.iterdir():
            count += 1
            assert count <= 64, 'backup fixture filesystem bound'
            assert not path.is_symlink(), 'backup symlink forbidden'
            if path.is_dir(): walk(path)
            else:
                filesystem.append(runner.private(path, 0o600))
                assert path.stat().st_size <= 65536, 'backup file bound'
                assert path.suffix != '.csbackup', 'encrypted backup persisted unexpectedly'
    walk(base / 'source'); walk(base / 'target')
    target = base / 'target'
    active = json.loads(runner.bounded_text(target / 'active.json', 4096))
    assert active == {'schema': 1, 'generation': proof['new_generation']}, 'independent active generation mismatch'
    state = json.loads(runner.bounded_text(target / active['generation'] / 'state.json', 65536))
    assert [slot['enabled'] for slot in state['draft']['slots']] == [True, False, True]
    assert state['recovery_required'] is True, 'independent recovery gate missing'
    old = json.loads(runner.bounded_text(target / proof['old_generation'] / 'state.json', 65536))
    assert [slot['enabled'] for slot in old['draft']['slots']] == [False] * 3, 'old generation damaged'
    return dict(proof, status='PASS', edition=edition, mode='backup', filesystem=filesystem,
                memory_vault_only=True, original_handler='NOT_RUN', real_keychain='NOT_ACCESSED')
