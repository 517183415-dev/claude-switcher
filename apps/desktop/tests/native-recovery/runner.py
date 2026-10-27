"""Run the recovery scenario using the existing isolated WebView harness."""
import argparse
import importlib.util
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('publisher_runner', HERE.parent / 'native-publisher/runner.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
runner.HERE = HERE
runner.PHASES = ('baseline', 'prepared', 'canceled', 'rejected', 'confirmed', 'republished')

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--reuse-build', type=Path)
    args = parser.parse_args()
    runner.execute(args.output, args.reuse_build.resolve() if args.reuse_build else None)
