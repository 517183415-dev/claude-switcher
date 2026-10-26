import io
import copy
import argparse
import pwd
from unittest.mock import patch
import json
import os
from pathlib import Path
import tempfile
import unittest
from contextlib import redirect_stderr
import runner
import support
import build_native


class BusinessReceiptTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='business-parser-')
        self.addCleanup(self.temp.cleanup)
        self.folder = Path(self.temp.name).resolve()
        (self.folder / 'home').mkdir(mode=0o700)
        self.root = self.folder / 'tmp/business-root'
        self.root.mkdir(parents=True, mode=0o700)
        self.result = {'fixture': 'ipc-business', 'ordinary_runtime': False,
                       'edition': 'personal', 'preflight': True, 'ok': True,
                       'responses': {}, 'denials': [{'command': command, 'kind': 'permission' if index == 0 else 'missing'} for index, command in enumerate(runner.DENIALS)], 'error': None}

    def validate(self, mode='preflight', code=0):
        return runner.validate('BUSINESS_IPC_RESULT ' + json.dumps(self.result, ensure_ascii=False), code, 'personal', mode, self.folder)

    def business(self):
        # Independent literal oracle. Do not generate the expected output using production redact.
        message = '正常业务\nIP [网络地址已隐藏]\nIPv6 [网络地址已隐藏]\n[网络地址已隐藏]\n[敏感字段已隐藏]\n路径 [本机目录]/synthetic-profile'
        entries = [{'observed_at': 1700000000, 'level': 'info', 'message': message},
                   {'observed_at': 1, 'level': 'warning', 'message': message}]
        directory = self.root / 'v2/personal'
        directory.mkdir(parents=True, mode=0o700)
        os.chmod(self.root / 'v2', 0o700)
        exports = directory / 'exports'
        exports.mkdir(mode=0o700)
        path = exports / 'activity-redacted.json'
        path.write_text(json.dumps(entries)); path.chmod(0o600)
        raw = '正常业务\nIP 203.0.113.9\nIPv6 2001:db8::1\nhttps://private.example/path\nCookie: BUSINESS_SYNTHETIC_ONLY\n路径 ' + str(self.folder / 'home') + '/synthetic-profile'
        activity = [entries[0], dict(entries[1], message=raw)]
        (directory / 'activity.json').write_text(json.dumps(activity)); (directory / 'activity.json').chmod(0o600)
        self.result.update(preflight=False, responses={'preview': entries, 'exported': [str(path), entries], 'after': entries})
        return directory, path

    def preferences(self, edition='personal'):
        # Synthetic decoder evidence only. Production IPC must separately reproduce it.
        directory = self.root / 'v2' / edition
        directory.mkdir(parents=True, mode=0o700)
        (self.root / 'v2').chmod(0o700)
        entries = [{'observed_at': 1700000000, 'level': 'info', 'message': runner.EXPECTED_MESSAGE},
                   {'observed_at': 1, 'level': 'warning', 'message': runner.RAW_PREFIX + str(self.folder / 'home') + '/synthetic-profile'}]
        state = {'preferences': None, 'activity': list(json.dumps(entries, ensure_ascii=False).encode())}
        self.snapshots = []
        steps = []
        for index, (name, request, accepted) in enumerate(runner.preference_cases()):
            self.snapshots.append(dict(copy.deepcopy(state), label=name + ':before'))
            if accepted:
                entries[1]['message'] = runner.EXPECTED_MESSAGE
                entries.append({'observed_at': 1700000000 + index, 'level': 'info',
                                'message': '已保存本机设置；全部广告关闭时不发送广告请求'})
                state = {'preferences': list(json.dumps(request['preferences']).encode()),
                         'activity': list(json.dumps(entries, ensure_ascii=False).encode())}
            self.snapshots.append(dict(copy.deepcopy(state), label=name + ':after'))
            steps.append({'id': name, 'input': request, 'accepted': accepted,
                          'value': copy.deepcopy(request['preferences']) if accepted else None,
                          'error': None if accepted else ('请先确认广告请求用途，再开启广告位' if name.startswith('unconfirmed_') else 'invalid args for command save_preferences')})
        for name, value in state.items():
            path = directory / (name + '.json')
            path.write_bytes(bytes(value)); path.chmod(0o600)
        self.result.update(edition=edition, preflight=False, responses={'preferences': steps})
        self.preference_edition = edition
        return directory

    def validate_preferences(self):
        stdout = '\n'.join('BUSINESS_IPC_PREFERENCES ' + json.dumps(item) for item in self.snapshots)
        stdout += '\nBUSINESS_IPC_RESULT ' + json.dumps(self.result)
        return runner.validate(stdout, 0, self.preference_edition, 'preferences', self.folder)

    def test_preferences_personal_matrix_accepts_five_and_rejects_seventeen(self):
        self.preferences()
        result = self.validate_preferences()
        self.assertEqual((result['accepted'], result['rejected'], result['snapshot_count']), (5, 17, 44))

    def test_preferences_public_uses_only_public_directory(self):
        self.preferences('public')
        self.assertTrue(self.validate_preferences()['edition_isolation'])
        self.assertFalse((self.root / 'v2/personal').exists())

    def test_preferences_rejected_write_to_preference_bytes_fails(self):
        self.preferences()
        self.snapshots[3]['preferences'].append(32)
        with self.assertRaises(AssertionError): self.validate_preferences()

    def test_preferences_rejected_write_to_activity_bytes_fails(self):
        self.preferences()
        self.snapshots[3]['activity'].append(32)
        with self.assertRaises(AssertionError): self.validate_preferences()

    def test_preferences_return_must_match_saved_request(self):
        self.preferences()
        self.result['responses']['preferences'][0]['value']['ads']['slots_enabled'][0] = True
        with self.assertRaises(AssertionError): self.validate_preferences()

    def test_preferences_missing_snapshot_fails(self):
        self.preferences(); self.snapshots.pop()
        with self.assertRaises(AssertionError): self.validate_preferences()

    def test_preferences_missing_required_field_case_fails(self):
        self.preferences(); self.result['responses']['preferences'].pop(10)
        with self.assertRaises(AssertionError): self.validate_preferences()

    def test_preferences_unconfirmed_slot_must_return_business_error(self):
        self.preferences(); self.result['responses']['preferences'][1]['error'] = 'missing command save_preferences'
        with self.assertRaises(AssertionError): self.validate_preferences()

    def test_preferences_final_disk_must_match_snapshot(self):
        directory = self.preferences(); (directory / 'preferences.json').write_text('{}')
        with self.assertRaises(AssertionError): self.validate_preferences()

    def test_preferences_final_file_requires_private_mode(self):
        directory = self.preferences(); (directory / 'preferences.json').chmod(0o644)
        with self.assertRaises(AssertionError): self.validate_preferences()

    def test_preferences_rejects_cross_edition_side_effect(self):
        self.preferences(); (self.root / 'v2/public').mkdir(mode=0o700)
        with self.assertRaises(AssertionError): self.validate_preferences()

    def test_preferences_snapshot_rejects_bool_as_byte(self):
        self.preferences(); self.snapshots[0]['activity'][0] = True
        with self.assertRaises(AssertionError): self.validate_preferences()

    def test_preferences_oracle_distinguishes_boolean_from_number(self):
        self.preferences(); self.result['responses']['preferences'][0]['value']['ads']['request_purpose_acknowledged'] = 0
        with self.assertRaises(AssertionError): self.validate_preferences()

    def test_preflight_accepts_only_empty_business_responses(self):
        self.assertEqual(self.validate()['status'], 'PASS')

    def test_preflight_rejects_business_command_results(self):
        self.result['responses'] = {'preview': []}
        with self.assertRaises(AssertionError): self.validate()

    def test_preflight_rejects_business_files(self):
        (self.root / 'unexpected').write_text('synthetic')
        with self.assertRaises(AssertionError): self.validate()

    def test_result_rejects_nonzero_exit_even_when_js_reports_success(self):
        with self.assertRaises(AssertionError): self.validate(code=1)

    def test_result_rejects_ordinary_runtime(self):
        self.result['ordinary_runtime'] = True
        with self.assertRaises(AssertionError): self.validate()

    def test_registered_denial_requires_permission_error(self):
        self.result['denials'][0]['kind'] = 'missing'
        with self.assertRaises(AssertionError): self.validate()

    def test_result_rejects_reordered_denials(self):
        self.result['denials'].reverse()
        with self.assertRaises(AssertionError): self.validate()

    def test_result_rejects_duplicate_completion(self):
        line = 'BUSINESS_IPC_RESULT ' + json.dumps(self.result)
        with self.assertRaises(AssertionError): runner.validate(line + '\n' + line, 0, 'personal', 'preflight', self.folder)

    def test_business_accepts_independent_oracle_and_raw_historical_control(self):
        self.business()
        self.assertTrue(self.validate('business')['historical_raw_positive_control'])

    def test_business_rejects_unredacted_ip_in_actual_ipc_response(self):
        self.business()
        self.result['responses']['preview'][0]['message'] = '203.0.113.9'
        with self.assertRaises(AssertionError): self.validate('business')

    def test_business_rejects_changed_export_path(self):
        self.business()
        self.result['responses']['exported'][0] = str(self.root / 'unexpected.json')
        with self.assertRaises(AssertionError): self.validate('business')

    def test_business_rejects_disk_export_mismatch(self):
        _, path = self.business(); path.write_text('[]')
        with self.assertRaises(AssertionError): self.validate('business')

    def test_business_rejects_public_export_file_mode(self):
        _, path = self.business(); path.chmod(0o644)
        with self.assertRaises(AssertionError): self.validate('business')

    def test_business_rejects_public_export_directory_mode(self):
        _, path = self.business(); path.parent.chmod(0o755)
        with self.assertRaises(AssertionError): self.validate('business')

    def test_business_rejects_export_hardlink(self):
        _, path = self.business(); os.link(path, path.parent / 'other.json')
        with self.assertRaises(AssertionError): self.validate('business')

    def test_business_rejects_export_symlink(self):
        _, path = self.business(); target = path.parent / 'target.json'; path.rename(target); path.symlink_to(target)
        with self.assertRaises(AssertionError): self.validate('business')

    def test_business_rejects_removed_historical_raw_control(self):
        directory, _ = self.business()
        (directory / 'activity.json').write_text(json.dumps(self.result['responses']['preview']))
        with self.assertRaises(AssertionError): self.validate('business')

    def test_attempt_rejects_path_traversal(self):
        with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            runner.parse_args(['run', '--output', str(self.folder.resolve()), '--edition', 'personal', '--mode', 'preflight', '--build-attempt', '../r1'])

    def test_run_requires_all_explicit_arguments(self):
        with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit): runner.parse_args(['run', '--output', str(self.folder.resolve())])

    def test_output_rejects_existing_nonprivate_directory(self):
        path = self.folder / 'public-output'
        path.mkdir(mode=0o755)
        with self.assertRaises(argparse.ArgumentTypeError): support.output_root(str(path.resolve()))

    def test_output_rejects_symlink(self):
        path = self.folder / 'linked-output'
        path.symlink_to(self.folder, target_is_directory=True)
        with self.assertRaises(argparse.ArgumentTypeError): support.output_root(str(path))

    def test_output_rejects_repository_source_directory(self):
        with self.assertRaises(argparse.ArgumentTypeError): support.output_root(str(support.ROOT / 'apps'))

    def test_run_directory_preserves_existing_evidence(self):
        previous = runner.OUT
        self.addCleanup(setattr, runner, 'OUT', previous)
        runner.OUT = self.folder.resolve()
        first = runner.new_run('unit')
        marker = first / 'historical.json'
        marker.write_text('unchanged')
        second = runner.new_run('unit')
        self.assertNotEqual(first, second)
        self.assertEqual(marker.read_text(), 'unchanged')

    def test_profile_denies_all_network_without_localhost_exception(self):
        folder = self.folder.resolve()
        path = support.make_profile(folder, folder)
        policy = path.read_text()
        self.assertIn('(deny network*)', policy)
        self.assertNotIn('(allow network', policy)
        self.assertIn('(subpath ' + json.dumps(str(Path.home() / 'Library')) + ')', policy)

    def test_binding_detects_new_harness_input(self):
        previous = support.ROOT
        self.addCleanup(setattr, support, 'ROOT', previous)
        support.ROOT = self.folder
        (self.folder / 'Cargo.toml').write_text('synthetic')
        (self.folder / 'Cargo.lock').write_text('synthetic')
        directory = self.folder / 'apps/desktop/tests/native-ipc-business'
        directory.mkdir(parents=True)
        before = support.inputs()
        (directory / 'new-case.py').write_text('synthetic')
        self.assertNotEqual(support.inputs(), before)
        self.assertIn('apps/desktop/tests/native-ipc-business/new-case.py', support.inputs())

    def test_profile_generation_refuses_existing_policy(self):
        folder = self.folder.resolve()
        path = support.make_profile(folder, folder)
        before = path.read_bytes()
        with self.assertRaises(FileExistsError): support.make_profile(folder, folder)
        self.assertEqual(path.read_bytes(), before)

    def test_build_profile_allows_toolchain_libraries_read_only(self):
        folder = self.folder.resolve()
        tools = folder / '.rustup/toolchains/synthetic/bin'
        policy = support.make_profile(folder, folder, build=True, tools=tools,
                                      cache=folder / 'cache', target=folder / 'target').read_text()
        self.assertIn('(allow file-read* (subpath ' + json.dumps(str(tools.parent)) + '))', policy)
        self.assertNotIn('(allow file-write* (subpath ' + json.dumps(str(tools.parent)) + '))', policy)

    def build_paths(self):
        folder = self.folder.resolve()
        tools = folder / '.rustup/toolchains/synthetic/bin'
        tools.mkdir(parents=True)
        cache = folder / 'cache'
        cache.mkdir()
        return folder, tools, cache

    def test_build_target_rejects_user_documents(self):
        folder, tools, cache = self.build_paths()
        with self.assertRaises(ValueError): support.validate_build_paths(folder, tools, cache, Path(pwd.getpwuid(os.geteuid()).pw_dir) / 'Documents')

    def test_build_toolchain_rejects_broad_home_parent(self):
        folder, _, cache = self.build_paths()
        with self.assertRaises(ValueError): support.validate_build_paths(folder, folder / 'bin', cache, folder / 'target')

    def test_build_registry_rejects_link_to_home(self):
        folder, tools, cache = self.build_paths()
        (cache / 'registry').symlink_to(Path(pwd.getpwuid(os.geteuid()).pw_dir), target_is_directory=True)
        with self.assertRaises(ValueError): support.validate_build_paths(folder, tools, cache, folder / 'target')

    def test_private_directory_rejects_build_symlink_before_creation(self):
        folder = self.folder.resolve()
        outside = folder / 'outside'
        outside.mkdir(mode=0o700)
        (folder / 'build').symlink_to(outside, target_is_directory=True)
        with self.assertRaises(ValueError): support.private_directory(folder / 'build', create=True)
        self.assertEqual(list(outside.iterdir()), [])

    def test_safe_file_rejects_symlink_receipt(self):
        target = self.folder / 'target.json'
        target.write_text('{}')
        link = self.folder / 'result.json'
        link.symlink_to(target)
        with self.assertRaises(ValueError): support.safe_file(link)

    def test_profile_denies_account_home_when_environment_home_is_forged(self):
        folder = self.folder.resolve()
        account_home = Path(pwd.getpwuid(os.geteuid()).pw_dir)
        with patch.dict(os.environ, {'HOME': str(folder / 'forged-home')}):
            policy = support.make_profile(folder, folder).read_text()
        self.assertIn('(deny file-read* (subpath ' + json.dumps(str(account_home)) + '))', policy)
        self.assertIn('(subpath ' + json.dumps(str(account_home / 'Library')) + ')', policy)

    def test_source_binding_rejects_linked_manifest(self):
        previous = support.ROOT
        self.addCleanup(setattr, support, 'ROOT', previous)
        support.ROOT = self.folder.resolve()
        (self.folder / 'outside').write_text('synthetic')
        (self.folder / 'Cargo.toml').symlink_to(self.folder / 'outside')
        (self.folder / 'Cargo.lock').write_text('synthetic')
        with self.assertRaises(ValueError): support.inputs()

    def test_safe_file_rejects_hardlinked_receipt(self):
        target = self.folder / 'receipt.json'
        target.write_text('{}')
        os.link(target, self.folder / 'second.json')
        with self.assertRaises(ValueError): support.safe_file(target)

    def test_private_directory_rejects_nonprivate_attempt(self):
        path = self.folder / 'personal-r1'
        path.mkdir(mode=0o755)
        with self.assertRaises(ValueError): support.private_directory(path)

    def test_source_binding_rejects_linked_fixture_directory(self):
        previous = support.ROOT
        self.addCleanup(setattr, support, 'ROOT', previous)
        support.ROOT = self.folder.resolve()
        (self.folder / 'Cargo.toml').write_text('synthetic')
        (self.folder / 'Cargo.lock').write_text('synthetic')
        directory = self.folder / 'crates'
        directory.mkdir()
        (directory / 'linked').symlink_to(self.folder / 'elsewhere', target_is_directory=True)
        with self.assertRaises(ValueError): support.inputs()

    def test_configuration_rejects_link_before_reading_target(self):
        previous = build_native.ROOT
        self.addCleanup(setattr, build_native, 'ROOT', previous)
        build_native.ROOT = self.folder
        directory = self.folder / 'packaging/editions'
        directory.mkdir(parents=True)
        target = self.folder / 'external.json'
        target.write_text('{}')
        (directory / 'ipc-business-personal.json').symlink_to(target)
        with self.assertRaises(ValueError): build_native.configuration('personal')

    def test_receipt_rejects_external_profile_before_hash_read(self):
        folder = self.folder.resolve()
        previous = runner.OUT
        self.addCleanup(setattr, runner, 'OUT', previous)
        runner.OUT = folder
        build = folder / 'build'
        build.mkdir(mode=0o700)
        attempt = build / 'personal-r1'
        attempt.mkdir(mode=0o700)
        binary = attempt / 'switcher-desktop'
        binary.write_text('synthetic')
        external = folder / 'external-profile'
        external.write_text('DO_NOT_READ')
        record = {'status': 'PASS_BUILD_AND_ADHOC_STATIC_VERIFY', 'edition': 'personal',
                  'binary': str(binary), 'signed_sha256': runner.sha(binary),
                  'source_before': {}, 'source_after': {}, 'sandbox': str(external), 'profile_sha256': 'unused'}
        runner.write(attempt / 'result.json', record)
        original = runner.sha
        def guarded_hash(path):
            if Path(path) == external:
                raise RuntimeError('attempted external profile content read')
            return original(path)
        with patch.object(runner, 'sha', guarded_hash):
            self.assertFalse(runner.run('personal', 'preflight', 'r1'))

    def test_business_mode_is_preserved_by_parser(self):
        args = runner.parse_args(['run', '--output', str(self.folder.resolve()), '--edition', 'public', '--mode', 'business', '--build-attempt', 'r1'])
        self.assertEqual((args.mode, args.edition, args.build_attempt), ('business', 'public', 'r1'))


if __name__ == '__main__':
    unittest.main(verbosity=2)
