"""Keep account source visible without exposing generated account data."""
import subprocess
import unittest
from pathlib import Path


class SourceTrackingTests(unittest.TestCase):
    def test_account_source_is_visible_and_account_data_stays_ignored(self):
        source = {
            "crates/switcher-core/src/accounts/legacy.rs",
            "crates/switcher-core/src/accounts/tests/native_metadata.rs",
        }
        data = {"accounts/A/session", "output/synthetic-home/accounts/B/session"}
        result = subprocess.run(
            ["git", "check-ignore", "--no-index", "--stdin"],
            cwd=Path(__file__).resolve().parents[1],
            input="\n".join(sorted(source | data)) + "\n",
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertIn(result.returncode, (0, 1), result.stderr)
        self.assertEqual(set(result.stdout.splitlines()), data)


if __name__ == "__main__":
    unittest.main()
