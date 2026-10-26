"""Draft receipt oracle regressions. No native process or network."""
import copy
import unittest
from pathlib import Path
from urllib.parse import urlsplit
import re
import tempfile
import json
import hashlib
import draft


class DraftOracleTests(unittest.TestCase):
    def rows(self):
        current = draft.initial_example()
        rows = []
        for index, (name, command, request, accepted) in enumerate(draft.cases(current)):
            before = {'label': name + ':before', 'files': {'state.json': str(index).zfill(64)}}
            value = draft.expected_value(current, command, request) if accepted else None
            if accepted:
                value.update(content_revision=current['content_revision'] + 1, release_id=('a' if index % 2 else 'b') * 32)
            after = {'label': name + ':after', 'files': {'state.json': str(index + 1 if accepted else index).zfill(64)}}
            rows.append(dict(id=name, command=command, input=request, accepted=accepted, value=value,
                             readback=copy.deepcopy(value if accepted else current), error=None if accepted else self.error(name),
                             before=before, after=after))
            if accepted: current = value
        # Snapshot chain must model no mutation between operations.
        for i, row in enumerate(rows):
            if i: row['before']['files'] = copy.deepcopy(rows[i - 1]['after']['files'])
            if not row['accepted']: row['after']['files'] = copy.deepcopy(row['before']['files'])
        return rows

    def error(self, name):
        business = {'overlap': 'campaign_overlap', 'duplicate_slot': 'manifest_slots', 'invalid_url': 'merchant_scheme', 'reversed_time': 'campaign_time'}
        if name in business: return '发布数据无效：' + business[name]
        detail = {'unknown_field': 'unknown field `unknown`', 'missing_slots': 'missing field `slots`',
                  'wrong_slots_type': 'invalid type: string "wrong", expected a sequence',
                  'missing_manifest': 'command ad_draft_save missing required key manifest',
                  'invalid_pause': 'unknown variant `unknown`'}[name]
        command, arg = ('pause_ad', 'slot') if name == 'invalid_pause' else ('ad_draft_save', 'manifest')
        return f'invalid args `{arg}` for command `{command}`: {detail}'

    def test_rejects_wrong_error_classification_for_each_rejected_input(self):
        for i in range(9, 18):
            with self.subTest(case=i):
                self.reject(lambda rows: rows[i].update(error='unrelated operational failure'))

    def verify(self, rows=None):
        rows = self.rows() if rows is None else rows
        events = [r[k] for r in rows for k in ('before', 'after')]
        return draft.validate_steps(draft.initial_example(), rows, events)

    def test_success_campaign_hosts_satisfy_public_merchant_url_contract(self):
        for _, command, request, accepted in draft.cases(draft.initial_example()):
            if accepted and command == 'ad_draft_save':
                for slot in request['manifest']['slots']:
                    for campaign in slot['campaigns']:
                        url = urlsplit(campaign['target_url'])
                        self.assertEqual(url.scheme, 'https')
                        self.assertNotIn(url.hostname.rsplit('.', 1)[-1], ['example', 'test', 'invalid', 'localhost', 'local'])

    def test_embedded_success_campaign_host_satisfies_merchant_contract(self):
        page = (Path(__file__).parent / 'draft.mjs').read_text()
        url = urlsplit(re.search(r"target_url: '([^']+)'", page)[1])
        self.assertEqual(url.scheme, 'https')
        self.assertNotIn(url.hostname.rsplit('.', 1)[-1], ['example', 'test', 'invalid', 'localhost', 'local'])

    def test_accepts_multi_slot_edit_pause_and_rejection_matrix(self): self.assertEqual(self.verify()['accepted'], 9)
    def reject(self, change):
        rows = self.rows(); change(rows)
        with self.assertRaises(AssertionError): self.verify(rows)
    def test_rejects_failed_save_changing_files(self): self.reject(lambda r: r[-1]['after']['files'].update(extra='a' * 64))
    def test_rejects_pause_losing_other_slot_campaigns(self): self.reject(lambda r: r[4]['value']['slots'][1].update(campaigns=[]))
    def test_rejects_readback_differing_from_saved_value(self): self.reject(lambda r: r[0]['readback']['slots'][0].update(enabled=False))
    def test_rejects_missing_case(self): self.reject(lambda r: r.pop())
    def test_rejects_wrong_request(self): self.reject(lambda r: r[0]['input'].update(manifest={}))
    def test_rejects_nonincrementing_revision(self): self.reject(lambda r: r[0]['value'].update(content_revision=1))
    def disk_case(self, mutate=None):
        with tempfile.TemporaryDirectory(prefix='draft-oracle-') as temporary:
            folder = Path(temporary)
            publisher = folder / 'tmp/business-root/v2/personal/publisher'
            for path in [folder / 'tmp', folder / 'tmp/business-root', folder / 'tmp/business-root/v2', publisher.parent, publisher, publisher / ('a' * 32), publisher / ('a' * 32) / 'assets']:
                path.mkdir(mode=0o700)
            rows = self.rows()
            files = {'active.json': dict(schema=1, generation='a' * 32),
                     'a' * 32 + '/state.json': dict(draft=rows[-1]['readback'], assets=[], history=[], pending=None)}
            hashes = {}
            for name, data in files.items():
                path = publisher / name
                path.write_text(json.dumps(data)); path.chmod(0o600)
                hashes[name] = hashlib.sha256(path.read_bytes()).hexdigest()
            rows[8]['after']['files'] = hashes.copy()
            for row in rows[9:]:
                row['before']['files'] = hashes.copy(); row['after']['files'] = hashes.copy()
            events = [r[k] for r in rows for k in ('before', 'after')]
            stdout = ''.join('BUSINESS_IPC_DRAFT ' + json.dumps(e) + '\n' for e in events)
            if mutate: mutate(publisher)
            return draft.validate(stdout, {'initial': draft.initial_example(), 'draft': rows}, 'personal', folder, [])

    def test_accepts_independently_read_final_disk_and_permissions(self): self.assertEqual(self.disk_case()['status'], 'PASS')
    def test_rejects_final_state_byte_tampering(self):
        with self.assertRaises(AssertionError):
            self.disk_case(lambda p: (p / ('a' * 32) / 'state.json').write_text('{}'))
    def test_rejects_final_state_world_readable_permissions(self):
        with self.assertRaises(AssertionError):
            self.disk_case(lambda p: (p / ('a' * 32) / 'state.json').chmod(0o644))
    def test_rejects_unexpected_final_asset(self):
        def mutate(p):
            asset = p / ('a' * 32) / 'assets/unexpected'
            asset.write_bytes(b'synthetic'); asset.chmod(0o600)
        with self.assertRaises(AssertionError): self.disk_case(mutate)

    def test_rejects_arbitrary_public_failure(self):
        with self.assertRaises(AssertionError): draft.validate_public([{'command': c, 'kind': 'missing'} for c in draft.COMMANDS])
    def test_accepts_exact_public_permission_denials(self):
        draft.validate_public([{'command': c, 'kind': 'permission'} for c in draft.COMMANDS])

if __name__ == '__main__': unittest.main()
