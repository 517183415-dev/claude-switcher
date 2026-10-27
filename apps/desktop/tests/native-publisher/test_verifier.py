"""Replay a synthetic native receipt; never launch the app or access Keychain."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
FIXTURE = None


class VerifierTests(unittest.TestCase):
    def check_fixture(self, mutate=None):
        with tempfile.TemporaryDirectory() as name:
            out = Path(name)
            for filename in ('webview.json', 'saved.json', 'paused.json', 'wrong-password.json',
                             'restored.json', 'republished.json', 'process.json',
                             'process-result.json', 'runtime.log', 'inputs.json'):
                shutil.copy2(FIXTURE / filename, out / filename)
            shutil.copytree(FIXTURE / 'tmp/business-root', out / 'tmp/business-root')
            # Git records executable bits, not 0600/0700. Reconstruct only this
            # disposable replay copy. The live verifier checked original modes.
            for path in (out / 'tmp').rglob('*'):
                path.chmod(0o700 if path.is_dir() else 0o600)
            if mutate:
                mutate(out)
            result = subprocess.run(['node', str(HERE / 'verify.mjs'), str(out)],
                                    capture_output=True, text=True, timeout=10)
            if mutate:
                self.assertNotEqual(result.returncode, 0, result.stdout)
                self.assertFalse((out / 'receipt.json').exists())
                self.assertIn('AssertionError', result.stderr)
            else:
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(json.loads((out / 'receipt.json').read_text())['responses'], 17)

    @staticmethod
    def change_json(out, filename, mutate):
        path = out / filename
        value = json.loads(path.read_text())
        mutate(value)
        path.write_text(json.dumps(value))

    def test_native_receipt_accepts_verified_fixture(self):
        self.check_fixture()

    def test_substituted_response_is_rejected(self):
        self.check_fixture(lambda out: self.change_json(out, 'webview.json',
            lambda value: value['rows'][0].update(label='substituted')))

    def test_tampered_signature_is_rejected(self):
        def mutate(out):
            path = next((out / 'tmp/business-root/v2/local-ads/production/versions').glob('*.json'))
            envelope = json.loads(path.read_text())
            envelope['signature'][0] ^= 1
            path.write_text(json.dumps(envelope))
        self.check_fixture(mutate)

    def test_changed_previous_generation_is_rejected(self):
        def mutate(out):
            paused = json.loads((out / 'paused.json').read_text())
            generation = paused['active']['generation']
            self.change_json(out, 'restored.json',
                             lambda value: value['states'].update({generation: '0' * 64}))
        self.check_fixture(mutate)

    def test_automatic_publish_during_restore_is_rejected(self):
        self.check_fixture(lambda out: self.change_json(out, 'restored.json',
            lambda value: value['public'].pop('production/current.json')))


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--fixture', required=True, type=Path)
    arguments, remaining = parser.parse_known_args()
    FIXTURE = arguments.fixture.resolve(strict=True)
    unittest.main(argv=[__file__, *remaining])
