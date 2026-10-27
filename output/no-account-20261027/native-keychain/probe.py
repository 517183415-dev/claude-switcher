"""Read-only default-Keychain status; never lists items or changes preferences."""
from pathlib import Path
import os
import subprocess
OUT = Path(__file__).resolve().parent
subprocess.run(['/usr/bin/swiftc', str(OUT/'default-keychain.swift'), '-o', str(OUT/'default-keychain')], check=True)
for label, home in [('isolated', OUT/'home'), ('user-home-denied-library', Path('/Users/gua'))]:
    result = subprocess.run(['/usr/bin/sandbox-exec', '-f', str(OUT/'native.sb'), str(OUT/'default-keychain')],
        env={'HOME':str(home), 'TMPDIR':str(OUT/'tmp'), 'PATH':'/usr/bin:/bin'}, capture_output=True, text=True, check=True)
    (OUT/f'probe-{label}.json').write_text(result.stdout)
    print(label, result.stdout.strip())
