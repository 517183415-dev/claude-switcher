"""Check the recorded ordinary-App observations without rerunning product tests."""
import hashlib
import json
from pathlib import Path
import stat

OUT = Path(__file__).resolve().parent


def read(relative):
    return json.loads((OUT / relative).read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(relative, value):
    (OUT / relative).write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def file_snapshot(root):
    return {str(path.relative_to(root)): digest(path)
            for path in sorted(root.rglob('*')) if path.is_file()}


runtime = read('runtime.json')
editions = {}
for edition in ('personal', 'public'):
    folder = OUT / ('runtime-' + edition)
    artifact = read('artifact-' + edition + '.json')
    bundle = read('bundle-' + edition + '.json')
    exclusion = read('fixture-exclusion-' + edition + '.json')
    controls = read('runtime-' + edition + '/controls.json')
    assert bundle['exit'] == 0 and bundle['commit'] == runtime['commit']
    assert artifact['commit'] == runtime['commit']
    assert controls['exit'] == 0 and all(json.loads(controls['stdout']).values())
    assert exclusion['build_fixture_features_disabled']
    assert exclusion['runtime_body_exclusion'] == 'PASS_NO_RUNTIME_BODY_MARKERS'
    assert digest(Path(artifact['app']) / 'Contents/MacOS/switcher-desktop') == artifact['signed_main_sha256']
    attempts = []
    for path in sorted(folder.glob('*.result.json')):
        attempt = json.loads(path.read_text())
        assert attempt['ordinary_entry'] and not attempt['test_feature']
        assert attempt['binary_sha256'] == artifact['signed_main_sha256']
        assert attempt['profile_sha256'] == digest(folder / 'runtime.sb')
        assert attempt['exit'] == -15 and attempt['stop_requested']
        attempts.append(path.stem.removesuffix('.result'))
    assert attempts
    root = folder / 'home/.claude-switcher/v2' / edition
    preferences = json.loads((root / 'preferences.json').read_text())
    assert preferences['ads']['slots_enabled'] == [False, False, False]
    assert preferences['ads']['request_purpose_acknowledged']
    for relative in ('preferences.json', 'activity.json', 'exports/activity-redacted.json'):
        path = root / relative
        info = path.lstat()
        assert stat.S_ISREG(info.st_mode) and info.st_nlink == 1
        assert stat.S_IMODE(info.st_mode) == 0o600
    editions[edition] = {
        'build_exit': bundle['exit'], 'commit': artifact['commit'],
        'binary_sha256': artifact['signed_main_sha256'],
        'tree_manifest_sha256': artifact['tree_manifest_sha256'],
        'static_checks': artifact['static_check_command_count'],
        'fixture_body_exclusion': exclusion['runtime_body_exclusion'],
        'binary_name_scan': exclusion['binary_scan_status'],
        'controls': json.loads(controls['stdout']),
        'stopped_attempts': attempts, 'final_preferences': preferences,
        'files': file_snapshot(root),
    }

before_personal = read('personal-before-restart.json')
assert before_personal['preferences']['ads']['slots_enabled'] == [True, False, False]
assert before_personal['export_equals_activity']
before_public = read('public-before-restart.json')
assert before_public['preferences.json']['value']['ads']['slots_enabled'] == [False, True, False]
assert before_public['activity.json']['value'] == before_public['exports/activity-redacted.json']['value']
personal_root = OUT / 'runtime-personal/home/.claude-switcher'
assert read('personal-before-public.json') == file_snapshot(personal_root)
publisher = personal_root / 'v2/personal/publisher'
active = json.loads((publisher / 'active.json').read_text())
state = json.loads((publisher / active['generation'] / 'state.json').read_text())
assert state['draft']['content_revision'] == 2 and not state['draft']['slots'][0]['enabled']
personal_restart = (OUT / 'personal-draft-after-restart.txt').read_text()
assert '本地草稿 序号 2' in personal_restart and 'checkbox 允许此广告位展示, Value: 0' in personal_restart
public_restart = (OUT / 'public-after-restart.txt').read_text()
assert '显示注册好的账号广告, Value: on' in public_restart and 'button 广告运营' not in public_restart
assert editions['public']['files']['exports/activity-redacted.json'] == before_public['exports/activity-redacted.json']['sha256']

save('receipt.json', {
    'status': 'PASS_SCOPED_ORDINARY_APP_BUSINESS',
    'source_commit': runtime['commit'], 'source_tree': runtime['commit_tree'],
    'archive_sha256': runtime['archive_sha256'], 'editions': editions,
    'new_evidence': ['ordinary entry startup', 'preferences save and process restart',
                     'activity read and redacted export equal at export checkpoint',
                     'Personal draft save and restart', 'Personal local remote-configuration read and R2 form',
                     'Public UI has no publisher navigation', 'Personal files unchanged during Public validation'],
    'interpretation': 'UI observations plus independent synthetic-file checks; not a complete IPC/ACL command matrix',
    'environment_checks': '0 pass, 1 advisory, 13 blocking under isolation; not launch acceptance',
    'reuse': 'No product source changed; backend 223-pass evidence belongs to the preceding remote-locks stage. Not rerun or added to this count.',
    'not_run': ['real Claude accounts', 'real Keychain', 'real R2', 'deployment', 'payment',
                'complete ordinary-App ACL denial matrix', 'installation and guardian services',
                'remote propagation', 'Developer ID and notarization'],
    'not_proven': ['system-managed WebKit helper termination', 'direct securityd connection negative control'],
    'attempt_notes': ['Personal r3 and Public r1 interrupted for user design requests before assertions; not counted as additional cases',
                      'Initial system Python lacked tar extraction filter argument; harness bootstrap retried with installed Python 3.14, no product failure',
                      'One Personal r4 stale AX index minimized the window; recorded draft state precedes this harness interaction',
                      'Some AX transcript files contain diffs or no-change observations; use named restart/full-form snapshots and disk checkpoints for assertions'],
    'ui_rewrite': 'Queued last; native SwiftUI Liquid Glass effect, current product layout, non-blue-purple palette; not implemented',
})
paths = [path for path in OUT.iterdir() if path.is_file() and path.name not in ('source.tar', 'evidence-manifest.json')]
for edition in editions:
    paths += [path for path in (OUT / ('runtime-' + edition)).iterdir()
              if path.is_file() and path.suffix in ('.json', '.sb', '.stdout', '.stderr')]
save('evidence-manifest.json', {str(path.relative_to(OUT)): digest(path) for path in sorted(paths)})
print(json.dumps({'status': 'PASS_SCOPED_ORDINARY_APP_BUSINESS', 'source_commit': runtime['commit'],
                  'editions': list(editions), 'manifest_files': len(paths)}, ensure_ascii=False))
