"""The formal build must validate fixed trust material before packaging."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class BuildGateTests(unittest.TestCase):
    def run_build(self, *arguments, validation_exit=0):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            log = path / "commands"
            for name, body in {
                "uname": 'case "$1" in -s) echo Darwin;; -m) echo arm64;; esac',
                "cargo": 'echo "cargo $*" >> "$TEST_LOG"\n'
                'case " $* " in *" --bin validate-ad-release "*) exit "$TEST_VALIDATION_EXIT";; esac',
                "npm": 'echo "npm $*" >> "$TEST_LOG"',
            }.items():
                command = path / name
                command.write_text("#!/bin/bash\n" + body + "\n")
                command.chmod(0o755)
            environment = dict(os.environ, PATH=f"{path}:{os.environ['PATH']}",
                               TEST_LOG=str(log), TEST_VALIDATION_EXIT=str(validation_exit))
            result = subprocess.run(["bash", "scripts/build-rust.sh", *arguments], cwd=ROOT,
                                    env=environment, capture_output=True, text=True)
            return result.returncode, log.read_text() if log.exists() else ""

    def test_formal_public_checks_before_build_and_stops_on_failure(self):
        code, commands = self.run_build("public", "formal", validation_exit=1)
        self.assertEqual(code, 1)
        self.assertIn("--bin validate-ad-release", commands)
        self.assertNotIn("cargo build", commands)
        self.assertNotIn("npm ", commands)

    def test_formal_public_builds_after_successful_validation(self):
        code, commands = self.run_build("public", "formal")
        self.assertEqual(code, 0)
        self.assertLess(commands.index("--bin validate-ad-release"), commands.index("cargo build"))

    def test_default_public_stays_local_and_personal_formal_is_rejected(self):
        code, commands = self.run_build("public")
        self.assertEqual(code, 0)
        self.assertNotIn("--bin validate-ad-release", commands)
        code, commands = self.run_build("personal", "formal")
        self.assertEqual(code, 64)
        self.assertEqual(commands, "")


if __name__ == "__main__":
    unittest.main()
