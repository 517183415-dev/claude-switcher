"""Static exclusion evidence only. Never starts an App or invokes product commands."""
from pathlib import Path
import hashlib
import json
import re
import sys
import tomllib
import shutil

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[2]
HISTORICAL_E01 = [b'forbidden_probe', b'finish_smoke']
RUNTIME_BODY = [b'A01_IPC_EVENT', b'A01_IPC_RESULT', b'BUSINESS_IPC_RESULT',
                b'BUSINESS_IPC_DRAFT', b'BUSINESS_IPC_BACKUP', b'ipc-synthetic-backup', b'synthetic_restore_rejected', b'BUSINESS_SYNTHETIC_ONLY', b'BUSINESS_IPC_PREFERENCES', b'fixture_acl_unexpectedly_allowed',
                b'synthetic-preview-only', b'synthetic-production-only']
FIXTURE_NAMES = [b'ipc_limits_', b'ipc-limits', b'ipc_business_', b'ipc-business',
                 b'allow-ipc-limits', b'allow-ipc-business', b'native-ipc',
                 b'ipc_business_preferences_snapshot', b'allow-ipc-business-preferences-snapshot',
                 b'ipc_business_snapshot']
BACKUP_NAMES = [b'ipc_business_backup_export', b'ipc_business_backup_restore', b'allow-ipc-business-backup-export', b'allow-ipc-business-backup-restore']
DRAFT_NAMES = [b'ipc_business_draft_snapshot', b'allow-ipc-business-draft-snapshot']
PUBLISH_NAMES = [b'ipc_business_publish_init', b'ipc_business_publish',
                 b'allow-ipc-business-publish-init', b'allow-ipc-business-publish']
TOKENS = RUNTIME_BODY + FIXTURE_NAMES + BACKUP_NAMES + DRAFT_NAMES + PUBLISH_NAMES + HISTORICAL_E01
FIXTURES = ['ipc-smoke', 'ipc-limits', 'ipc-business']


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_contract(work):
    app = work / 'apps/desktop'
    config = json.loads((app / 'src-tauri/tauri.conf.json').read_text())
    editions = {edition: json.loads((work / f'packaging/editions/{edition}.json').read_text())
                for edition in ['personal', 'public']}
    main_cap = json.loads((app / 'src-tauri/capabilities/main.json').read_text())
    cargo = tomllib.loads((app / 'src-tauri/Cargo.toml').read_text())
    main = (app / 'src-tauri/src/main.rs').read_text()
    build = (app / 'src-tauri/build.rs').read_text()
    business = (app / 'src-tauri/src/ipc_business.rs').read_text()
    backup = (app / 'src-tauri/src/ipc_business/backup.rs').read_text()
    publish = (app / 'src-tauri/src/ipc_business/publish.rs').read_text()
    assert '#[cfg(feature = "personal")]\nmod publish;' in business
    for command in ['ipc_business_publish_init', 'ipc_business_publish']:
        assert f'fn {command}(' in publish and f'publish::{command}' in business and command in build
    assert 'synthetic-preview-only' in publish and 'synthetic-production-only' in publish
    for command in ['ipc_business_backup_export', 'ipc_business_backup_restore']:
        assert f'fn {command}(' in backup and f'backup::{command}' in business and command in build
    assert 'BUSINESS_IPC_BACKUP' in backup and 'ads-personal-v1' in backup
    assert '#[cfg(feature = "personal")]\nmod backup;' in business
    assert 'fn ipc_business_draft_snapshot(' in business and 'BUSINESS_IPC_DRAFT' in business
    assert 'ipc_business_draft_snapshot' in build
    assert 'fn ipc_business_preferences_snapshot(' in business
    assert 'BUSINESS_IPC_PREFERENCES' in business
    assert 'ipc_business_preferences_snapshot' in build
    vite = (app / 'vite.config.ts').read_text()
    assert config['build']['frontendDist'] == '../dist'
    assert config['app']['security']['capabilities'] == ['main']
    for edition, value in editions.items():
        assert value.get('build', {}).get('frontendDist', '../dist') == '../dist', edition
    assert cargo['features']['default'] == ['personal']
    for edition in ['personal', 'public']:
        assert not any('ipc' in feature for feature in cargo['features'][edition])
    for fixture in FIXTURES:
        assert cargo['features'][fixture] == [], fixture
        assert 'CARGO_FEATURE_' + fixture.upper().replace('-', '_') in build, fixture
    for feature, module in [('ipc-limits', 'ipc_limits'), ('ipc-business', 'ipc_business')]:
        assert re.search(r'#\[cfg\(feature = "' + feature + r'"\)\]\s*mod ' + module + ';', main)
        assert re.search(r'#\[cfg\(feature = "' + feature + r'"\)\]\s*fn main\(\)\s*\{\s*' + module + r'::run\(\);\s*\}', main)
    assert re.search(r'#\[cfg\(not\(any\(feature = "ipc-limits", feature = "ipc-business"\)\)\)\]\s*fn main\(\)', main)
    for left, right in [('ipc-smoke', 'ipc-limits'), ('ipc-smoke', 'ipc-business'), ('ipc-limits', 'ipc-business')]:
        assert f'all(feature = "{left}", feature = "{right}")' in main
    assert 'compile_error!("Select exactly one IPC test fixture")' in main
    assert 'rollupOptions' not in vite
    index = (app / 'index.html').read_text()
    assert 'tests/native-ipc' not in index
    capabilities = {'base': config['app']['security']['capabilities'], 'main': main_cap}
    for edition, value in editions.items():
        capabilities[edition] = value.get('app', {}).get('security', {}).get('capabilities', capabilities['base'])
    assert capabilities['public'] == ['main']
    assert capabilities['personal'][0] == 'main'
    assert capabilities['personal'][1]['identifier'] == 'personal-operations'
    assert len(capabilities['personal']) == 2
    assert not any(token.decode() in json.dumps(capabilities) for token in TOKENS)
    # The nested personal branch in ipc-limits starts with `if`, so this exact
    # `else if` separator selects the ordinary edition command branches.
    normal = build.split('} else if std::env::var_os("CARGO_FEATURE_PERSONAL").is_some() {', 1)[1]
    command_names = re.findall(r'"([a-z_]+)"', normal.split('let manifest', 1)[0])
    assert command_names and not any(token.decode() in command for token in TOKENS for command in command_names)
    handler_names = {}
    for edition in ['personal', 'public']:
        pattern = (r'#\[cfg\(all\(not\(feature = "ipc-smoke"\), feature = "' + edition
                   + r'"\)\)\]\s*let builder = builder.invoke_handler\(tauri::generate_handler!\[(.*?)\]\);')
        match = re.search(pattern, main, re.S)
        assert match, edition
        names = re.findall(r'(?:commands|publisher_commands)::([a-z_]+)', match.group(1))
        assert names and not any(token.decode() in name for token in TOKENS for name in names)
        handler_names[edition] = names
    fixture_configs = {}
    for fixture in ['ipc-limits', 'ipc-business']:
        for edition in ['personal', 'public']:
            name = f'packaging/editions/{fixture}-{edition}.json'
            value = json.loads((work / name).read_text())
            # Fixture capability is selected only by an explicitly different
            # config, while ordinary builds use packaging/editions/<edition>.
            fixture_configs[name] = {'sha256': sha(work / name), 'config': value}
            assert value['build']['frontendDist'] != '../dist'
            assert value['app']['security']['capabilities'] != capabilities[edition]
            if fixture == 'ipc-business':
                permissions = value['app']['security']['capabilities'][0]['permissions']
                assert 'allow-ipc-business-preferences-snapshot' in permissions
                assert ('allow-ipc-business-draft-snapshot' in permissions) == (edition == 'personal')
                publish_permissions = ['allow-ipc-business-publish-init', 'allow-ipc-business-publish']
                assert all(permission in permissions for permission in publish_permissions) if edition == 'personal' else not any(permission in permissions for permission in publish_permissions)
                backup_permissions = ['allow-ipc-business-backup-export', 'allow-ipc-business-backup-restore']
                assert all(permission in permissions for permission in backup_permissions) if edition == 'personal' else not any(permission in permissions for permission in backup_permissions)
    inputs = ['apps/desktop/src-tauri/tauri.conf.json', 'packaging/editions/personal.json',
              'packaging/editions/public.json', 'apps/desktop/src-tauri/capabilities/main.json',
              'apps/desktop/src-tauri/Cargo.toml', 'apps/desktop/src-tauri/build.rs',
              'apps/desktop/src-tauri/src/main.rs', 'apps/desktop/src-tauri/src/ipc_business.rs', 'apps/desktop/vite.config.ts', 'apps/desktop/index.html', 'apps/desktop/src-tauri/src/ipc_business/backup.rs', 'apps/desktop/src-tauri/src/ipc_business/publish.rs']
    return {'status': 'PASS_SOURCE_CONFIGURATION_ONLY', 'frontend_dist': '../dist',
            'capabilities': capabilities, 'ordinary_handler_names': handler_names,
            'normal_command_names': sorted(set(command_names)),
            'fixture_features_default_disabled': True, 'fixture_module_and_main_cfg_gated': True,
            'fixture_features_mutually_exclusive': True,
            'draft_fixture_commands': [token.decode() for token in DRAFT_NAMES],
            'draft_fixture_excluded_from_ordinary_handlers_commands_capabilities': True,
            'publish_fixture_commands': [token.decode() for token in PUBLISH_NAMES],
            'publish_fixture_personal_only_and_excluded_from_ordinary_handlers_commands_capabilities': True,
            'backup_fixture_commands': [token.decode() for token in BACKUP_NAMES],
            'backup_fixture_personal_only_and_excluded_from_ordinary_handlers_commands_capabilities': True,
            'preferences_snapshot_entry': 'ipc_business_preferences_snapshot',
            'preferences_snapshot_excluded_from_normal_handler_commands_capabilities': True, 'unselected_fixture_configs': fixture_configs,
            'input_sha256': {name: sha(work / name) for name in inputs}}


def marker_scan(path):
    data = path.read_bytes()
    hits = []
    for token in TOKENS:
        offsets = [match.start() for match in re.finditer(re.escape(token), data)]
        if offsets:
            category = ('historical_e01_permission_review' if token in HISTORICAL_E01 else
                        'fixture_runtime_body' if token in RUNTIME_BODY else 'publish_fixture_command_or_acl' if token in PUBLISH_NAMES else 'backup_fixture_command_or_acl' if token in BACKUP_NAMES else 'draft_fixture_command_or_acl' if token in DRAFT_NAMES else 'fixture_name_or_permission_review')
            hits.append({'token': token.decode(), 'category': category,
                         'count': len(offsets), 'first_offsets': offsets[:12],
                         'contexts': [data[max(0, offset - 50):offset + len(token) + 80]
                                      .decode('utf-8', errors='replace') for offset in offsets[:3]]})
    return hits


def verify(edition):
    assert edition in ['personal', 'public']
    state = json.loads((OUT / 'runtime.json').read_text())
    work = Path(state['workspace'])
    contract = source_contract(work)
    bundle = json.loads((OUT / ('bundle-' + edition + '.json')).read_text())
    argv = bundle['argv']
    assert argv[argv.index('--features') + 1] == edition
    assert argv[argv.index('--config') + 1] == f'../../packaging/editions/{edition}.json'
    assert '--no-default-features' in argv and '--no-sign' in argv
    assert '--offline' in argv and '--locked' in argv
    assert state['environment']['VITE_IPC_SMOKE'] == '0'
    assert not any(name.startswith(('IPC_', 'CARGO_FEATURE_IPC')) or name == 'TAURI_CONFIG'
                   for name in state['environment'])
    artifact = json.loads((OUT / ('artifact-' + edition + '.json')).read_text())
    app = Path(artifact['app'])
    dist = work / 'apps/desktop/dist'
    assert dist.is_dir()
    retained = OUT / 'dist' / edition
    assert not retained.exists(), 'Never overwrite an edition dist'
    retained.parent.mkdir(exist_ok=True)
    shutil.copytree(dist, retained)
    dist = retained
    dist_entries = {}
    for path in sorted(dist.rglob('*')):
        if path.is_file():
            hits = marker_scan(path)
            dist_entries[str(path.relative_to(dist))] = {'sha256': sha(path), 'bytes': path.stat().st_size,
                                                       'fixture_marker_hits': hits}
            assert not hits, 'Fixture marker in ordinary frontendDist: ' + str(path)
    assert dist_entries and 'index.html' in dist_entries
    resources, binaries, violations = {}, {}, []
    for path in sorted(app.rglob('*')):
        if not path.is_file() or path.is_symlink():
            continue
        relative = str(path.relative_to(app))
        assert not any(token.decode() in relative for token in TOKENS), relative
        hits = marker_scan(path)
        if path.read_bytes()[:4] == b'\xcf\xfa\xed\xfe':
            binaries[relative] = hits
            violations += [{'path': relative, **hit} for hit in hits if hit['category'] in ['fixture_runtime_body', 'publish_fixture_command_or_acl', 'backup_fixture_command_or_acl', 'draft_fixture_command_or_acl']]
        else:
            resources[relative] = hits
            assert not hits, 'Fixture marker in App resource: ' + relative
    record = {'edition': edition, 'commit': state['commit'], 'source_contract': contract,
              'build_argv': argv, 'build_fixture_features_disabled': True,
              'retained_frontend_dist': str(dist), 'ordinary_frontend_dist': dist_entries, 'app_resource_marker_hits': resources,
              'binary_marker_hits': binaries, 'runtime_body_violations': violations,
              'runtime_body_exclusion': 'FAIL_RUNTIME_BODY_PRESENT' if violations else 'PASS_NO_RUNTIME_BODY_MARKERS',
              'binary_scan_status': ('FAIL_RUNTIME_BODY_PRESENT' if violations else
                                     'REVIEW_MARKERS_PRESENT' if any(binaries.values()) else 'NO_FIXTURE_MARKERS_FOUND'),
              'interpretation': 'Raw names/permission descriptions are reported as REVIEW, including historical E01 forbidden_probe/finish_smoke; they do not prove an active callable handler. Runtime-body markers fail this static gate. Static evidence does not establish native runtime ACL behavior.',
              'historical_e01_baseline': {'personal': {'forbidden_probe': 1, 'finish_smoke': 1},
                                          'public': {'forbidden_probe': 1, 'finish_smoke': 1},
                                          'status': 'HISTORICAL_REVIEW_NOT_ERASED_BY_THIS_RUN'},
              'native_runtime_acl': 'NOT_RUN', 'product_launch': 'NOT_RUN'}
    target = OUT / ('fixture-exclusion-' + edition + '.json')
    assert not target.exists()
    target.write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n')
    assert not violations, 'Fixture runtime body present; preserve receipt and stop'
    print(json.dumps({'edition': edition, 'binary_scan_status': record['binary_scan_status'], 'receipt': str(target)}))


if __name__ == '__main__':
    if sys.argv[1:] == ['--source-only']:
        target = OUT / 'source-configuration-preparation.json'
        assert not target.exists()
        target.write_text(json.dumps(source_contract(ROOT), ensure_ascii=False, indent=2) + '\n')
    else:
        verify(sys.argv[1])
