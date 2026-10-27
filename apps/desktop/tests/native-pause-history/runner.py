"""Run a pause/history scenario with the shared isolated native publisher harness."""
import argparse
import importlib.util
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('publisher_runner', HERE.parent / 'native-publisher/runner.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)
runner.HERE = HERE
runner.PHASES = ('baseline', 'single-draft', 'single-published', 'all-draft',
                 'all-published', 'rejected', 'history-draft', 'history-published')

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--reuse-build', type=Path)
    args = parser.parse_args()
    runner.execute(args.output, args.reuse_build.resolve() if args.reuse_build else None)
