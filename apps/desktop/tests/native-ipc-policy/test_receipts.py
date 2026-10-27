"""The receipt oracle must fail when IPC evidence is missing or misclassified."""
import copy
import importlib.util
from pathlib import Path
import unittest
import tempfile

spec = importlib.util.spec_from_file_location("ipc_policy_runner", Path(__file__).with_name("run.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class ReceiptTests(unittest.TestCase):
    def test_synthetic_home_has_private_intermediate_directories(self):
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary)
            preferences = runner.prepare_home(home, "public")
            directory = home
            for name in [".claude-switcher", "v2", "public"]:
                directory /= name
                self.assertEqual(directory.stat().st_mode & 0o777, 0o700)
            self.assertEqual(preferences.stat().st_mode & 0o777, 0o600)

    def reports(self):
        values = [{"edition": "public"}, {}, {}, runner.PREFERENCES,
                  {"preferences": runner.PREFERENCES, "runtime": {"edition": "public"}}, [], ["export", []]]
        rows = [{"command": command, "ok": True, "value": copy.deepcopy(value)}
                for command, value in zip(runner.SUCCESS, values)]
        rows += [{"command": command, "ok": False, "error": "invalid args: missing key"}
                 for command in runner.INVALID]
        rows += [{"command": command, "ok": False, "error": "not allowed by ACL"}
                 for command in runner.UNKNOWN]
        return ({"phase": "local", "edition": "public", "rows": rows},
                {"phase": "remote", "edition": "public", "bridge": True,
                 "rows": [{"command": "runtime_info", "ok": False, "error": "not allowed by ACL"}]})

    def check(self, local, remote):
        runner.validate(local, remote, "public", runner.UNKNOWN)

    def test_complete_local_and_remote_denial(self):
        self.check(*self.reports())

    def test_missing_duplicate_and_reordered_commands_fail(self):
        for change in [lambda rows: rows.pop(), lambda rows: rows.append(rows[-1]),
                       lambda rows: rows.reverse()]:
            local, remote = self.reports()
            change(local["rows"])
            with self.assertRaises(AssertionError):
                self.check(local, remote)

    def test_not_found_does_not_count_as_acl_denial_or_argument_validation(self):
        for index in [len(runner.SUCCESS), -1]:
            local, remote = self.reports()
            local["rows"][index]["error"] = "Command not found"
            with self.assertRaises(AssertionError):
                self.check(local, remote)

    def test_unexpected_success_in_local_or_remote_denials_fails(self):
        for phase in ["local", "remote"]:
            local, remote = self.reports()
            (local if phase == "local" else remote)["rows"][-1]["ok"] = True
            with self.assertRaises(AssertionError):
                self.check(local, remote)

    def test_wrong_edition_or_persisted_preferences_fail(self):
        for index, key, value in [(0, "edition", "personal"), (3, "ads", {}),
                                  (4, "preferences", {})]:
            local, remote = self.reports()
            local["rows"][index]["value"][key] = value
            with self.assertRaises(AssertionError):
                self.check(local, remote)

    def test_absent_remote_bridge_is_reported_separately(self):
        local, remote = self.reports()
        remote.update(bridge=False, rows=[{"ok": False, "error": "bridge_not_injected"}])
        self.check(local, remote)
        remote["rows"][0]["error"] = "network timeout"
        with self.assertRaises(AssertionError):
            self.check(local, remote)


if __name__ == "__main__":
    unittest.main()
