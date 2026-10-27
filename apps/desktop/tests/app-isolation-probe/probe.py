#!/usr/bin/env python3
"""Direct-process isolation probe. Never opens Claude, Keychain, or a network connection."""
import ctypes
import errno
import json
import os
import socket
import subprocess
import sys
from pathlib import Path

lib = ctypes.CDLL('/usr/lib/libSystem.B.dylib', use_errno=True)


def file_probe():
    home = Path(os.environ['HOME'])
    target = home / ('probe-' + str(os.getpid()))
    target.write_bytes(b'synthetic-only')
    good = target.read_bytes() == b'synthetic-only'
    target.unlink()
    try:
        fd = os.open(os.environ['PRIVATE_PROBE_PATH'], os.O_RDONLY | os.O_DIRECTORY)
    except OSError as error:
        private = {'denied': error.errno in (errno.EACCES, errno.EPERM), 'errno': error.errno}
    else:
        os.close(fd)  # No directory listing or content read, even on an unexpected allow.
        private = {'denied': False, 'errno': 0}
    return {'synthetic_home_roundtrip': good, 'private_directory': private}


def network_probe():
    # A bind on loopback cannot send packets and never creates a listener.
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    except OSError as error:
        loopback = {'denied': False, 'attempted': False, 'socket_errno': error.errno}
    else:
        with sock:
            try:
                sock.bind(('127.0.0.1', 0))
            except OSError as error:
                loopback = {'denied': error.errno in (errno.EACCES, errno.EPERM),
                            'attempted': True, 'errno': error.errno}
            else:
                loopback = {'denied': False, 'attempted': True, 'errno': 0}
    # Inspect the blanket policy; never issue an external connect or send.
    try:
        check = lib.sandbox_check
        check.restype = ctypes.c_int
        outcome = check(os.getpid(), b'network-outbound', 0)
        outbound = {'policy_denied': outcome > 0, 'return': outcome,
                    'actual_nonlocal_connection': 'NOT_RUN'}
    except (AttributeError, OSError) as error:
        outbound = {'policy_denied': False, 'actual_nonlocal_connection': 'NOT_RUN',
                    'error': type(error).__name__}
    return {'loopback_bind': loopback, 'outbound': outbound}


def mach_probe():
    # The installed SDK does not expose a verifiable service-name filter for
    # sandbox_check. Never attempt a real service lookup on a guessed filter.
    return {'lookup': 'NOT_PROVEN', 'reason': 'service-name policy filter unavailable'}


def run_one():
    return {'pid': os.getpid(), 'ppid': os.getppid(), **file_probe(),
            'network': network_probe(), 'securityd': mach_probe()}


def main():
    if len(sys.argv) == 2 and sys.argv[1] == '--child':
        print(json.dumps(run_one(), sort_keys=True))
        return
    parent = run_one()
    child = subprocess.run([sys.executable, __file__, '--child'], check=True,
                           text=True, capture_output=True, timeout=15)
    data = json.loads(child.stdout)
    if data['ppid'] != parent['pid'] or data['pid'] == parent['pid']:
        raise ValueError('child identity mismatch')
    try:
        os.kill(data['pid'], 0)
    except ProcessLookupError:
        reaped = True
    except PermissionError:
        reaped = False
    else:
        reaped = False
    print(json.dumps({'parent': parent, 'child': data, 'direct_child_reaped': reaped,
                      'webkit_process_isolation': 'NOT_PROVEN',
                      'system_managed_process_cleanup': 'NOT_PROVEN'}, sort_keys=True))


if __name__ == '__main__':
    main()
