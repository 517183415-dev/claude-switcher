import http.client
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import threading
import unittest

spec = importlib.util.spec_from_file_location('publisher_native_runner', Path(__file__).with_name('runner.py'))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class IsolationTests(unittest.TestCase):
    def test_native_fixture_root_is_private_and_exists_before_launch(self):
        with tempfile.TemporaryDirectory() as name:
            out = Path(name) / 'run'
            home, tmp, dist = runner.prepare_paths(out)
            root = tmp / 'business-root'
            for path in (home, tmp, dist, root):
                self.assertTrue(path.is_dir())
                self.assertEqual(path.stat().st_mode & 0o777, 0o700)
            self.assertEqual(root.resolve(), tmp.resolve() / 'business-root')
            with self.assertRaises(FileExistsError):
                runner.prepare_paths(out)

    def test_private_file_rejects_wide_permissions_and_symlinks(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            value = root / 'value'
            value.write_bytes(b'synthetic')
            value.chmod(0o644)
            with self.assertRaises(AssertionError):
                runner.private_file(root, value)
            value.chmod(0o600)
            self.assertEqual(runner.private_file(root, value), b'synthetic')
            link = root / 'link'
            link.symlink_to(value)
            with self.assertRaises(AssertionError):
                runner.private_file(root, link)
            with self.assertRaises(AssertionError):
                runner.private_file(root, value, limit=1)

    def test_receiver_requires_token_and_refuses_paths_and_malformed_json(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            with runner.Receiver(root, root) as server:
                thread = threading.Thread(target=server.serve_forever)
                thread.start()
                try:
                    def post(route, body, token=None):
                        connection = http.client.HTTPConnection('127.0.0.1', server.server_port, timeout=3)
                        try:
                            connection.request('POST', route, body, {'X-Fixture-Token': token or 'invalid'})
                            response = connection.getresponse()
                            response.read()
                            return response.status
                        finally:
                            connection.close()
                    self.assertEqual(post('/finish', '{}'), 403)
                    self.assertEqual(post('/unknown', '{}', server.token), 404)
                    self.assertEqual(post('/checkpoint/../../outside', '{}', server.token), 400)
                    self.assertEqual(post('/finish', '{broken', server.token), 400)
                    self.assertFalse(server.finished.is_set())
                    self.assertFalse(list(root.iterdir()))
                finally:
                    server.shutdown()
                    thread.join(timeout=3)
                    self.assertFalse(thread.is_alive())


if __name__ == '__main__':
    unittest.main()
