"""Exercise the shipped Svelte page through native IPC and a synthetic vault."""
import argparse
import html
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('publisher_runner', HERE.parent / 'native-publisher/runner.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
runner.HERE = HERE
runner.PHASES = ('baseline', 'done')


def frontend(out, dist, plan):
    profile = out / 'frontend-isolation.sb'
    profile.write_text(runner.POLICY.runtime_profile())
    env = {'HOME': str(out / 'home'), 'TMPDIR': str(out / 'tmp') + '/',
           'PATH': '/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin',
           'LANG': 'en_US.UTF-8', 'VITE_EDITION': 'personal'}
    command = ['/usr/bin/sandbox-exec', '-f', str(profile), '/opt/homebrew/bin/node',
               'node_modules/vite/bin/vite.js', 'build', '--config', str(HERE / 'vite.config.js'),
               '--outDir', str(dist), '--emptyOutDir']
    with (out / 'frontend-build.log').open('wb') as log:
        result = subprocess.run(command, cwd=runner.ROOT / 'apps/desktop', env=env,
                                stdout=log, stderr=subprocess.STDOUT)
    assert result.returncode == 0, 'see frontend-build.log'
    index = dist / 'index.html'
    source = index.read_text()
    assert source.count('<head>') == 1
    injection = '<meta name="native-plan" content="' + html.escape(json.dumps(plan), quote=True) + '">'
    assert source.count('<body>') == 1
    index.write_text(source.replace('<head>', '<head>' + injection, 1)
                     .replace('<body>', '<body><script src="probe.js"></script>', 1))
    shutil.copy2(HERE / 'probe.js', dist / 'probe.js')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    paths = subprocess.check_output(['git', 'ls-files', '-z', '--', 'apps/desktop/src',
        'apps/desktop/index.html', 'apps/desktop/vite.config.ts', 'apps/desktop/package.json',
        'apps/desktop/package-lock.json'], cwd=runner.ROOT).decode().split('\0')
    runner.execute(args.output, prepare_frontend=frontend, additional_inputs=paths)
