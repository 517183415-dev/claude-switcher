"""Native WKWebView + shared product registry; never a real account or Keychain."""
import argparse
import hashlib
import html
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import secrets
import shutil
import signal
import stat
import subprocess
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
TOOL = Path('/Users/gua/.rustup/toolchains/1.96.0-aarch64-apple-darwin/bin')
spec = importlib.util.spec_from_file_location('native_policy', HERE.parent / 'native-ipc-policy/run.py')
POLICY = importlib.util.module_from_spec(spec)
spec.loader.exec_module(POLICY)
PHASES = ('saved', 'paused', 'wrong-password', 'restored', 'republished')


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def private_file(root, path, limit=64 * 1024 * 1024):
    path.relative_to(root)
    for parent in (path, *path.parents):
        if parent == root.parent:
            break
        metadata = parent.lstat()
        assert not stat.S_ISLNK(metadata.st_mode)
        assert metadata.st_uid == os.geteuid() and metadata.st_mode & 0o077 == 0
    assert path.is_file() and path.stat().st_size <= limit
    return path.read_bytes()


def checkpoint(root):
    publisher = root / 'v2/personal/publisher'
    active = json.loads(private_file(root, publisher / 'active.json'))
    assert len(active['generation']) == 32 and all(c in '0123456789abcdef' for c in active['generation'])
    state = json.loads(private_file(root, publisher / active['generation'] / 'state.json'))
    states = {p.parent.name: hashlib.sha256(private_file(root, p)).hexdigest()
              for p in publisher.glob('*/state.json')}
    public = {}
    for p in (root / 'v2/local-ads').rglob('*'):
        assert not p.is_symlink()
        if p.is_file():
            public[str(p.relative_to(root / 'v2/local-ads'))] = {
                'sha256': hashlib.sha256(private_file(root, p)).hexdigest(), 'mode': stat.S_IMODE(p.stat().st_mode)}
    return {'active': active, 'state': state, 'states': states, 'public': public}


class Receiver(ThreadingHTTPServer):
    def __init__(self, root, out, port=0, token=None):
        super().__init__(('127.0.0.1', port), Handler)
        self.root, self.out, self.token = root, out, token or secrets.token_hex(24)
        self.finished = threading.Event()
        self.failure = None


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def reply(self, code, body=b'{}'):
        self.send_response(code)
        self.send_header('Access-Control-Allow-Origin', 'tauri://localhost')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type, X-Fixture-Token')
        self.send_header('Access-Control-Allow-Methods', 'POST, OPTIONS')
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Cache-Control', 'no-store')
        self.end_headers()
        self.wfile.write(body)

    def do_OPTIONS(self):
        self.reply(204, b'')

    def do_POST(self):
        if self.headers.get('X-Fixture-Token') != self.server.token:
            self.reply(403)
            return
        try:
            size = int(self.headers.get('Content-Length', '-1'))
            assert 0 <= size <= 2 * 1024 * 1024
            body = json.loads(self.rfile.read(size))
            if self.path == '/backup':
                files = list((self.server.root / 'v2/personal/publisher/backups').glob('*.csbackup'))
                assert len(files) == 1
                self.reply(200, private_file(self.server.root, files[0]))
                return
            if self.path.startswith('/checkpoint/'):
                phase = self.path.removeprefix('/checkpoint/')
                assert phase in PHASES and not (self.server.out / (phase + '.json')).exists()
                save(self.server.out / (phase + '.json'), checkpoint(self.server.root))
            elif self.path == '/finish':
                assert isinstance(body, dict) and isinstance(body.get('rows'), list)
                assert not self.server.finished.is_set()
                save(self.server.out / 'webview.json', body)
                self.server.finished.set()
            else:
                self.reply(404)
                return
            self.reply(200)
        except Exception as error:
            self.server.failure = type(error).__name__
            self.reply(400)


def prepare_paths(out):
    out.mkdir(parents=True, exist_ok=False)
    home, tmp, dist = out / 'home', out / 'tmp', out / 'dist'
    for directory in (home, tmp, dist, tmp / 'business-root'):
        directory.mkdir(mode=0o700)
    return home, tmp, dist


def execute(out, reuse=None, prepare_frontend=None, additional_inputs=()):
    out = out.resolve()
    out.relative_to(ROOT / 'output')
    home, tmp, dist = prepare_paths(out)
    root = tmp / 'business-root'
    previous_plan = json.loads((reuse / 'plan.json').read_text()) if reuse else None
    port = urlsplit(previous_plan['endpoint']).port if reuse else 0
    with Receiver(root, out, port, previous_plan['token'] if reuse else None) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            plan = {'endpoint': f'http://127.0.0.1:{server.server_port}', 'token': server.token}
            save(out / 'plan.json', plan)
            if prepare_frontend:
                prepare_frontend(out, dist, plan)
            else:
                (dist / 'index.html').write_text('<!doctype html><html><meta charset="utf-8"><body data-plan="'
                    + html.escape(json.dumps(plan), quote=True) + '">正在验证正式运营 IPC…<script src="probe.js"></script></body></html>')
                shutil.copy2(HERE / 'probe.js', dist / 'probe.js')
                shutil.copy2(HERE.parent / 'native-ipc-business/backup-source.png', dist / 'image.png')
            config = json.loads((ROOT / 'packaging/editions/personal.json').read_text())
            config['build'] = {'frontendDist': str(dist)}
            # Only the test frontend can contact this random local receipt endpoint.
            config['app']['security']['csp'] = "default-src 'self' tauri:; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost " + plan['endpoint']
            if reuse:
                assert plan == previous_plan
                assert digest(dist / 'probe.js') == digest(reuse / 'dist/probe.js')
                assert digest(dist / 'index.html') == digest(reuse / 'dist/index.html')
                prior_inputs = json.loads((reuse / 'inputs.json').read_text())
                for name, expected in prior_inputs.items():
                    if not name.startswith('apps/desktop/tests/'):
                        assert digest(ROOT / name) == expected, 'build source changed: ' + name
                config = json.loads((reuse / 'config.json').read_text())
            save(out / 'config.json', config)
            profile = out / 'isolation.sb'
            profile.write_text(POLICY.runtime_profile())
            env = {'HOME': str(home), 'TMPDIR': str(tmp) + '/', 'CARGO_HOME': '/Users/gua/.cargo',
                'RUSTUP_HOME': '/Users/gua/.rustup', 'RUSTC': str(TOOL / 'rustc'), 'RUSTDOC': str(TOOL / 'rustdoc'),
                'PATH': str(TOOL) + ':/usr/bin:/bin:/usr/sbin:/sbin', 'LANG': 'en_US.UTF-8',
                'CARGO_NET_OFFLINE': 'true', 'TAURI_CONFIG': json.dumps(config), 'MACOSX_DEPLOYMENT_TARGET': '13.0',
                'IPC_BUSINESS_ROOT': str(root), 'IPC_ORDINARY_PUBLISHER_OPT_IN': 'synthetic-memory-vault-only'}
            paths = subprocess.check_output(['git', 'ls-files', '-z', '--', 'Cargo.toml', 'Cargo.lock', '.cargo',
                'crates', 'apps/desktop/src-tauri', 'packaging/editions'], cwd=ROOT).decode().split('\0')
            paths += [str(p.relative_to(ROOT)) for p in HERE.glob('*') if p.is_file()]
            paths += ['apps/desktop/src-tauri/src/ipc_business/ordinary.rs']
            paths += list(additional_inputs)
            inputs = {p: digest(ROOT / p) for p in sorted(set(paths)) if p and (ROOT / p).is_file()}
            save(out / 'inputs.json', inputs)
            command = [str(TOOL / 'cargo'), 'build', '--release', '--offline', '--locked', '--target',
                'aarch64-apple-darwin', '-p', 'switcher-desktop', '--bin', 'switcher-desktop',
                '--no-default-features', '--features', 'personal,ipc-business,custom-protocol']
            if reuse:
                previous_build = json.loads((reuse / 'build.json').read_text())
                assert previous_build['exit'] == 0
                source_binary = reuse / 'Publisher IPC.app/Contents/MacOS/switcher-desktop'
                assert digest(source_binary) == json.loads((reuse / 'process.json').read_text())['binary_sha256']
                save(out / 'build.json', {**previous_build, 'reused_from': str(reuse), 'binary_sha256': digest(source_binary)})
            else:
                with (out / 'build.log').open('wb') as log:
                    built = subprocess.run(['/usr/bin/sandbox-exec', '-f', str(profile), *command], cwd=ROOT,
                        env=env, stdout=log, stderr=subprocess.STDOUT)
                save(out / 'build.json', {'exit': built.returncode, 'command': command, 'log_sha256': digest(out / 'build.log')})
                assert built.returncode == 0, 'see build.log'
                source_binary = ROOT / 'target/aarch64-apple-darwin/release/switcher-desktop'
            contents = out / 'Publisher IPC.app/Contents'
            (contents / 'MacOS').mkdir(parents=True)
            executable = contents / 'MacOS/switcher-desktop'
            shutil.copy2(source_binary, executable)
            (contents / 'Info.plist').write_bytes(plistlib.dumps({'CFBundleIdentifier': 'local.claudeswitcher.personal',
                'CFBundleExecutable': 'switcher-desktop', 'CFBundleName': 'Publisher IPC', 'CFBundlePackageType': 'APPL'}))
            subprocess.run(['/usr/bin/codesign', '--force', '--sign', '-', '--timestamp=none', str(contents.parent)],
                check=True, capture_output=True)
            with (out / 'runtime.log').open('wb') as log:
                process = subprocess.Popen(['/usr/bin/sandbox-exec', '-f', str(profile), str(executable), '--ordinary-publisher'],
                    cwd=out, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                save(out / 'process.json', {'pid': process.pid, 'binary_sha256': digest(executable)})
                try:
                    deadline = time.monotonic() + 75
                    while not server.finished.wait(0.25) and time.monotonic() < deadline and process.poll() is None:
                        pass
                    assert server.finished.is_set(), 'native receipt missing; see runtime.log'
                finally:
                    if process.poll() is None:
                        os.killpg(process.pid, signal.SIGTERM)
                    process.wait(timeout=10)
                    save(out / 'process-result.json', {'pid': process.pid, 'exit': process.returncode})
            assert not server.failure, server.failure
            assert inputs == {p: digest(ROOT / p) for p in inputs}, 'source inputs changed during native run'
            subprocess.run(['node', str(HERE / 'verify.mjs'), str(out)], check=True, cwd=ROOT)
        finally:
            server.shutdown()
            thread.join(timeout=5)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--reuse-build', type=Path)
    args = parser.parse_args()
    execute(args.output, args.reuse_build.resolve() if args.reuse_build else None)
