import copy
import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import runner
from runner import check_result


def sample():
    def process(pid, ppid):
        return {'pid': pid, 'ppid': ppid, 'synthetic_home_roundtrip': True,
                'private_directory': {'denied': True},
                'network': {'loopback_bind': {'denied': True}, 'outbound': {
                    'policy_denied': True, 'actual_nonlocal_connection': 'NOT_RUN'}},
                'securityd': {'lookup': 'denied'}}
    return {'parent': process(41, 2), 'child': process(42, 41), 'direct_child_reaped': True,
            'webkit_process_isolation': 'NOT_PROVEN',
            'system_managed_process_cleanup': 'NOT_PROVEN'}


class ResultTests(unittest.TestCase):
    def test_invalid_nested_schema_produces_failure_receipt(self):
        value = sample()
        value['parent'] = []
        class FakeProcess:
            pid = 101
            def __init__(self, *args, stdout, **kwargs):
                stdout.write(json.dumps(value).encode())
            def wait(self, timeout):
                return 0
        with tempfile.TemporaryDirectory(dir=runner.support.ROOT / 'output') as directory:
            root = Path(directory)
            def profile(run, out, build):
                path = run / 'runtime.sb'
                path.write_text('(deny network*) .*securityd.*')
                return path
            with patch.object(runner.support, 'output_root', return_value=root), \
                 patch.object(runner.support, 'make_profile', side_effect=profile), \
                 patch.object(runner.subprocess, 'Popen', FakeProcess), \
                 patch('sys.argv', ['runner.py', '--output', str(root)]), \
                 contextlib.redirect_stdout(io.StringIO()) as output:
                self.assertEqual(runner.main(), 1)
            receipt = json.loads(Path(json.loads(output.getvalue())['receipt']).read_text())
            self.assertEqual(receipt['status'], 'NOT_PROVEN')
            self.assertIn('error', receipt)

    def test_all_nested_objects_reject_invalid_types(self):
        for path in [('parent',), ('child',), ('parent', 'private_directory'),
                     ('parent', 'network'), ('parent', 'network', 'loopback_bind'),
                     ('parent', 'network', 'outbound'), ('parent', 'securityd')]:
            for invalid in (None, [], 'invalid', 42):
                value = sample()
                node = value
                for key in path[:-1]:
                    node = node[key]
                node[path[-1]] = invalid
                with self.subTest(path=path, invalid=invalid), self.assertRaises(ValueError):
                    check_result(value)

    def test_exact_success_and_securityd_unproven(self):
        value = sample()
        self.assertEqual(check_result(value), 'DIRECT_PROCESSES_PASS')
        value['child']['securityd']['lookup'] = 'NOT_RUN'
        self.assertEqual(check_result(value), 'PARTIAL_SECURITYD_NOT_PROVEN')

    def test_each_direct_process_must_pass_each_control(self):
        for name in ('parent', 'child'):
            for section, key in [('private_directory', 'denied'),
                                 ('network', 'loopback_bind'), ('network', 'outbound')]:
                with self.subTest(process=name, section=section, key=key):
                    value = sample()
                    if key == 'loopback_bind':
                        value[name][section][key]['denied'] = False
                    elif key == 'outbound':
                        value[name][section][key]['policy_denied'] = False
                    else:
                        value[name][section][key] = False
                    with self.assertRaises(ValueError):
                        check_result(value)

    def test_unexpected_securityd_lookup_is_failure(self):
        value = sample()
        value['parent']['securityd']['lookup'] = 'unexpected_allowed'
        with self.assertRaises(ValueError):
            check_result(value)

    def test_reaping_and_identity_are_required(self):
        for field in ('direct_child_reaped', 'child_pid', 'child_ppid'):
            value = copy.deepcopy(sample())
            if field == 'direct_child_reaped':
                value[field] = False
            elif field == 'child_pid':
                value['child']['pid'] = value['parent']['pid']
            else:
                value['child']['ppid'] = 999
            with self.subTest(field=field), self.assertRaises(ValueError):
                check_result(value)

    def test_unproven_boundaries_and_no_nonlocal_connection_are_required(self):
        for field in ('webkit_process_isolation', 'system_managed_process_cleanup', 'nonlocal'):
            value = sample()
            if field == 'nonlocal':
                value['parent']['network']['outbound']['actual_nonlocal_connection'] = 'PASS'
            else:
                value[field] = 'PASS'
            with self.subTest(field=field), self.assertRaises(ValueError):
                check_result(value)


if __name__ == '__main__':
    unittest.main()
