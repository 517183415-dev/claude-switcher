"""The receipt oracle must fail when IPC evidence is missing or misclassified."""
import copy
import importlib.util
from pathlib import Path
import unittest
import tempfile
from urllib.request import build_opener, ProxyHandler
from urllib.error import HTTPError
from unittest.mock import patch
import socket

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
        return {"phase": "local", "edition": "public", "rows": rows}

    def check(self, local):
        runner.validate(local, "public", runner.UNKNOWN)

    def test_complete_local_report(self):
        self.check(self.reports())

    def test_missing_duplicate_and_reordered_commands_fail(self):
        for change in [lambda rows: rows.pop(), lambda rows: rows.append(rows[-1]),
                       lambda rows: rows.reverse()]:
            local = self.reports()
            change(local["rows"])
            with self.assertRaises(AssertionError):
                self.check(local)

    def test_not_found_does_not_count_as_acl_denial_or_argument_validation(self):
        for index in [len(runner.SUCCESS), -1]:
            local = self.reports()
            local["rows"][index]["error"] = "Command not found"
            with self.assertRaises(AssertionError):
                self.check(local)

    def test_unexpected_success_in_local_denials_fails(self):
        local = self.reports()
        local["rows"][-1]["ok"] = True
        with self.assertRaises(AssertionError):
            self.check(local)

    def test_wrong_edition_or_persisted_preferences_fail(self):
        for index, key, value in [(0, "edition", "personal"), (3, "ads", {}),
                                  (4, "preferences", {})]:
            local = self.reports()
            local["rows"][index]["value"][key] = value
            with self.assertRaises(AssertionError):
                self.check(local)



class BoundaryReceiptTests(unittest.TestCase):
    def report(self):
        return {"phase": "remote", "edition": "public", "url": "http://127.0.0.1:12345/",
                "bridge": True, "rows": [{"command": c, "ok": False, "error": "not allowed by ACL"}
                                         for c in ["runtime_info", "workspace_snapshot"]]}

    def check(self, report):
        runner.validate_boundary(report, "public", "remote", "http://127.0.0.1:12345/",
                                 ["runtime_info", "workspace_snapshot"])

    def test_loopback_server_stops_after_exception_and_serves_only_assets(self):
        # A loopback test must not send its request through inherited proxies.
        direct = build_opener(ProxyHandler({}))
        with self.assertRaisesRegex(RuntimeError, "fixture failure"), patch.dict(
                runner.os.environ, {"HTTP_PROXY": "http://127.0.0.1:1",
                                    "HTTPS_PROXY": "http://127.0.0.1:1", "NO_PROXY": ""}):
            with runner.boundary_server("public") as server:
                url = server["url"]
                with direct.open(url, timeout=2) as response:
                    self.assertIn(b'"remote"', response.read().replace(b'&quot;', b'"'))
                with direct.open(url + "probe.js", timeout=2) as response:
                    self.assertEqual(response.read(), (runner.HERE / "probe.js").read_bytes())
                try:
                    direct.open(url + "../run.py", timeout=2)
                    self.fail("unexpected server asset")
                except HTTPError as rejected:
                    self.assertEqual(rejected.code, 404)
                    rejected.close()
                port = runner.urlsplit(url).port
                raise RuntimeError("fixture failure")
        with socket.socket() as connection:
            self.assertNotEqual(connection.connect_ex(("127.0.0.1", port)), 0)

    def test_reuse_binds_exact_previous_port_and_refuses_busy_port(self):
        with runner.boundary_server("public") as first:
            port = runner.urlsplit(first["url"]).port
            with self.assertRaises(OSError):
                with runner.boundary_server("public", port):
                    self.fail("must not replace another listener")
        with runner.boundary_server("public", port) as reused:
            self.assertEqual(reused["url"], first["url"])

    def test_complete_boundary_report(self):
        self.check(self.report())

    def test_missing_bridge_wrong_context_or_wrong_origin_cannot_pass(self):
        for key, value in [("bridge", False), ("phase", "other"), ("edition", "personal"),
                           ("url", "tauri://localhost/")]:
            report = self.report()
            report[key] = value
            with self.assertRaises(AssertionError):
                self.check(report)

    def test_incomplete_reordered_successful_or_wrong_failure_cannot_pass(self):
        for change in [lambda rows: rows.pop(), lambda rows: rows.reverse(),
                       lambda rows: rows[-1].update(ok=True),
                       lambda rows: rows[-1].update(error="invalid args: key"),
                       lambda rows: rows[-1].update(error="Command not found")]:
            report = self.report()
            change(report["rows"])
            with self.assertRaises(AssertionError):
                self.check(report)

    def test_commands_match_edition_permission_counts(self):
        for edition, count in [("public", 23), ("personal", 48)]:
            commands = runner.boundary_commands(edition)
            self.assertEqual(len(commands), count)
            self.assertEqual(len(set(commands)), count)
            self.assertEqual(commands[0], "runtime_info")


if __name__ == "__main__":
    unittest.main()
