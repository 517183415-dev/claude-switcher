import importlib.util
import json
from pathlib import Path
import plistlib
import subprocess
import shlex
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('build_pkg', ROOT/'build_pkg.py')
pkg = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pkg)

class PackagingTests(unittest.TestCase):
    def test_all_generated_scripts_have_valid_shell_syntax(self):
        with tempfile.TemporaryDirectory() as temporary:
            for restore, uninstall in [(False, False), (True, False), (False, True)]:
                for name, text in pkg.scripts(501, 'TESTTEAM01', restore, uninstall).items():
                    path = Path(temporary)/f'{restore}-{uninstall}-{name}'
                    path.write_text(text)
                    subprocess.run(['/bin/sh', '-n', str(path)], check=True)
                    self.assertIn('/dev/console', text)
                    self.assertNotIn('~/', text)
                    self.assertNotIn('accounts/', text)
    def test_recovery_codesign_receives_inline_requirement_in_actual_shell_argv(self):
        with tempfile.TemporaryDirectory() as temporary:
            stub = Path(temporary)/"codesign stub"
            stub.write_text("#!/bin/sh\nprintf '%s\\0' \"$@\"\n")
            stub.chmod(0o700)
            script = pkg.scripts(501, 'TESTTEAM01', restore=True)['postinstall']
            commands = [line for line in script.splitlines() if line.startswith('/usr/bin/codesign ')]
            self.assertEqual(len(commands), 1)
            # Only the generated verifier line runs, never install/launchctl or root writes.
            command = commands[0].replace('/usr/bin/codesign', shlex.quote(str(stub)), 1)
            result = subprocess.run(['/bin/sh', '-c', command], check=True, capture_output=True)
            args = result.stdout.decode().split('\0')[:-1]
            self.assertEqual(args, ['--verify', '--strict', '--test-requirement',
                '=anchor apple generic and certificate leaf[subject.OU] = "TESTTEAM01" and identifier "local.claudeswitcher.helper"',
                '/Library/PrivilegedHelperTools/local.claudeswitcher.helper'])
    def test_services_and_root_helper_paths_are_fixed(self):
        for name in ['helper', 'guardian']:
            with (ROOT/f'launchd/local.claudeswitcher.{name}.plist').open('rb') as stream:
                data = plistlib.load(stream)
            self.assertEqual(data['Label'], f'local.claudeswitcher.{name}')
            self.assertTrue(data['ProgramArguments'][0].startswith('/Library/'))
        self.assertEqual(data['KeepAlive'], {'SuccessfulExit': False})
    def test_plan_is_read_only_and_owner_bound(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)/'does-not-exist'
            command = ['python3', str(ROOT/'build_pkg.py'), '--team', 'TESTTEAM01',
                       '--application-identity', 'Developer ID Application: Test (TESTTEAM01)',
                       '--installer-identity', 'Developer ID Installer: Test (TESTTEAM01)',
                       '--owner-uid', '501', '--edition', 'personal', '--helper', '/missing/helper',
                       '--guardian', '/missing/guardian', '--output', str(output)]
            plan = json.loads(subprocess.check_output(command+['--plan'], text=True))
            self.assertFalse(plan['system_modified'])
            self.assertEqual(plan['owner_uid'], 501)
            self.assertFalse(output.exists())
            failure = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(failure.returncode, 0)
            self.assertIn('signing_identity_unavailable', failure.stderr)
            self.assertFalse(output.exists())

if __name__ == '__main__':
    unittest.main()
