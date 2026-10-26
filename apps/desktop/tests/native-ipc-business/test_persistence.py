import copy
import argparse
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import runner
import persistence


class PersistenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='persistence-parser-')
        self.addCleanup(self.temp.cleanup)
        self.folder = Path(self.temp.name).resolve()
        self.root = self.folder / 'business-root'
        self.root.mkdir(mode=0o700)
        self.log = [{'observed_at': 1700000000, 'level': 'info', 'message': '已保存本机设置；全部广告关闭时不发送广告请求'}]
        self.values = {
            'personal': {'ads': {'slots_enabled': [True, False, True], 'request_purpose_acknowledged': True}, 'updates_paused_until': 2000000001},
            'public': {'ads': {'slots_enabled': [False, True, False], 'request_purpose_acknowledged': True}, 'updates_paused_until': 2000000002},
        }

    def state(self, editions):
        result = {}
        for edition in editions:
            result['v2/' + edition + '/preferences.json'] = json.dumps(self.values[edition]).encode()
            result['v2/' + edition + '/activity.json'] = json.dumps(self.log, ensure_ascii=False).encode()
        return result

    def protocol(self, edition, action, before, after, value=None):
        rows = []
        for moment, state in [('before', before), ('after', after)]:
            entry = {'label': 'persistence-' + action + ':' + moment}
            for name in ('preferences', 'activity'):
                raw = state.get('v2/' + edition + '/' + name + '.json')
                entry[name] = list(raw) if raw is not None else None
            rows.append('BUSINESS_IPC_PREFERENCES ' + json.dumps(entry))
        result = {'fixture': 'ipc-business', 'ordinary_runtime': False, 'edition': edition,
                  'preflight': False, 'ok': True, 'error': None,
                  'denials': [{'command': name, 'kind': 'permission' if index == 0 else 'missing'} for index, name in enumerate(runner.DENIALS)],
                  'responses': {'persistence': {'action': action, 'value': value if value is not None else self.values[edition]}}}
        rows.append('BUSINESS_IPC_RESULT ' + json.dumps(result))
        return '\n'.join(rows)

    def test_personal_writer_preserves_nondefault_preferences(self):
        after = self.state(['personal'])
        result = persistence.validate_phase(self.protocol('personal', 'write', {}, after), 0, 'personal', 'write', {}, after)
        self.assertEqual(result['value'], self.values['personal'])

    def test_public_writer_preserves_existing_personal_bytes(self):
        before, after = self.state(['personal']), self.state(['personal', 'public'])
        result = persistence.validate_phase(self.protocol('public', 'write', before, after), 0, 'public', 'write', before, after)
        self.assertTrue(result['other_edition_unchanged'])

    def test_personal_reader_preserves_both_editions(self):
        state = self.state(['personal', 'public'])
        result = persistence.validate_phase(self.protocol('personal', 'read', state, state), 0, 'personal', 'read', state, state)
        self.assertTrue(result['read_only_bytes_unchanged'])

    def test_public_reader_uses_public_preference_value(self):
        state = self.state(['personal', 'public'])
        result = persistence.validate_phase(self.protocol('public', 'read', state, state), 0, 'public', 'read', state, state)
        self.assertEqual(result['value'], self.values['public'])

    def test_reader_rejects_default_instead_of_persisted_value(self):
        state = self.state(['personal'])
        wrong = {'ads': {'slots_enabled': [False, False, False], 'request_purpose_acknowledged': False}, 'updates_paused_until': None}
        with self.assertRaises(AssertionError):
            persistence.validate_phase(self.protocol('personal', 'read', state, state, wrong), 0, 'personal', 'read', state, state)

    def test_reader_rejects_other_edition_value(self):
        state = self.state(['personal', 'public'])
        with self.assertRaises(AssertionError):
            persistence.validate_phase(self.protocol('personal', 'read', state, state, self.values['public']), 0, 'personal', 'read', state, state)

    def test_reader_rejects_activity_append(self):
        before = self.state(['personal']); after = dict(before)
        after['v2/personal/activity.json'] += b' '
        with self.assertRaises(AssertionError):
            persistence.validate_phase(self.protocol('personal', 'read', before, after), 0, 'personal', 'read', before, after)

    def test_reader_rejects_preference_rewrite(self):
        before = self.state(['personal']); after = dict(before)
        after['v2/personal/preferences.json'] += b' '
        with self.assertRaises(AssertionError):
            persistence.validate_phase(self.protocol('personal', 'read', before, after), 0, 'personal', 'read', before, after)

    def test_writer_rejects_other_edition_mutation(self):
        before, after = self.state(['personal']), self.state(['personal', 'public'])
        after['v2/personal/preferences.json'] += b' '
        with self.assertRaises(AssertionError):
            persistence.validate_phase(self.protocol('public', 'write', before, after), 0, 'public', 'write', before, after)

    def test_reader_rejects_missing_native_snapshot(self):
        state = self.state(['personal'])
        stdout = '\n'.join(self.protocol('personal', 'read', state, state).splitlines()[1:])
        with self.assertRaises(AssertionError): persistence.validate_phase(stdout, 0, 'personal', 'read', state, state)

    def test_reader_rejects_nonzero_process_exit(self):
        state = self.state(['personal'])
        with self.assertRaises(AssertionError): persistence.validate_phase(self.protocol('personal', 'read', state, state), 1, 'personal', 'read', state, state)

    def test_reader_rejects_number_in_boolean_field(self):
        state = self.state(['personal']); wrong = copy.deepcopy(self.values['personal'])
        wrong['ads']['request_purpose_acknowledged'] = 1
        with self.assertRaises(AssertionError): persistence.validate_phase(self.protocol('personal', 'read', state, state, wrong), 0, 'personal', 'read', state, state)

    def test_file_snapshot_rejects_linked_edition_directory(self):
        (self.root / 'v2').mkdir(mode=0o700)
        (self.root / 'v2/personal').symlink_to(self.folder, target_is_directory=True)
        with self.assertRaises(AssertionError): persistence.files(self.root)

    def test_file_snapshot_rejects_nonprivate_file(self):
        directory = self.root / 'v2/personal'
        directory.mkdir(parents=True, mode=0o700); directory.parent.chmod(0o700)
        for name in ('preferences.json', 'activity.json'):
            (directory / name).write_text('{}'); (directory / name).chmod(0o600)
        (directory / 'preferences.json').chmod(0o644)
        with self.assertRaises(AssertionError): persistence.files(self.root)

    def test_log_label_rejects_path_traversal_before_process_start(self):
        with patch.object(runner.subprocess, 'Popen', side_effect=RuntimeError('must not launch')):
            with self.assertRaises(ValueError): runner.execute(['/usr/bin/true'], self.folder, label='../escape')
        self.assertEqual(list(self.folder.iterdir()), [self.root])

    def test_build_binding_rejects_attempt_traversal_before_read(self):
        with self.assertRaises(argparse.ArgumentTypeError): runner.build_binding('personal', '../r1')

    def test_build_binding_rejects_unknown_edition_before_read(self):
        with self.assertRaises(ValueError): runner.build_binding('../personal', 'r1')

    def test_named_logs_refuse_reuse_before_process_start(self):
        path = self.folder / 'personal-write-stdout.log'; path.write_text('HISTORICAL')
        with patch.object(runner.subprocess, 'Popen', side_effect=RuntimeError('must not launch')):
            with self.assertRaises(FileExistsError): runner.execute(['/usr/bin/true'], self.folder, label='personal-write')
        self.assertEqual(path.read_text(), 'HISTORICAL')


if __name__ == '__main__':
    unittest.main(verbosity=2)
