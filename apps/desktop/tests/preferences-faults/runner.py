"""Run a bound synthetic test binary under a single-layer account-free Seatbelt policy."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import signal
import time
import os
import pwd

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent


def baseline_profile():
    template = (HERE / 'account-free.sb.in').read_text()
    home = pwd.getpwuid(os.geteuid()).pw_dir
    # Substitute inside existing Seatbelt string literals with escaped path contents.
    return template.replace('@PROJECT@', json.dumps(str(ROOT))[1:-1]).replace(
        '@HOME@', json.dumps(home)[1:-1]).encode()


TEST = 'activity_write_denial_returns_error_after_preferences_commit_both_editions'
BLOCKED = 'blocked_preferences_destination_preserves_existing_activity_both_editions'


def combined_profile(baseline, home, paths):
    if baseline != baseline_profile():
        raise ValueError('baseline restrictions changed')
    expected = [home / '.claude-switcher/v2' / edition / 'activity.json'
                for edition in ('personal', 'public')]
    if paths != expected or not home.is_absolute() or home.resolve() != home:
        raise ValueError('unapproved synthetic destinations')
    return baseline + b'\n' + ''.join(
        '(deny file-write* (literal ' + json.dumps(str(path)) + '))\n'
        for path in paths).encode()


def phase_environment(environment, out, mode):
    result = environment.copy()
    if mode:
        result['SWITCHER_E11_PREF_FAULT'] = mode
    if mode == 'blocked':
        blocked_home = out.resolve() / 'blocked-home'
        blocked_home.mkdir(mode=0o700)
        result['HOME'] = str(blocked_home)
    return result


def require_child(exit_code, text):
    if exit_code != 0 or len(re.findall(r'^test result: ok\. 1 passed; 0 failed;', text, re.M)) != 1:
        raise ValueError('child harness failed or did not execute exactly one test')


CHILD_TIMEOUT_SECONDS = 30


def run_bounded(argv, cwd, environment, log, timeout=CHILD_TIMEOUT_SECONDS):
    """Persist output directly; kill the isolated process group on timeout."""
    with log.open('xb') as stream:
        with subprocess.Popen(argv, cwd=cwd, env=environment, stdout=stream,
                              stderr=subprocess.STDOUT, start_new_session=True) as child:
            try:
                return child.wait(timeout=timeout), False
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait(timeout=5)
                stream.write(b'\nPREFERENCE_FAULT_TIMEOUT: child exceeded bounded deadline\n')
                stream.flush()
                return 124, True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--binding', type=Path, required=True,
                        help='prior build/run receipt with binary_sha256 and source_after')
    args = parser.parse_args()
    out = args.output.absolute()
    if out.resolve() != out or not out.is_relative_to(ROOT / 'output'):
        raise ValueError('output must be a fresh non-symlink directory below project output/')
    out.mkdir(mode=0o700, exist_ok=False)
    home, tmp = out / 'home', out / 'tmp'
    home.mkdir(mode=0o700)
    tmp.mkdir(mode=0o700)
    binding = json.loads(args.binding.read_text())
    binary = str(args.binary.resolve(strict=True))
    if hashlib.sha256(Path(binary).read_bytes()).hexdigest() != binding['binary_sha256']:
        raise ValueError('binary differs from prior bound build/run')
    env = {'HOME': str(home), 'TMPDIR': str(tmp) + '/', 'PATH': '/usr/bin:/bin:/usr/sbin:/sbin',
           'LC_ALL': 'en_US.UTF-8', 'PYTHONDONTWRITEBYTECODE': '1'}
    baseline = out / 'account-free.sb'
    baseline.write_bytes(baseline_profile())
    files = ['apps/desktop/src-tauri/tests/e11_log_privacy.rs', 'apps/desktop/src-tauri/src/bridge.rs',
             'apps/desktop/src-tauri/src/update_runtime.rs', 'crates/switcher-core/src/storage.rs',
             'Cargo.lock', 'apps/desktop/src-tauri/Cargo.toml']
    def inputs():
        return {p: hashlib.sha256((ROOT / p).read_bytes()).hexdigest() for p in files}
    before = inputs()
    if binding['source_after'] != before:
        raise ValueError('binary source binding differs')
    runner_sha256 = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    binary_sha256 = hashlib.sha256(Path(binary).read_bytes()).hexdigest()
    support_hashes = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                      for p in sorted(HERE.iterdir()) if p.is_file()}
    records = []
    def run(label, argv, profile, mode=None):
        call_env = phase_environment(env, out, mode)
        wrapped = ['/usr/bin/sandbox-exec', '-f', str(profile), *argv]
        start = time.monotonic()
        log = out / (label + '.log')
        exit_code, timed_out = run_bounded(wrapped, ROOT, call_env, log)
        stdout = log.read_bytes()
        record = dict(label=label, argv=wrapped, environment=call_env, exit=exit_code,
                      timed_out=timed_out, timeout_seconds=CHILD_TIMEOUT_SECONDS,
                      seconds=time.monotonic()-start, log_sha256=hashlib.sha256(stdout).hexdigest(),
                      profile_sha256=hashlib.sha256(profile.read_bytes()).hexdigest())
        records.append(record)
        (out / 'receipt.json').write_text(json.dumps(dict(records=records, source_before=before, source_after=inputs(), runner_sha256=runner_sha256, binary_sha256=binary_sha256, support_hashes=support_hashes, binding_path=str(args.binding.resolve()), binding_sha256=hashlib.sha256(args.binding.read_bytes()).hexdigest()), indent=2)+'\n')
        if timed_out:
            raise TimeoutError('child deadline exceeded; log and failure receipt retained: ' + label)
        require_child(exit_code, stdout.decode())
        return stdout.decode()
    run('blocked', [binary, '--exact', BLOCKED, '--nocapture'], baseline, 'blocked')
    argv = [binary, '--exact', TEST, '--ignored', '--nocapture']
    run('prepare', argv, baseline, 'prepare')
    paths = [home / '.claude-switcher/v2' / e / 'activity.json' for e in ('personal', 'public')]
    def logs():
        return [(p.read_bytes(), p.stat().st_ino, p.stat().st_mtime_ns) for p in paths]
    original_logs = logs()
    profile = out / 'activity-write-denied.sb'
    profile.write_bytes(combined_profile(baseline.read_bytes(), home, paths))
    run('denied', argv, profile, 'denied')
    if logs() != original_logs:
        raise ValueError('denied write changed original activity')
    run('reopen', argv, baseline, 'reopen')
    if inputs() != before:
        raise ValueError('source changed during validation')
    receipt = json.loads((out / 'receipt.json').read_text())
    receipt['passed'] = True
    (out / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(dict(passed=True, phases=len(records), source_unchanged=True, receipt=str(out/'receipt.json'))))


if __name__ == '__main__':
    main()
