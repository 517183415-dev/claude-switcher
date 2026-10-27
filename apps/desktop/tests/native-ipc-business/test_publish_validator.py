"""Regression for cleanup's optional serialized field."""
import unittest
from publish_persistence import assert_cleanup_finished


class CleanupFinishedTest(unittest.TestCase):
    def test_missing_and_null_are_finished(self):
        for state in ({}, {'cleanup': None}):
            with self.subTest(state=state):
                assert_cleanup_finished(state)

    def test_nonempty_batch_is_rejected(self):
        with self.assertRaisesRegex(AssertionError, 'cleanup still pending'):
            assert_cleanup_finished({'cleanup': {'batch_id': 'synthetic'}})


if __name__ == '__main__':
    unittest.main()
