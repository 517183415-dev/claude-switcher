"""Independent bounded oracle for real production draft commands; synthetic metadata only."""
import copy
import json
import re
from pathlib import Path

COMMANDS = ['ad_draft_read', 'ad_draft_save', 'pause_ad']
SLOTS = ['residential_broadband', 'account_service', 'kyc_service']


def initial_example():
    return dict(schema_version=1, channel='preview', content_revision=1, key_id='ads-personal-v1',
                issued_at=1000, expires_at=7777000, release_id='0' * 32,
                slots=[dict(slot=slot, enabled=True, campaigns=[]) for slot in SLOTS])


def filled(base):
    value = copy.deepcopy(base)
    for i, slot in enumerate(value['slots']):
        slot['enabled'] = True
        slot['campaigns'] = [dict(campaign_id=f'synthetic-{i}-{j}', merchant_id='synthetic-merchant',
            merchant_name='Synthetic merchant', title=f'Synthetic {i}/{j}', description='Synthetic draft only',
            image=dict(sha256='a' * 64, mime='image/png', bytes=1, width=1, height=1),
            button_text='View', target_url='https://synthetic.example.com/', starts_at=1000 + j * 1000,
            ends_at=2000 + j * 1000, enabled=True) for j in range(2)]
    return value


def expected_value(current, command, request):
    value = copy.deepcopy(request['manifest'] if command == 'ad_draft_save' else current)
    if command == 'pause_ad':
        for slot in value['slots']:
            if request['slot'] is None or slot['slot'] == request['slot']:
                slot['enabled'] = False
                if request['slot'] is None: slot['campaigns'] = []
    return value


def cases(base):
    # Inputs carry prior metadata. Production assigns its own revision/release/time.
    current = filled(base)
    result = [('save_six', 'ad_draft_save', {'manifest': copy.deepcopy(current)}, True)]
    for i in range(3):
        current['slots'][i]['campaigns'][1]['title'] = f'Edited {i}'
        result.append((f'edit_{i}', 'ad_draft_save', {'manifest': copy.deepcopy(current)}, True))
    for i, slot in enumerate(SLOTS): result.append((f'pause_{i}', 'pause_ad', {'slot': slot}, True))
    result.append(('resume_six', 'ad_draft_save', {'manifest': filled(base)}, True))
    result.append(('pause_all', 'pause_ad', {'slot': None}, True))
    valid = filled(base)
    def bad(name, mutate):
        value = copy.deepcopy(valid); mutate(value)
        result.append((name, 'ad_draft_save', {'manifest': value}, False))
    bad('overlap', lambda v: v['slots'][0]['campaigns'][1].update(starts_at=1500))
    bad('duplicate_slot', lambda v: v['slots'][1].update(slot=SLOTS[0]))
    bad('invalid_url', lambda v: v['slots'][0]['campaigns'][0].update(target_url='http://synthetic.example/'))
    bad('reversed_time', lambda v: v['slots'][0]['campaigns'][0].update(ends_at=999))
    bad('unknown_field', lambda v: v.update(unknown=True))
    bad('missing_slots', lambda v: v.pop('slots'))
    bad('wrong_slots_type', lambda v: v.update(slots='wrong'))
    result.extend([('missing_manifest', 'ad_draft_save', {}, False),
                   ('invalid_pause', 'pause_ad', {'slot': 'unknown'}, False)])
    return result


def validate_public(rows):
    assert rows == [{'command': command, 'kind': 'permission'} for command in COMMANDS], 'Public requires exact permission denial'


def semantic(value):
    return {k: v for k, v in value.items() if k not in ('content_revision', 'release_id', 'issued_at', 'expires_at')}


def validate_error(name, error):
    business = {'overlap': 'campaign_overlap', 'duplicate_slot': 'manifest_slots',
                'invalid_url': 'merchant_scheme', 'reversed_time': 'campaign_time'}
    if name in business:
        assert error == '发布数据无效：' + business[name], 'wrong business rejection category'
        return
    command, argument = ('pause_ad', 'slot') if name == 'invalid_pause' else ('ad_draft_save', 'manifest')
    prefix = f'invalid args `{argument}` for command `{command}`: '
    assert error.startswith(prefix), 'wrong decoder rejection category'
    detail = error[len(prefix):]
    expected = {'unknown_field': 'unknown field `unknown`', 'missing_slots': 'missing field `slots`',
                'wrong_slots_type': 'invalid type: string "wrong", expected a sequence',
                'missing_manifest': 'command ad_draft_save missing required key manifest',
                'invalid_pause': 'unknown variant `unknown`'}[name]
    assert detail == expected or (name in ('unknown_field', 'invalid_pause') and detail.startswith(expected + ', expected ')), 'wrong decoder detail'


def validate_steps(initial, rows, events):
    assert initial['content_revision'] == 1 and initial['key_id'] == 'ads-personal-v1'
    assert [s['slot'] for s in initial['slots']] == SLOTS and all(not s['campaigns'] and s['enabled'] is True for s in initial['slots'])
    expected = cases(initial)
    assert len(rows) == len(expected) and events == [r[k] for r in rows for k in ('before', 'after')], 'native snapshots mismatch'
    current = copy.deepcopy(initial)
    previous = None
    for row, (name, command, request, accepted) in zip(rows, expected):
        assert set(row) == {'id', 'command', 'input', 'accepted', 'value', 'readback', 'error', 'before', 'after'}
        assert (row['id'], row['command'], row['input'], row['accepted']) == (name, command, request, accepted), 'draft matrix mismatch'
        for key in ('before', 'after'):
            snapshot = row[key]
            assert set(snapshot) == {'label', 'files'} and snapshot['label'] == name + ':' + key
            assert snapshot['files'] and len(snapshot['files']) <= 32
            for path, digest in snapshot['files'].items():
                assert not Path(path).is_absolute() and '..' not in Path(path).parts
                assert isinstance(digest, str) and re.fullmatch('[0-9a-f]{64}', digest)
        if previous is not None: assert row['before']['files'] == previous, 'unobserved file mutation'
        if accepted:
            value = row['value']
            assert row['error'] is None and isinstance(value, dict)
            assert semantic(value) == semantic(expected_value(current, command, request)), 'draft semantic mismatch'
            assert type(value['content_revision']) is int and value['content_revision'] == current['content_revision'] + 1
            assert re.fullmatch('[0-9a-f]{32}', value['release_id']) and value['release_id'] != current['release_id']
            assert type(value['issued_at']) is int and value['issued_at'] >= initial['issued_at']
            assert value['expires_at'] - value['issued_at'] == 90 * 24 * 60 * 60
            current = value
        else:
            assert row['value'] is None and isinstance(row['error'], str) and row['error']
            validate_error(name, row['error'])
            assert row['before']['files'] == row['after']['files'], 'rejection mutated publisher files'
        assert row['readback'] == current, 'reopened draft mismatch'
        previous = row['after']['files']
    return {'accepted': 9, 'rejected': len(expected) - 9, 'final': current, 'files': previous}


def validate(stdout, responses, edition, folder, filesystem):
    from runner import private, bounded_text, sha
    root = folder / 'tmp/business-root'
    if edition == 'public':
        assert set(responses) == {'draft'}; validate_public(responses['draft'])
        assert list(root.iterdir()) == [], 'Public draft probes wrote files'
        return dict(status='PASS', mode='draft', edition=edition, permission_denials=3, no_writes=True, filesystem=filesystem)
    assert set(responses) == {'draft', 'initial'}
    prefix = 'BUSINESS_IPC_DRAFT '
    events = [json.loads(line[len(prefix):]) for line in stdout.splitlines() if line.startswith(prefix)]
    result = validate_steps(responses['initial'], responses['draft'], events)
    directory = root / 'v2/personal'
    assert set((root / 'v2').iterdir()) == {directory}, 'edition isolation failed'
    assert set(p.name for p in directory.iterdir()) == {'publisher'}, 'unexpected side effects'
    publisher = directory / 'publisher'
    actual = {}
    for path in [root / 'v2', directory, publisher]: filesystem.append(private(path, 0o700, True))
    for path in publisher.rglob('*'):
        if path.is_dir() and not path.is_symlink(): filesystem.append(private(path, 0o700, True))
        else:
            filesystem.append(private(path, 0o600)); actual[str(path.relative_to(publisher))] = sha(path)
    assert actual == result['files'], 'native snapshot differs from independent disk hashes'
    active = json.loads(bounded_text(publisher / 'active.json', 4096))
    assert re.fullmatch('[0-9a-f]{32}', active['generation'])
    state = json.loads(bounded_text(publisher / active['generation'] / 'state.json', 65536))
    assert state['draft'] == result['final'] and state['assets'] == [] and state['history'] == [] and state['pending'] is None
    return dict(status='PASS', mode='draft', edition=edition, accepted=result['accepted'], rejected=result['rejected'],
                files=actual, filesystem=filesystem, reopened_after_each_call=True, metadata_only_images=True,
                keychain='NOT_RUN', object_store='NOT_RUN', ordinary_startup='NOT_RUN')
