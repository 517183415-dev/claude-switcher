"""Reconcile saved execution evidence without rerunning tests or touching Keychain."""
import hashlib
import json
from pathlib import Path
import subprocess

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]


def read(name):
    return json.loads((OUT / name).read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


inputs = read('validation-inputs.json')
assert all(sha(ROOT / name) == value for name, value in inputs['sha256'].items())
executable = read('executable.json')
assert sha(Path(executable['executable'])) == executable['sha256']
for name in ('build', 'native', 'default-skip', 'clippy'):
    receipt = read(name + '.json')
    assert receipt['exit'] == 0
    assert sha(OUT / (name + '.log')) == receipt['log_sha256']
    assert sha(Path(receipt['command'][2])) == receipt['policy_sha256']
assert read('format.json')['exit'] == 0
guard = read('opt-in-guard.json')
assert guard['exit'] == 101
assert sha(OUT / 'opt-in-guard.log') == guard['log_sha256']
assert 'SWITCHER_NATIVE_KEYCHAIN_OPT_IN' in (OUT / 'opt-in-guard.log').read_text()
assert not (OUT / 'opt-in-guard-state').exists()
assert not (OUT / 'default-skip-state').exists()
assert '0 passed; 0 failed; 2 ignored' in (OUT / 'default-skip.log').read_text()
assert '1 passed; 0 failed' in (OUT / 'native.log').read_text()
result = read('synthetic-state/result.json')
assert all(result[k] is True for k in ('exercise_passed', 'owned_items_removed',
                                      'private_keychain_removed', 'preferences_unchanged'))
assert read('synthetic-state/owned-keychain-items.json') == []
reader = read('synthetic-state/reader.json')
for phase in ('writer', 'reader'):
    exit_receipt = read('synthetic-state/' + phase + '-exit.json')
    assert exit_receipt['success'] is True and exit_receipt['exit'] == 0
    assert exit_receipt['pid'] == reader[phase + '_pid']
    assert '1 passed; 0 failed' in (OUT / ('synthetic-state/' + phase + '.log')).read_text()
assert reader['writer_pid'] != reader['reader_pid']
assert all(value is True for key, value in reader.items() if not key.endswith('_pid'))
for state in ('synthetic-state', 'synthetic-state-failed'):
    assert not list((OUT / state).glob('*.keychain*'))
parent = inputs['source_parent']
source = 'apps/desktop/src-tauri/src/publisher/native_keychain_tests.rs'
previous = subprocess.check_output(['git', 'show', parent + ':' + source], cwd=ROOT)
assert (ROOT / source).read_bytes() == previous + b'\n#[path = "native_keychain_process_tests.rs"]\nmod process_tests;\n'
changed = subprocess.check_output(['git', 'diff', '--name-only', parent], cwd=ROOT, text=True).splitlines()
assert all(p.startswith(('docs/', 'output/')) or p in (
    source, 'apps/desktop/src-tauri/src/publisher/native_keychain_process_tests.rs') for p in changed)
graph = read('codegraph-status.json')
assert graph['index']['state'] == 'complete' and graph['index']['pendingRefs'] == 0
receipt = {
    'status': 'PASS_NATIVE_PRIVATE_KEYCHAIN_WRITER_EXIT_THEN_NEW_READER',
    'source_parent': parent, 'source_sha256': inputs['sha256'],
    'validation_inputs_unchanged': True, 'test_binary_unchanged': True,
    'acceptance_test_names': 1, 'worker_executions': 2,
    'writer_pid': reader['writer_pid'], 'reader_pid': reader['reader_pid'],
    'writer_reaped_before_reader': 'sequential run_worker calls; writer success exit receipt precedes reader spawn',
    'native': result, 'reader': reader, 'default_ignored': 2,
    'missing_opt_in_rejected_before_root_creation': True,
    'clippy': 'PASS_WITH_3_PREEXISTING_MIGRATION_STYLE_WARNINGS',
    'format': 'PASS',
    'reused_validation': {
        'path': 'output/no-account-20261027/native-keychain-isolation/receipt.json',
        'commit': parent, 'personal_main_tests_passed': 236,
        'reason': 'production code/dependencies unchanged; old native test only appends a test-only module',
        'not_rerun': ['Personal full suite', 'Public tests/check', 'frontend', 'native ACL matrix']},
    'harness_fix': 'RED permission assertion -> private fixture chmod 0600 -> GREEN; assertion retained',
    'not_proven': ['user-default Keychain permission', 'successful native WebView business',
                   'machine reboot', 'real R2/CDN propagation', 'whole AD-10 and exhaustive branch coverage'],
    'paused': ['registration', 'deployment', 'payment'],
}
(OUT / 'receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps({'status': receipt['status'], 'writer_pid': reader['writer_pid'],
                  'reader_pid': reader['reader_pid'], 'production_unchanged': True}))
