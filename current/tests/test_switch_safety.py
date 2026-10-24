"""Fail-closed account switching tests with all system commands isolated."""
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "core/claude-switch.sh"


class SwitchSafetyTests(unittest.TestCase):
    def setUp(self):
        (ROOT / "_trash").mkdir(exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(dir=ROOT / "_trash")
        self.addCleanup(self.temp.cleanup)
        self.folder = Path(self.temp.name)
        self.home = self.folder / "home"
        self.bin = self.folder / "bin"
        self.home.mkdir()
        self.bin.mkdir()
        self.open_marker = self.folder / "claude-opened"

        for name in ("pgrep", "ifconfig", "launchctl", "defaults", "networksetup",
                     "dig", "osascript", "pkill", "scutil"):
            self._stub(name, "exit 1\n")
        self._stub("curl", "exit 0\n")
        self._stub("python3", "echo RESULT:NONE\n")
        self._stub("sleep", "exit 0\n")
        self._stub("open", "touch " + shlex.quote(str(self.open_marker)) + "\n")
        self.env = dict(os.environ, HOME=str(self.home),
                        PATH=str(self.bin) + os.pathsep + os.defpath)

    def _stub(self, name, body):
        path = self.bin / name
        path.write_text("#!/bin/sh\n" + body)
        path.chmod(0o755)

    def run_switch(self, target):
        return subprocess.run(["/bin/bash", str(SCRIPT), "switch", target],
                              env=self.env, capture_output=True, text=True, timeout=10)

    def test_rejects_targets_other_than_a_or_b_before_starting_claude(self):
        result = self.run_switch("C")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.open_marker.exists())
        self.assertFalse((self.home / "Library/Application Support/Claude").exists())
        self.assertIn("FAIL:", result.stdout)

    def test_blocks_account_switch_when_preflight_has_failed_checks(self):
        result = self.run_switch("A")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.open_marker.exists())
        self.assertFalse((self.home / "Library/Application Support/Claude").exists())
        self.assertIn("启动被阻止", result.stdout)
        audit_rows = [line for line in result.stdout.splitlines()
                      if line.split("=", 1)[0] in {
                          "代理内核", "TUN网卡", "出口IP", "IP一致", "时区", "地区", "IPv6",
                          "DNS", "WebRTC", "保镖", "规则置顶", "客户端语言", "ASN属性", "生态可达性"}]
        self.assertEqual(len(audit_rows), 14)

    def test_fix_does_not_change_system_settings_without_a_tunnel(self):
        result = subprocess.run(["/bin/bash", str(SCRIPT), "fix"], env=self.env,
                                capture_output=True, text=True, timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("TUN 必须同时在线", result.stdout)
        self.assertFalse((self.home / "Library/Application Support/Claude").exists())

    def test_auto_proxy_detects_odcloud_core(self):
        self._stub("pgrep", '[ "$1" = "-x" ] && [ "$2" = "0dcloudCore" ] && exit 0\nexit 1\n')
        result = subprocess.run(
            ["/bin/bash", "-c", 'source "$1"; PROXY_MODE=auto; proxy_alive', "_", str(SCRIPT)],
            env=self.env, capture_output=True, text=True, timeout=5)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


class GeoLookupTests(unittest.TestCase):
    def test_ip_sb_fallback_supplies_country_and_timezone(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            home = root / "home"
            bin_dir = root / "bin"
            home.mkdir()
            bin_dir.mkdir()
            curl = bin_dir / "curl"
            curl.write_text(
                '#!/bin/sh\n'
                'case "$*" in\n'
                '  *https://ipinfo.io/json*) exit 22 ;;\n'
                '  *https://ipwho.is/*) exit 22 ;;\n'
                '  *https://www.cloudflare.com/cdn-cgi/trace*) exit 22 ;;\n'
                '  *https://api.ip.sb/geoip*) printf \'%s\\n\' \'{"country_code":"US","timezone":"America/Los_Angeles","ip":"203.0.113.10"}\' ;;\n'
                '  *) exit 22 ;;\n'
                'esac\n'
            )
            curl.chmod(0o755)
            (bin_dir / "python3").symlink_to(sys.executable)
            env = dict(os.environ, HOME=str(home),
                       PATH=str(bin_dir) + os.pathsep + os.defpath)
            result = subprocess.run(
                ["/bin/bash", "-c", 'source "$1"; lookup_geo', "_", str(SCRIPT)],
                env=env, capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(result.stdout.strip(), "US\tAmerica/Los_Angeles\t203.0.113.10")
            result = subprocess.run(
                ["/bin/bash", "-c", 'source "$1"; lookup_egress', "_", str(SCRIPT)],
                env=env, capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(result.stdout.strip(), "203.0.113.10\tUS")


class LaunchPreflightTests(unittest.TestCase):
    def run_preflight(self, output):
        with tempfile.TemporaryDirectory() as home:
            env = dict(os.environ, HOME=home, TEST_CHECK_OUTPUT=output)
            return subprocess.run(
                ["/bin/bash", "-c",
                 'source "$1"; checkdetail() { printf "%s\\n" "$TEST_CHECK_OUTPUT"; }; launch_preflight',
                 "_", str(SCRIPT)], env=env, capture_output=True, text=True, timeout=5)

    @staticmethod
    def rows():
        return [
            "代理内核=ok", "TUN网卡=ok", "出口IP=ok", "IP一致=warn:节点变化",
            "时区=ok", "地区=ok", "IPv6=ok", "DNS=ok", "WebRTC=ok",
            "保镖=ok", "规则置顶=ok", "客户端语言=ok", "ASN属性=warn:未获取",
            "生态可达性=ok",
        ]

    def test_warnings_are_allowed_after_all_fourteen_checks_run(self):
        output = "# 2026-10-21 pid=123 开始\n" + "\n".join(self.rows()) + "\n# 结束"
        result = self.run_preflight(output)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("14项环境审计通过", result.stdout)

    def test_missing_check_is_blocked(self):
        rows = self.rows()
        rows.remove("IP一致=warn:节点变化")
        result = self.run_preflight("\n".join(rows))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("审计行数异常(13/14)", result.stdout)
        self.assertIn("缺少或重复检查项:IP一致", result.stdout)

    def test_warning_is_allowed_only_for_ip_history_and_asn(self):
        rows = self.rows()
        rows[1] = "TUN网卡=warn:unexpected"
        result = self.run_preflight("\n".join(rows))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("不允许的警告:TUN网卡", result.stdout)


if __name__ == "__main__":
    unittest.main()
