import json
from pathlib import Path
import re
import hashlib
import tempfile
import unittest

from .reconcile import CHILD, PARENT, reconcile, reconcile_receipts

SUMMARY = 'test result: ok. {count} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s'
CHILD_BLOCK = (
    'running 1 test\n'
    'F06 fresh recovery pid={pid} recoveries=2 metadata/stages/journal=ok\n'
    f'test {CHILD} ... ok\n\n'
    + SUMMARY.format(count=1) + '\n'
)
TEXT = (
    'Finished test profile\nRunning unittests core (target/core)\n'
    'running 3 tests\n'
    f'test {CHILD} ... ok\n'
    f'test {PARENT} ... \n'
    + '\n'.join(CHILD_BLOCK.format(pid=pid) for pid in [101, 102, 103])
    + '\nok\n'
    'test shared ... ok\n'
    + SUMMARY.format(count=3) + '\n'
    'Running unittests other (target/other)\nrunning 1 test\ntest shared ... ok\n'
    + SUMMARY.format(count=1) + '\n'
    'Running unittests empty (target/empty)\nrunning 0 tests\n'
    + SUMMARY.format(count=0) + '\n'
)
LIST = (
    'Finished test profile\nRunning unittests core (target/core)\n'
    f'{CHILD}: test\n{PARENT}: test\nshared: test\n3 tests, 0 benchmarks\n'
    'Running unittests other (target/other)\nshared: test\n1 test, 0 benchmarks\n'
    'Running unittests empty (target/empty)\n0 tests, 0 benchmarks\n'
)
BLOCK = CHILD_BLOCK.format(pid=101).rstrip('\n')
SOURCES = {'synthetic/source.rs': hashlib.sha256(b'synthetic source input').hexdigest()}


def write_receipts(directory):
    """Create complete, self-consistent evidence without reading project files."""
    for label, text in [('tests', TEXT), ('list', LIST), ('fmt', ''), ('clippy', '')]:
        log = directory / (label+'.log')
        log.write_text(text)
        receipt = {
            'exit': 0, 'source_unchanged': True,
            'source_before': dict(SOURCES), 'source_after': dict(SOURCES),
            'log_sha256': hashlib.sha256(log.read_bytes()).hexdigest(),
        }
        if label == 'list':
            receipt['listed_names'] = [CHILD, PARENT, 'shared', 'shared']
        (directory / (label+'-result.json')).write_text(json.dumps(receipt))
    summary = {
        'passed': False, 'source_before': dict(SOURCES), 'source_after': dict(SOURCES),
        'actual_execution_count': 6, 'passed_execution_count': 6,
        'listed_count': 4, 'run_list_multiset_equal': False,
    }
    (directory / 'summary.json').write_text(json.dumps(summary))


class ReconciliationTests(unittest.TestCase):
    def rejects(self, text=TEXT, inventory=LIST):
        with self.assertRaises(ValueError):
            reconcile(text, inventory)

    def test_synthetic_nested_log_and_per_harness_inventory(self):
        result = reconcile(TEXT, LIST)
        self.assertEqual(result['top_level_passed_records'], 4)
        self.assertEqual(result['distinct_top_level_names'], 3)
        self.assertEqual(result['nested_child_passed_records'], 3)

    def test_flat_and_empty_harnesses(self):
        text = 'Running example (bin)\nrunning 1 test\ntest one ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\nRunning empty (bin2)\nrunning 0 tests\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n'
        inventory = 'Running example (bin)\none: test\n1 test, 0 benchmarks\nRunning empty (bin2)\n0 tests, 0 benchmarks\n'
        self.assertEqual(reconcile(text, inventory)['top_level_passed_records'], 1)

    def test_failed_child_record(self):
        self.rejects(TEXT.replace(BLOCK, BLOCK.replace(f'test {CHILD} ... ok', f'test {CHILD} ... FAILED'), 1))

    def test_failed_child_summary(self):
        self.rejects(TEXT.replace(BLOCK, BLOCK.replace('test result: ok. 1 passed; 0 failed', 'test result: FAILED. 0 passed; 1 failed'), 1))

    def test_truncated_child(self):
        self.rejects(TEXT[:TEXT.index(BLOCK) + len(BLOCK)//2])

    def test_truncated_parent(self):
        self.rejects(TEXT[:TEXT.index('\nok\n')])

    def test_missing_parent_ok(self):
        self.rejects(TEXT.replace('\nok\n', '\n', 1))

    def test_extra_child(self):
        extra = re.sub(r'pid=\d+', 'pid=999999999', BLOCK)
        self.rejects(TEXT.replace('\nok\n', '\n'+extra+'\n\nok\n', 1))

    def test_missing_child(self):
        self.rejects(TEXT.replace(BLOCK, '', 1))

    def test_wrong_child_name(self):
        self.rejects(TEXT.replace(BLOCK, BLOCK.replace(CHILD, 'wrong_child'), 1))

    def test_repeated_pid(self):
        pids = re.findall(r'F06 fresh recovery pid=(\d+)', TEXT)
        self.rejects(TEXT.replace('pid='+pids[1], 'pid='+pids[0], 1))

    def test_missing_recovery_proof(self):
        changed = re.sub(r'F06 fresh recovery pid=\d+ recoveries=2 metadata/stages/journal=ok\n', '', BLOCK)
        self.rejects(TEXT.replace(BLOCK, changed, 1))

    def test_unexpected_nested_output(self):
        self.rejects(TEXT.replace(BLOCK, BLOCK.replace('F06 fresh recovery', 'panic: unexpected F06 fresh recovery'), 1))

    def test_orphan_ok(self):
        self.rejects(TEXT+'\nok\n')

    def test_parent_summary_count_mismatch(self):
        self.rejects(TEXT.replace('test result: ok. 3 passed;', 'test result: ok. 2 passed;', 1))

    def test_top_level_failure(self):
        self.rejects(TEXT.replace(' ... ok\n', ' ... FAILED\n', 1))

    def test_list_name_mismatch(self):
        self.rejects(inventory=LIST.replace(PARENT+': test', 'different_parent: test', 1))

    def test_list_harness_mismatch(self):
        self.rejects(inventory=LIST.replace('Running unittests core', 'Running unittests wrong', 1))

    def test_truncated_inventory(self):
        self.rejects(inventory=LIST.rsplit('0 tests, 0 benchmarks', 1)[0])

    def ignored_log(self, status='ignored, requires isolated fixture'):
        text = 'Running ignored suite (bin)\nrunning 2 tests\ntest ordinary ... ok\ntest isolated ... ' + status + '\n' + 'test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'
        inventory = 'Running ignored suite (bin)\nordinary: test\nisolated: test\n2 tests, 0 benchmarks\n'
        return text, inventory

    def test_ignored_with_reason_is_reported_separately_from_passed(self):
        result = reconcile(*self.ignored_log())
        self.assertEqual(result['top_level_passed_records'], 1)
        self.assertEqual(result['top_level_ignored_records'], 1)
        self.assertEqual(result['top_level_total_records'], 2)
        self.assertEqual(result['ignored_tests'], [{'harness': 'ignored suite (bin)', 'name': 'isolated', 'reason': 'requires isolated fixture'}])

    def test_bare_ignored_records_absent_reason_without_inventing_one(self):
        result = reconcile(*self.ignored_log('ignored'))
        self.assertIsNone(result['ignored_tests'][0]['reason'])

    def test_ignored_reason_can_include_commas(self):
        result = reconcile(*self.ignored_log('ignored, isolated fixture, no production data'))
        self.assertEqual(result['ignored_tests'][0]['reason'], 'isolated fixture, no production data')

    def test_ignored_summary_must_match_records(self):
        text, inventory = self.ignored_log()
        self.rejects(text.replace('1 passed; 0 failed; 1 ignored', '2 passed; 0 failed; 0 ignored'), inventory)

    def test_ignored_name_must_remain_in_per_harness_inventory(self):
        text, inventory = self.ignored_log()
        self.rejects(text, inventory.replace('isolated: test', 'different: test'))

    def test_ignored_does_not_hide_failed_test(self):
        text, inventory = self.ignored_log()
        self.rejects(text.replace('ordinary ... ok', 'ordinary ... FAILED'), inventory)

    def test_unknown_or_prefixed_ignored_status_is_rejected(self):
        for status in ['ignored_extra', 'ignored: reason', 'UNKNOWN', 'ignored,', 'ok, reason', 'FAILED, reason']:
            with self.subTest(status=status):
                self.rejects(*self.ignored_log(status))

    def test_duplicate_ignored_names_are_rejected(self):
        text, inventory = self.ignored_log()
        self.rejects(text.replace('ordinary ... ok', 'isolated ... ignored'), inventory)

    def test_nested_child_cannot_be_ignored(self):
        self.rejects(TEXT.replace(BLOCK, BLOCK.replace(f'test {CHILD} ... ok', f'test {CHILD} ... ignored, dedicated fixture'), 1))

    def test_receipts_bind_synthetic_inputs(self):
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            write_receipts(directory)
            result = reconcile_receipts(directory)
        self.assertFalse(result['original_summary_passed'])
        self.assertEqual(result['status'], 'PASS_STRICT_RECONCILIATION')
        self.assertEqual(result['source_snapshot'], SOURCES)

    def receipt_mutation_rejected(self, filename, mutate):
        with tempfile.TemporaryDirectory() as name:
            directory = Path(name)
            write_receipts(directory)
            path = directory/filename
            data = json.loads(path.read_text())
            mutate(data)
            path.write_text(json.dumps(data))
            with self.assertRaises(ValueError):
                reconcile_receipts(directory)

    def test_receipt_log_hash_mismatch(self):
        self.receipt_mutation_rejected('tests-result.json', lambda x: x.update(log_sha256='0'*64))

    def test_receipt_source_changed(self):
        self.receipt_mutation_rejected('tests-result.json', lambda x: x.update(source_after={}))

    def test_cross_command_source_mismatch(self):
        self.receipt_mutation_rejected('fmt-result.json', lambda x: x.update(source_before={}, source_after={}))

    def test_receipt_inventory_disagrees(self):
        self.receipt_mutation_rejected('list-result.json', lambda x: x.update(listed_names=[]))

    def test_nonzero_command_exit(self):
        self.receipt_mutation_rejected('clippy-result.json', lambda x: x.update(exit=1))


if __name__ == '__main__':
    unittest.main(verbosity=2)
