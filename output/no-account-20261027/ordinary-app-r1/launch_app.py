"""Launch the archived ordinary App in the already checked Seatbelt profile."""
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

out = Path(__file__).resolve().parent
edition, attempt = sys.argv[1:]
assert edition in ('personal', 'public') and attempt.isalnum()
folder = out / ('runtime-' + edition)
controls = json.loads((folder / 'controls.json').read_text())
assert controls['exit'] == 0 and all(json.loads(controls['stdout']).values())
app = out / 'artifacts' / edition / ('Claude 切换器 ' + edition.title() + '.app')
executable = app / 'Contents/MacOS/switcher-desktop'
assert executable.is_file() and not executable.is_symlink()
prefix = folder / attempt
assert not prefix.with_suffix('.launch.json').exists()
command = ['/usr/bin/sandbox-exec', '-f', controls['profile'], str(executable)]
started = time.time()
with prefix.with_suffix('.stdout').open('xb') as stdout, prefix.with_suffix('.stderr').open('xb') as stderr:
    process = subprocess.Popen(command, cwd=folder, env=controls['environment'],
                               stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                               start_new_session=True)
    record = {'pid': process.pid, 'command': command, 'started': started,
              'edition': edition, 'binary_sha256': hashlib.sha256(executable.read_bytes()).hexdigest(),
              'profile_sha256': hashlib.sha256(Path(controls['profile']).read_bytes()).hexdigest(),
              'environment': controls['environment'], 'ordinary_entry': True,
              'test_feature': False, 'stop_policy': 'only this spawned process group'}
    prefix.with_suffix('.launch.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps(record), flush=True)
    stopped = False
    while process.poll() is None and time.time() - started < 1200:
        if prefix.with_suffix('.stop').exists():
            stopped = True
            break
        time.sleep(0.5)
    if process.poll() is None:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=5)
    record.update(exit=process.returncode, elapsed_seconds=time.time()-started, stop_requested=stopped,
                  webkit_helper_termination='NOT_PROVEN')
    prefix.with_suffix('.result.json').write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps({'exit': process.returncode, 'stop_requested': stopped}), flush=True)
