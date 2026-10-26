"""Synthetic regression tests for the preference fault runner; no Cargo or network."""
import tempfile
import sys
import time
import unittest
from pathlib import Path
from runner import baseline_profile, combined_profile, phase_environment, require_child, run_bounded


class RunnerTests(unittest.TestCase):
    def test_profile_preserves_baseline(self):
        home = Path('/private/tmp/synthetic-preferences')
        baseline = baseline_profile()
        paths = [home / '.claude-switcher/v2' / e / 'activity.json' for e in ('personal', 'public')]
        self.assertTrue(combined_profile(baseline, home, paths).startswith(baseline + b'\n'))
        for bad in [paths[:1], list(reversed(paths)), [Path('/Users/gua/.claude/activity.json'), paths[1]]]:
            with self.assertRaises(ValueError):
                combined_profile(baseline, home, bad)
        with self.assertRaises(ValueError):
            combined_profile(baseline + b' ', home, paths)

    def test_blocked_destination_does_not_poison_prepare_or_reopen(self):
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory).resolve()
            home = out / 'home'
            home.mkdir(mode=0o700)
            environment = {'HOME': str(home)}
            blocked = phase_environment(environment, out, 'blocked')
            prepared = phase_environment(environment, out, 'prepare')
            denied = phase_environment(environment, out, 'denied')
            reopened = phase_environment(environment, out, 'reopen')
            relative = Path('.claude-switcher/v2/personal/preferences.json')
            obstruction = Path(blocked['HOME']) / relative
            obstruction.mkdir(parents=True)
            destination = Path(prepared['HOME']) / relative
            destination.parent.mkdir(parents=True)
            destination.write_text('committed synthetic preferences')
            self.assertTrue(obstruction.is_dir())
            self.assertEqual((Path(denied['HOME']) / relative).read_text(),
                             'committed synthetic preferences')
            self.assertEqual((Path(reopened['HOME']) / relative).read_text(),
                             'committed synthetic preferences')
            self.assertEqual(environment, {'HOME': str(home)})

    def test_hung_child_is_terminated_and_partial_log_is_retained_as_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            log = folder / 'timeout.log'
            started = time.monotonic()
            code, timed_out = run_bounded(
                [sys.executable, '-B', '-c',
                 "import time; print('synthetic progress before hang', flush=True); time.sleep(60)"],
                folder, {'HOME': str(folder), 'PATH': '/usr/bin:/bin'}, log, timeout=0.3)
            self.assertLess(time.monotonic() - started, 5)
            self.assertTrue(timed_out)
            self.assertEqual(code, 124)
            text = log.read_text()
            self.assertIn('synthetic progress before hang', text)
            self.assertIn('PREFERENCE_FAULT_TIMEOUT', text)
            with self.assertRaises(ValueError):
                require_child(code, text)

    def test_child_rejects_failure_missing_or_extra_harness(self):
        ok = 'test result: ok. 1 passed; 0 failed; 0 ignored;\n'
        require_child(0, ok)
        for code, text in [(101, ok), (0, ''), (0, ok + ok), (0, 'test result: FAILED. 0 passed; 1 failed;')]:
            with self.assertRaises(ValueError):
                require_child(code, text)


if __name__ == '__main__':
    unittest.main(verbosity=2)
