"""Validate preserved native receipts; do not relaunch or rebuild the App."""
import importlib.util
import json
from pathlib import Path

OUT = Path(__file__).resolve().parent
ROOT = next(path for path in OUT.parents if (path / 'Cargo.toml').is_file())
HERE = ROOT / 'apps/desktop/tests/native-ipc-policy'
spec = importlib.util.spec_from_file_location('policy_runner', HERE / 'run.py')
r = importlib.util.module_from_spec(spec)
spec.loader.exec_module(r)
prior = json.loads((OUT / 'inputs.json').read_text())
changed = [name for name, sha in prior.items() if r.digest(ROOT / name) != sha]
assert set(changed) <= {str((HERE / name).relative_to(ROOT)) for name in ('run.py', 'test_receipts.py', 'README.md')}, changed
server = json.loads((OUT / 'server.json').read_text())
commands = r.boundary_commands('public')
for phase, url in [('remote', server['url']), ('other', 'tauri://localhost/')]:
    r.validate_boundary(json.loads((OUT / (phase + '-result.json')).read_text()),
                        'public', phase, url, commands)
assert '/' in server['requests'] and '/probe.js' in server['requests']
assert json.loads((OUT / 'home/.claude-switcher/v2/public/preferences.json').read_text()) == r.PREFERENCES
process = json.loads((OUT / 'process-result.json').read_text())
assert process['exit_code'] == -15
receipt = {'status':'PASS_SCOPED_SAVED_NATIVE_RECEIPTS', 'edition':'public',
 'scope':'ordinary production entry and ACL with scripted frontend and test windows',
 'contexts':['remote','other'], 'denials_per_context':len(commands),
 'native_rerun':False, 'product_inputs_unchanged':True,
 'corrected_harness_files':changed,
 'validator_sha256':r.digest(HERE/'run.py'), 'validator_tests_sha256':r.digest(HERE/'test_receipts.py'),
 'original_inputs_sha256':r.digest(OUT/'inputs.json'),
 'executed_binary_sha256':r.digest(Path(json.loads((OUT/'process.json').read_text())['executable'])),
 'evidence_sha256':{name:r.digest(OUT/name) for name in ('remote-result.json','other-result.json','server.json','window-read.json','process-result.json','build.json','isolation.sb','runtime.log')},
 'not_verified':['unchanged product frontend','native Keychain','real accounts','external hosting','system-managed WebKit termination']}
r.save(OUT/'validated-receipt.json', receipt)
print(json.dumps({k:receipt[k] for k in ('status','edition','contexts','denials_per_context','product_inputs_unchanged')}))
