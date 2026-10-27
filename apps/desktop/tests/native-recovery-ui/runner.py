"""Run the product recovery page in the isolated native WebView harness."""
import argparse
import importlib.util
from pathlib import Path
import subprocess

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('remote_ui_runner', HERE.parent / 'native-remote-ui/runner.py')
ui = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ui)
ui.HERE = HERE
ui.runner.HERE = HERE
ui.runner.PHASES = ('baseline', 'prepared', 'canceled', 'confirmed', 'republished')

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    paths = subprocess.check_output(['git', 'ls-files', '-z', '--', 'apps/desktop/src',
        'apps/desktop/index.html', 'apps/desktop/vite.config.ts', 'apps/desktop/package.json',
        'apps/desktop/package-lock.json'], cwd=ui.runner.ROOT).decode().split('\0')
    paths += ['apps/desktop/tests/native-remote-ui/runner.py',
              'apps/desktop/tests/native-remote-ui/tauri-shim.js',
              'apps/desktop/tests/native-publisher/runner.py',
              'apps/desktop/tests/native-ipc-policy/run.py']
    ui.runner.execute(args.output, prepare_frontend=ui.frontend, additional_inputs=paths)
