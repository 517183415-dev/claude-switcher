"""Isolated failure injection. Never uses launchctl, sudo or production paths."""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('local_install', Path(__file__).with_name('local_install.py'))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)
REAL_SAFE_PARENTS = m.safe_parents
REAL_PRIVATE_JSON = m.private_json
REAL_CREATE_CHALLENGE = m.create_challenge
REAL_LEGACY_SNAPSHOT = m.legacy_snapshot
REAL_STOPPED = m.stopped
REAL_RUN = m.run
REAL_ROOT_SERVICE_OWNERSHIP = m.root_service_ownership


class LocalInstallTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name).resolve()
        self.system = self.root/'system'; self.system.mkdir()
        self.names = dict(SYSTEM=self.system, HELPER=self.root/'helper', GUARDIAN=self.root/'guardian',
                          DAEMON=self.root/'daemon', AGENT=self.root/'agent', RECORD=self.system/'install.json',
                          JOURNAL=self.system/'journal.json', BARRIER=self.system/'barrier',
                          CHALLENGE=self.system/'challenge.json',RECEIPT=self.system/'receipt.json',
                          LAST_RESULT=self.system/'last-result.json',
                          WATCHDOG_PLIST=self.root/'watchdog.plist',
                          BODYGUARD_PLIST=self.root/'bodyguard.plist')
        from types import SimpleNamespace
        account=m.pwd.getpwuid(os.geteuid())
        self.patches = [patch.object(m,k,v) for k,v in self.names.items()]
        self.patches += [patch.object(m.pwd,'getpwuid',return_value=SimpleNamespace(
            pw_dir=str(self.root/'default-owner-home'),pw_name=account.pw_name,pw_gid=account.pw_gid))]
        self.patches += [patch.object(m,'app',return_value=self.root/'Application.app'),
                         patch.object(m,'safe_parents'), patch.object(m,'legacy_snapshot',return_value=[]),
                         patch.object(m,'stopped'), patch.object(m,'service'),
                         patch.object(m,'create_challenge'), patch.object(m,'root_service_ownership'),
                         patch.object(m,'load_journal',side_effect=lambda:json.loads(m.JOURNAL.read_text())),
                         patch.object(m,'private_json',side_effect=lambda path,mode:json.loads(path.read_text())),
                         patch.object(m,'identity',return_value='a'*40),
                         patch.object(m,'run',return_value=type('Result',(),{'stdout':'11111111-1111-1111-1111-111111111111','stderr':'Could not find service fixture','returncode':113})())]
        for p in self.patches: p.start()
        self.source = self.root/'payload'; self.source.write_text('new')
        self.templates = self.root/'templates'; self.templates.mkdir()
        for name in ['daemon','agent']: (self.templates/name).write_text(name)
        self.tp = patch.object(m,'TEMPLATES',self.templates); self.tp.start()
        self.plan = dict(legacy=[],operation='install',edition='personal',owner_uid=501,version='0.2.0',
                         payload={k:dict(source=str(self.source),sha256=m.tree(self.source),cdhash='a'*40)
                                  for k in ['app','helper','guardian']},
                         templates={k:m.hashlib.sha256((self.templates/k).read_bytes()).hexdigest() for k in ['daemon','agent']},
                         destinations=list(map(str,[self.root/'Application.app',m.HELPER,m.GUARDIAN,m.DAEMON,m.AGENT,m.RECORD])))

    def tearDown(self):
        self.tp.stop()
        for p in reversed(self.patches): p.stop()
        self.temp.cleanup()

    def execute(self):
        self.plan['previous']=m.plan_state(self.plan)
        return m.transact(self.plan,m.digest(self.plan))

    def recover(self):
        # Fixtures are deliberately owned by the test user, not root.
        with patch.object(m,'load_journal',side_effect=lambda:json.loads(m.JOURNAL.read_text())):
            return m.recover(501)

    def test_digest_mismatch_never_stages(self):
        with self.assertRaisesRegex(ValueError,'digest'): m.transact(self.plan,'wrong')
        self.assertFalse(m.JOURNAL.exists())

    def test_payload_changed_after_preview(self):
        self.source.write_text('changed')
        with self.assertRaisesRegex(ValueError,'approved'): self.execute()
        self.assertFalse(m.RECORD.exists())

    def test_first_install_is_pending_and_reversible(self):
        result = self.execute()
        self.assertEqual(result['status'],'pending_component_verification')
        record=json.loads(m.RECORD.read_text())
        self.assertEqual(record['schema_version'],2)
        self.assertEqual(record['trust']['kind'],'source_local')
        self.recover()
        for target in self.plan['destinations']: self.assertFalse(Path(target).exists())
        self.assertFalse(m.JOURNAL.exists())

    def test_replace_restores_every_component_after_each_write_failure(self):
        original_save=m.save
        for fail_key in ['app','helper','guardian','daemon','agent','record','health']:
            with self.subTest(fail_key=fail_key):
                for target in self.plan['destinations']: Path(target).write_text('old')
                m.RECORD.write_text(json.dumps(dict(owner_uid=501,edition='personal')))
                before={v:Path(v).read_bytes() for v in self.plan['destinations']}
                self.plan['operation']='replace'
                def inject(path,value):
                    original_save(path,value)
                    if path==m.JOURNAL and value['pending']==fail_key:
                        raise RuntimeError('power loss')
                with patch.object(m,'save',side_effect=inject):
                    with self.assertRaisesRegex(RuntimeError,'power loss'): self.execute()
                self.recover()
                self.assertEqual(before,{v:Path(v).read_bytes() for v in before})
                m.JOURNAL.unlink(missing_ok=True)
                if m.BARRIER.exists(): m.BARRIER.unlink()

    def test_partial_copy_failure_recovers_first_install(self):
        original_copy=m.copy
        def fail(source,target):
            if target==m.HELPER:
                target.write_text('partial')
                raise RuntimeError('interrupted copy')
            return original_copy(source,target)
        with patch.object(m,'copy',side_effect=fail):
            with self.assertRaisesRegex(RuntimeError,'interrupted copy'): self.execute()
        self.recover()
        for target in self.plan['destinations']: self.assertFalse(Path(target).exists())

    def test_uninstall_is_reversible_and_retains_record_in_backup(self):
        self.execute()
        previous={v:Path(v).read_bytes() for v in self.plan['destinations']}
        m.JOURNAL.unlink(missing_ok=True); m.BARRIER.unlink()
        self.plan['operation']='uninstall'; self.plan['payload']={}
        self.execute()
        for target in self.plan['destinations']: self.assertFalse(Path(target).exists())
        self.recover()
        self.assertEqual(previous,{v:Path(v).read_bytes() for v in previous})

    def test_world_writable_artifact_rejected(self):
        self.source.chmod(0o666)
        with self.assertRaisesRegex(ValueError,'permissions'): m.tree(self.source)

    def test_interrupted_backup_recover_twice_never_removes_components(self):
        for target in self.plan['destinations']: Path(target).write_text('old')
        m.RECORD.write_text(json.dumps(dict(owner_uid=501,edition='personal')))
        before={v:Path(v).read_bytes() for v in self.plan['destinations']}
        self.plan['operation']='replace'
        original_copy=m.copy
        def fail(source,target):
            if target.name=='old-helper':
                raise RuntimeError('backup interrupted')
            return original_copy(source,target)
        with patch.object(m,'copy',side_effect=fail):
            with self.assertRaisesRegex(RuntimeError,'backup interrupted'): self.execute()
        self.assertEqual(self.recover()['status'],'backup_aborted')
        self.assertEqual(self.recover()['status'],'backup_aborted')
        self.assertEqual(before,{v:Path(v).read_bytes() for v in before})
        self.assertFalse(m.JOURNAL.exists())
        self.assertEqual(json.loads(m.LAST_RESULT.read_text())['status'],'backup_aborted')

    def test_standard_applications_permissions_only_exception(self):
        import stat
        good=type('Meta',(),dict(st_mode=stat.S_IFDIR|0o775,st_uid=0,st_gid=80))()
        root=type('Meta',(),dict(st_mode=stat.S_IFDIR|0o755,st_uid=0,st_gid=0))()
        with patch.object(Path,'lstat',side_effect=[good,root]):
            REAL_SAFE_PARENTS(Path('/Applications'))
        with patch.object(Path,'lstat',return_value=good):
            with self.assertRaisesRegex(ValueError,'unsafe root'):
                REAL_SAFE_PARENTS(Path('/Library/PrivilegedHelperTools'))

    def test_missing_fixed_system_directory_is_created(self):
        parent=self.root/'PrivilegedHelperTools'
        with patch.object(m,'HELPER',parent/'helper'):
            m.ensure_system_directories()
        self.assertTrue(parent.is_dir())
        self.assertEqual(parent.stat().st_mode & 0o777,0o755)

    def test_app_publish_refuses_competing_entry_without_following_it(self):
        target=self.root/'Application.app'; target.write_text('old')
        secret=self.root/'untouched'; secret.write_text('keep')
        stage=self.system/'stage'; stage.mkdir()
        original=m.rename_exclusive
        def race(source_fd,source,dest_fd,dest):
            original(source_fd,source,dest_fd,dest)
            if str(dest).startswith('displaced-'):
                target.symlink_to(secret)
        with patch.object(m,'rename_exclusive',side_effect=race):
            with self.assertRaises(FileExistsError):
                m.replace_component(self.source,target,stage,is_app=True)
        self.assertEqual(secret.read_text(),'keep')

    def health_receipt(self):
        record=json.loads(m.RECORD.read_text())
        now=int(m.time.time())
        challenge=dict(schema_version=1,record=record,generation=record['trust']['generation'],
                       nonce='b'*64,boot_session_uuid=m.boot_uuid(),expires_at=now+100)
        receipt=dict(schema_version=1,challenge=challenge,gui_verified=True,guardian_verified=True,
                     guardian_loop_sequence=1,observed_at=now)
        m.save(m.CHALLENGE,challenge,mode=0o644); m.save(m.RECEIPT,receipt)
        return challenge,receipt

    def test_authenticated_receipt_commits_and_retry_is_idempotent(self):
        self.execute(); self.health_receipt()
        with patch.object(m,'verify_processes',return_value=[dict(pid=123)]):
            result=m.finalize(501)
            self.assertEqual(result['status'],'committed')
            self.assertEqual(m.finalize(501)['status'],'committed')
        self.assertFalse(m.JOURNAL.exists()); self.assertFalse(m.BARRIER.exists())
        self.assertTrue((Path(result['backup'])/'result.json').exists())
        with patch.object(m,'verify_processes',return_value=[]):
            self.assertEqual(m.verify(501)['status'],'committed')

    def test_receipt_rejects_missing_role_sequence_and_stale_generation(self):
        self.execute(); challenge,receipt=self.health_receipt()
        for field,value in [('guardian_verified',False),('gui_verified',1),
                            ('guardian_loop_sequence',0),('guardian_loop_sequence',True),('observed_at',0)]:
            with self.subTest(field=field,value=value):
                bad=dict(receipt); bad[field]=value; m.save(m.RECEIPT,bad)
                with self.assertRaisesRegex(ValueError,'receipt rejected'):
                    m.valid_receipt(challenge,challenge['record'])
        m.save(m.RECEIPT,receipt)
        bad=dict(challenge); bad['generation']='0'*32
        with self.assertRaisesRegex(ValueError,'challenge'): m.valid_receipt(bad,challenge['record'])

    def test_receipt_rejects_cross_boot_and_expiry(self):
        self.execute(); challenge,_=self.health_receipt()
        with patch.object(m,'boot_uuid',return_value='different-boot'):
            with self.assertRaisesRegex(ValueError,'challenge'):
                m.valid_receipt(challenge,challenge['record'])
        with patch.object(m.time,'time',return_value=challenge['expires_at']+1):
            with self.assertRaisesRegex(ValueError,'challenge'):
                m.valid_receipt(challenge,challenge['record'])

    def test_commit_rechecks_changed_installed_component(self):
        self.execute(); self.health_receipt(); m.HELPER.write_text('tampered')
        with self.assertRaisesRegex(ValueError,'installed identity'): m.finalize(501)
        self.assertTrue(m.JOURNAL.exists())

    def test_cleanup_crash_can_finish_without_receipt(self):
        self.execute(); self.health_receipt()
        original_unlink=Path.unlink
        def fail(path,*args,**kwargs):
            if path==m.BARRIER: raise RuntimeError('cleanup interrupted')
            return original_unlink(path,*args,**kwargs)
        with patch.object(m,'verify_processes',return_value=[]),patch.object(Path,'unlink',fail):
            with self.assertRaisesRegex(RuntimeError,'cleanup interrupted'): m.finalize(501)
        self.assertFalse(m.RECEIPT.exists())
        self.assertEqual(m.finalize(501)['status'],'committed')
        self.assertFalse(m.JOURNAL.exists())

    def test_uninstall_finalizes_without_forever_pending(self):
        self.execute(); self.health_receipt()
        with patch.object(m,'verify_processes',return_value=[]): m.finalize(501)
        self.plan['operation']='uninstall'; self.plan['payload']={}
        self.execute()
        self.assertEqual(m.finalize(501)['status'],'uninstalled')
        self.assertEqual(m.verify(501)['status'],'uninstalled')
        self.assertFalse(m.JOURNAL.exists())

    def test_challenge_exact_mode_and_complete_record(self):
        self.execute(); j=json.loads(m.JOURNAL.read_text())
        challenge=REAL_CREATE_CHALLENGE(j)
        self.assertEqual(m.CHALLENGE.stat().st_mode & 0o777,0o644)
        self.assertEqual(challenge['record'],json.loads(m.RECORD.read_text()))
        self.assertEqual(len(challenge['nonce']),64)
        self.assertLessEqual(challenge['expires_at']-m.time.time(),120)

    def test_metadata_reader_rejects_user_owned_receipt(self):
        m.RECEIPT.write_text('{}'); m.RECEIPT.chmod(0o600)
        if os.geteuid()!=0:
            with self.assertRaisesRegex(ValueError,'unsafe root metadata'):
                REAL_PRIVATE_JSON(m.RECEIPT,0o600)

    def legacy_fixture(self, registered=True):
        from contextlib import ExitStack
        from types import SimpleNamespace
        uid=os.geteuid()
        path=self.root/'legacy.plist'
        item=dict(label='local.claude-update',domain='gui/'+str(uid),path=str(path),uid=uid,
                  interval=43200,run=False,arguments=['/bin/bash',str(self.root/'user-script'),'update','--auto'])
        data=m.plistlib.dumps(dict(Label=item['label'],ProgramArguments=item['arguments'],
                                  RunAtLoad=False,StartInterval=43200))
        path.write_bytes(data); path.chmod(0o644)
        state={'registered':registered,'commands':[]}
        def runner(*args,**kwargs):
            state['commands'].append(args)
            if args[0]=='/usr/sbin/sysctl':
                return SimpleNamespace(stdout='11111111-1111-1111-1111-111111111111',returncode=0)
            if args[1]=='print-disabled': return SimpleNamespace(stdout='disabled services = {}',returncode=0)
            if args[1]=='print':
                present=state['registered'] if args[2].endswith('/local.claude-update') else False
                return SimpleNamespace(stdout='',stderr='' if present else 'Could not find service fixture',returncode=0 if present else 113)
            if args[1]=='bootout': state['registered']=False
            if args[1]=='bootstrap': state['registered']=True
            return SimpleNamespace(stdout='',returncode=0)
        stack=ExitStack()
        stack.enter_context(patch.object(m,'legacy_specs',return_value=[item]))
        stack.enter_context(patch.object(m,'run',side_effect=runner))
        stack.enter_context(patch.object(m,'legacy_snapshot',side_effect=REAL_LEGACY_SNAPSHOT))
        self.addCleanup(stack.close)
        self.plan['legacy']=m.legacy_snapshot(501)
        return path,data,state

    def test_legacy_migration_and_recovery_restore_only_original_registration(self):
        path,data,state=self.legacy_fixture(registered=True)
        self.execute()
        self.assertFalse(path.exists()); self.assertFalse(state['registered'])
        self.recover(); self.recover()
        self.assertEqual(path.read_bytes(),data); self.assertTrue(state['registered'])
        self.assertFalse(any(c[1] in ('enable','disable') for c in state['commands'] if len(c)>1))

    def test_legacy_unregistered_agent_stays_unregistered_after_recovery(self):
        path,data,state=self.legacy_fixture(registered=False)
        self.execute(); self.recover()
        self.assertEqual(path.read_bytes(),data); self.assertFalse(state['registered'])
        self.assertFalse(any(c[1]=='bootstrap' for c in state['commands'] if len(c)>1))

    def test_legacy_crash_after_removal_restores_plist_and_schedule(self):
        path,data,state=self.legacy_fixture()
        original=m.os.unlink
        def fail(name,*args,**kwargs):
            original(name,*args,**kwargs)
            if name==path.name: raise RuntimeError('legacy removal interrupted')
        with patch.object(m.os,'unlink',side_effect=fail):
            with self.assertRaisesRegex(RuntimeError,'legacy removal interrupted'): self.execute()
        self.recover()
        self.assertEqual(path.read_bytes(),data); self.assertTrue(state['registered'])

    def test_legacy_nondefault_group_restored_exactly(self):
        path,data,_=self.legacy_fixture(registered=False)
        alternate=next(gid for gid in os.getgroups() if gid!=path.parent.stat().st_gid)
        os.chown(path,os.geteuid(),alternate)
        self.plan['legacy']=m.legacy_snapshot(501)
        self.execute(); self.recover(); self.recover()
        self.assertEqual(path.read_bytes(),data)
        self.assertEqual(path.stat().st_gid,alternate)
        self.assertEqual(path.stat().st_uid,os.geteuid())
        self.assertEqual(path.stat().st_mode & 0o777,0o644)
        self.assertFalse(m.JOURNAL.exists())

    def test_legacy_existing_restore_corrects_interrupted_metadata(self):
        path,data,_=self.legacy_fixture(registered=False)
        alternate=next(gid for gid in os.getgroups() if gid!=path.parent.stat().st_gid)
        os.chown(path,os.geteuid(),alternate)
        self.plan['legacy']=m.legacy_snapshot(501)
        self.execute()
        # Model a prior restoration that wrote bytes but died before fixing gid.
        path.write_bytes(data); path.chmod(0o644)
        self.assertNotEqual(path.stat().st_gid,alternate)
        self.recover()
        self.assertEqual(path.stat().st_gid,alternate)
        self.assertFalse(m.JOURNAL.exists())

    def test_legacy_unknown_program_rejected_before_mutation(self):
        path,_,_=self.legacy_fixture()
        value=m.plistlib.loads(path.read_bytes()); value['ProgramArguments']=['/bin/sh','/tmp/unapproved']
        path.write_bytes(m.plistlib.dumps(value))
        with self.assertRaisesRegex(ValueError,'fixed supported template'): m.legacy_snapshot(501)
        self.assertTrue(path.exists()); self.assertFalse(m.JOURNAL.exists())

    def test_legacy_user_symlink_is_never_followed(self):
        path,data,_=self.legacy_fixture()
        other=self.root/'other'; other.write_bytes(data)
        path.unlink(); path.symlink_to(other)
        with self.assertRaises(OSError): m.legacy_snapshot(501)
        self.assertEqual(other.read_bytes(),data)

    def test_approved_previous_components_cannot_change_before_staging(self):
        self.plan['previous']=m.installed_state('personal')
        m.HELPER.write_text('changed after preview')
        self.plan['operation']='replace'
        with self.assertRaisesRegex(ValueError,'changed since preview'):
            m.transact(self.plan,m.digest(self.plan))
        self.assertEqual(m.HELPER.read_text(),'changed after preview')
        self.assertFalse(m.JOURNAL.exists())

    def test_health_timeout_keeps_recoverable_journal(self):
        from unittest.mock import Mock
        self.execute()
        challenge,_=self.health_receipt(); m.RECEIPT.unlink()
        child=Mock(); child.poll.return_value=0
        with patch.object(m,'launch_health',return_value=child), \
             patch.object(m,'create_challenge',return_value=challenge), \
             patch.object(m.time,'monotonic',side_effect=[0,111]):
            with self.assertRaisesRegex(ValueError,'health timed out'): m.finalize(501,launch=True,authenticated=True)
        self.assertTrue(m.JOURNAL.exists())
        self.recover()
        self.assertFalse(m.JOURNAL.exists())

    def test_backup_drift_stops_before_installed_mutation(self):
        for target in self.plan['destinations']: Path(target).write_text('old')
        m.RECORD.write_text(json.dumps(dict(owner_uid=501,edition='personal')))
        self.plan['operation']='replace'
        before={v:Path(v).read_bytes() for v in self.plan['destinations']}
        original=m.copy
        def drift(source,target):
            original(source,target)
            if target.name=='old-helper': target.write_text('changed backup')
        with patch.object(m,'copy',side_effect=drift):
            with self.assertRaisesRegex(ValueError,'backup differs'): self.execute()
        self.assertEqual(before,{v:Path(v).read_bytes() for v in before})
        self.recover(); self.recover()
        self.assertEqual(before,{v:Path(v).read_bytes() for v in before})

    def test_codesign_adhoc_runtime_flags_and_candidate_hashes(self):
        info=('CodeDirectory v=20500 size=37374 flags=0x10002(adhoc,runtime) hashes=1161+3\n'
              'CandidateCDHash sha256='+'b'*40+'\nCandidateCDHashFull sha256='+'b'*64+'\n'
              'CDHash='+'a'*40+'\n')
        self.assertEqual(m.signing_cdhash(info),'a'*40)
        self.assertEqual(m.signing_cdhash(info.replace('0x10002(adhoc,runtime)','0x10000(runtime)')),'a'*40)
        for invalid in [info.replace('0x10002(adhoc,runtime)','0x2(adhoc)'),
                        info.replace('0x10002','0x2'),info+'CDHash='+'c'*40+'\n']:
            with self.assertRaisesRegex(ValueError,'Hardened Runtime'): m.signing_cdhash(invalid)

    def test_existing_install_is_replaced_by_default(self):
        m.HELPER.write_text('old')
        self.execute()
        self.assertEqual(m.HELPER.read_text(), 'new')
        self.recover()
        self.assertEqual(m.HELPER.read_text(), 'old')

    def managed_fixture(self):
        self.execute(); self.health_receipt()
        with patch.object(m, 'verify_processes', return_value=[]): m.finalize(501)
        self.plan['operation'] = 'replace'
        guard = 'b'*64
        original = dict(timezone_original='UTC', timezone_last='Asia/Tokyo',
                        timezone_guard_plan_id=guard, services={'Wi-Fi':['automatic','off']})
        state = dict(timezone='UTC', network_service='Wi-Fi', ipv6='automatic')
        target = dict(timezone='Asia/Tokyo', network_service='Wi-Fi', ipv6='off')
        receipt = dict(owner_uid=501, code='repair_applied',
                       plan=dict(id=guard, original=state, target=target, expires_at=123))
        directory = self.system/'repair-results'; directory.mkdir(mode=0o700)
        paths = [self.system/'system-original.json', directory/(guard+'.json')]
        for path, value in zip(paths, [original, receipt]):
            path.write_text(json.dumps(value)); path.chmod(0o600)
        return paths

    def managed_root_metadata(self):
        from types import SimpleNamespace
        real = os.fstat
        def metadata(fd):
            value = real(fd)
            fields = ['st_mode','st_nlink','st_size','st_uid','st_gid','st_dev','st_ino',
                      'st_mtime_ns','st_ctime_ns']
            return SimpleNamespace(**{key:0 if key=='st_uid' else getattr(value,key) for key in fields})
        return patch.object(m.os, 'fstat', side_effect=metadata)

    def test_repair_records_do_not_gate_install_replace_or_uninstall(self):
        paths = self.managed_fixture()
        pending = self.system/'repair-journal.json'
        pending.write_text('{"incomplete":true}')
        paths.append(pending)
        before = {p:p.read_bytes() for p in paths}
        for operation in ('install', 'replace', 'uninstall'):
            with self.subTest(operation=operation):
                self.plan['operation'] = operation
                if operation == 'uninstall':
                    self.plan['payload'] = {}
                self.execute()
                self.assertEqual(before, {p:p.read_bytes() for p in paths})
                self.recover()
                self.assertEqual(before, {p:p.read_bytes() for p in paths})

    def test_install_commits_without_gui_health_receipt(self):
        self.execute()
        with patch.object(m,'verify_processes',return_value=[]), patch.object(m,'launch_health') as launch:
            result=m.finalize(501,launch=True)
        launch.assert_not_called()
        self.assertEqual(result['status'],'committed')
        self.assertFalse(m.JOURNAL.exists())

    def test_automatic_quit_targets_only_owner_and_waits(self):
        from types import SimpleNamespace
        snapshots = [
            '101 501 Claude\n102 502 Claude\n103 501 /usr/bin/other\n104 501 switcher-desktop',
            '102 502 Claude',
        ]
        with patch.object(m,'run',side_effect=[SimpleNamespace(stdout=x) for x in snapshots]), \
             patch.object(m.os,'kill') as kill:
            REAL_STOPPED(501)
        self.assertEqual([call.args for call in kill.call_args_list],
                         [(101,m.signal.SIGTERM),(104,m.signal.SIGTERM)])

    def test_main_verify_is_unprivileged_and_never_stops_applications(self):
        from contextlib import redirect_stdout
        from io import StringIO
        with patch.object(m,'console_owner',return_value=501), patch.object(m.os,'geteuid',return_value=501), \
             patch.object(m,'verify_public',return_value={'status':'components_verified'}) as verify, \
             patch.object(m,'stopped') as stopped, patch.object(m,'locks') as locks, \
             patch('sys.argv',['local_install.py','verify']), redirect_stdout(StringIO()):
            m.main()
        verify.assert_called_once_with(501,'personal')
        stopped.assert_not_called(); locks.assert_not_called()

    def test_install_without_digest_generates_internal_plan_digest(self):
        from contextlib import nullcontext, redirect_stdout
        from io import StringIO
        self.plan['previous']=m.installed_state('personal')
        with patch.object(m,'console_owner',return_value=501), patch.object(m.os,'geteuid',return_value=0), \
             patch.object(m,'plan',return_value=self.plan), patch.object(m,'locks',return_value=nullcontext()), \
             patch.object(m,'finalize',return_value={'status':'committed'}), \
             patch('sys.argv',['local_install.py','install']), redirect_stdout(StringIO()):
            m.main()
        self.assertEqual(m.HELPER.read_text(),'new')

    def edition_fixture(self):
        patcher=patch.object(m,'app',side_effect=lambda edition:self.root/(edition+'.app'))
        patcher.start(); self.addCleanup(patcher.stop)
        self.plan['destinations'][0]=str(m.app('personal'))
        path,_=self.cli_fixture()
        self.execute()
        with patch.object(m,'verify_processes',return_value=[]): m.finalize(501)
        original=m.app('personal').read_bytes()
        self.plan['edition']='public'
        self.plan['destinations'][0]=str(m.app('public'))
        self.plan['cli_entry']=m.cli_snapshot(501)
        self.plan['previous']=m.installed_state('public')
        m.edition_transition(self.plan)
        self.source.write_text('public candidate')
        for item in self.plan['payload'].values(): item['sha256']=m.tree(self.source)
        return path, original

    def test_same_owner_edition_install_replaces_old_app_and_cli(self):
        path,_=self.edition_fixture()
        self.execute()
        self.assertFalse(m.app('personal').exists())
        self.assertEqual(m.app('public').read_text(),'public candidate')
        self.assertIn(b'--edition public',path.read_bytes())
        with patch.object(m,'verify_processes',return_value=[]):
            self.assertEqual(m.finalize(501)['status'],'committed')
        self.assertEqual(json.loads(m.RECORD.read_text())['edition'],'public')

    def test_same_owner_explicit_replace_also_changes_edition(self):
        path,_=self.edition_fixture()
        self.plan['operation']='replace'
        self.execute()
        self.assertFalse(m.app('personal').exists())
        self.assertTrue(m.app('public').exists())
        self.assertIn(b'--edition public',path.read_bytes())
        with patch.object(m,'verify_processes',return_value=[]): m.finalize(501)

    def test_edition_transition_failure_restores_single_old_app_and_cli(self):
        path, original=self.edition_fixture()
        old_cli=path.read_bytes()
        for phase in ('old_app','app','helper','record','cli-entry','health'):
            with self.subTest(phase=phase):
                save=m.save
                def fail(path,value,**kwargs):
                    save(path,value,**kwargs)
                    if path == m.JOURNAL and value['pending'] == phase:
                        raise RuntimeError('power loss')
                with patch.object(m,'save',side_effect=fail):
                    with self.assertRaisesRegex(RuntimeError,'power loss'): self.execute()
                self.recover()
                self.assertEqual(m.app('personal').read_bytes(),original)
                self.assertFalse(m.app('public').exists())
                self.assertEqual(path.read_bytes(),old_cli)
                self.assertEqual(json.loads(m.RECORD.read_text())['edition'],'personal')

    def test_edition_transition_rejects_old_identity_without_mutation(self):
        self.edition_fixture()
        # Remove the prepared transition, then re-evaluate the real old record.
        self.plan.pop('previous_edition')
        self.plan['destinations'].pop()
        self.plan['previous'].pop('old_app')
        record=json.loads(m.RECORD.read_text())
        for field,value in [('trust',dict(gui_cdhash='b'*40))]:
            with self.subTest(field=field):
                changed=dict(record); changed[field]=value; m.RECORD.write_text(json.dumps(changed))
                with self.assertRaises(ValueError): m.edition_transition(self.plan)
                self.assertTrue(m.app('personal').exists())
                self.assertFalse(m.app('public').exists())
                self.assertFalse(m.JOURNAL.exists())
        m.RECORD.write_text(json.dumps(record))

    def owner_handoff_fixture(self, edition='personal'):
        from contextlib import nullcontext
        from types import SimpleNamespace
        patcher=patch.object(m,'app',side_effect=lambda edition:self.root/(edition+'.app'))
        patcher.start(); self.addCleanup(patcher.stop)
        self.plan['destinations'][0]=str(m.app('personal'))
        old_cli,_=self.cli_fixture()
        self.execute()
        with patch.object(m,'verify_processes',return_value=[]): m.finalize(501)
        old_home=old_cli.parents[2]
        for relative in ['.claude-switcher/accounts/A/data','.claude-switcher/v2/config.json']:
            path=old_home/relative; path.parent.mkdir(parents=True,exist_ok=True); path.write_text('private old owner data')
        original={str(path.relative_to(old_home)):path.read_bytes() for path in old_home.rglob('*') if path.is_file()}
        new_home=self.root/'new-owner'; new_home.mkdir()
        patcher=patch.object(m.pwd,'getpwuid',side_effect=lambda uid:SimpleNamespace(pw_dir=str(old_home if uid==501 else new_home)))
        patcher.start(); self.addCleanup(patcher.stop)
        original_small=m.small_file
        def metadata(path):
            data,meta=original_small(path)
            if Path(path).is_relative_to(new_home):
                meta=SimpleNamespace(st_uid=502,st_gid=20,st_mode=meta.st_mode)
            return data,meta
        patcher=patch.object(m,'small_file',side_effect=metadata); patcher.start(); self.addCleanup(patcher.stop)
        self.plan.update(owner_uid=502,edition=edition,cli_entry=m.cli_snapshot(502))
        self.plan['destinations'][0]=str(m.app(edition))
        self.plan['previous']=m.installed_state(edition)
        m.edition_transition(self.plan)
        active={'system/local.claudeswitcher.helper','gui/501/local.claudeswitcher.guardian'}
        def run(*args,**kwargs):
            return SimpleNamespace(returncode=0 if args[-1] in active else 1,stdout='11111111-1111-1111-1111-111111111111')
        def service(action,owner,check=True,only=None):
            for target in ['system/local.claudeswitcher.helper','gui/'+str(owner)+'/local.claudeswitcher.guardian']:
                if only is not None and target not in only: continue
                if action=='bootout': active.discard(target)
                else: active.add(target)
        for patcher in [patch.object(m,'run',side_effect=run),patch.object(m,'service',side_effect=service)]:
            patcher.start(); self.addCleanup(patcher.stop)
        return old_home, original, new_home/'.local/bin/claude-switch.sh', active

    def assert_old_owner_untouched(self, home, expected):
        self.assertEqual({str(path.relative_to(home)):path.read_bytes() for path in home.rglob('*') if path.is_file()},expected)

    def test_admin_owner_handoff_same_edition_commits_without_old_data_changes(self):
        home,original,cli,active=self.owner_handoff_fixture()
        self.execute()
        with patch.object(m,'verify_processes',return_value=[]): self.assertEqual(m.finalize(502)['status'],'committed')
        self.assertEqual(json.loads(m.RECORD.read_text())['owner_uid'],502)
        self.assertEqual(active,{'system/local.claudeswitcher.helper','gui/502/local.claudeswitcher.guardian'})
        self.assertIn(b'--edition personal',cli.read_bytes())
        self.assert_old_owner_untouched(home,original)

    def test_admin_owner_handoff_and_edition_change_commits(self):
        home,original,cli,active=self.owner_handoff_fixture('public')
        self.execute()
        with patch.object(m,'verify_processes',return_value=[]): m.finalize(502)
        self.assertFalse(m.app('personal').exists()); self.assertTrue(m.app('public').exists())
        self.assertIn(b'--edition public',cli.read_bytes())
        self.assertNotIn('gui/501/local.claudeswitcher.guardian',active)
        self.assert_old_owner_untouched(home,original)

    def test_owner_handoff_failure_restores_record_registration_and_new_cli(self):
        home,original,cli,active=self.owner_handoff_fixture('public')
        for phase in ['services','old_app','record','cli-entry','health']:
            with self.subTest(phase=phase):
                save=m.save
                def fail(path,value,**kwargs):
                    save(path,value,**kwargs)
                    if path==m.JOURNAL and value['pending']==phase: raise RuntimeError('power loss')
                with patch.object(m,'save',side_effect=fail):
                    with self.assertRaisesRegex(RuntimeError,'power loss'): self.execute()
                m.recover(502)
                self.assertEqual(json.loads(m.RECORD.read_text())['owner_uid'],501)
                self.assertEqual(active,{'system/local.claudeswitcher.helper','gui/501/local.claudeswitcher.guardian'})
                self.assertFalse(cli.exists()); self.assertTrue(m.app('personal').exists())
                self.assertFalse(m.app('public').exists())
                self.assert_old_owner_untouched(home,original)

    def test_owner_handoff_partial_new_registration_failure_recovers(self):
        home,original,cli,active=self.owner_handoff_fixture()
        service=m.service.side_effect
        def fail(action,owner,**kwargs):
            service(action,owner,**kwargs)
            if action=='bootstrap' and owner==502 and kwargs.get('only') is None:
                raise RuntimeError('new guardian registration interrupted')
        with patch.object(m,'service',side_effect=fail):
            with self.assertRaisesRegex(RuntimeError,'registration interrupted'): self.execute()
        m.recover(502)
        self.assertEqual(json.loads(m.RECORD.read_text())['owner_uid'],501)
        self.assertEqual(active,{'system/local.claudeswitcher.helper','gui/501/local.claudeswitcher.guardian'})
        self.assertFalse(cli.exists()); self.assert_old_owner_untouched(home,original)

    def test_owner_handoff_rollback_restores_existing_new_owner_cli(self):
        home,original,cli,active=self.owner_handoff_fixture()
        cli.parent.mkdir(parents=True); cli.write_bytes(b'#!/bin/bash\nPROXY_MODE="auto"\n# new owner original entry\n'); cli.chmod(0o700)
        prior=cli.read_bytes(); self.plan['cli_entry']=m.cli_snapshot(502)
        self.execute(); m.recover(502)
        self.assertEqual(cli.read_bytes(),prior); self.assertEqual(cli.stat().st_mode & 0o777,0o700)
        self.assert_old_owner_untouched(home,original)

    def test_owner_record_race_rejects_before_writes(self):
        home,original,cli,active=self.owner_handoff_fixture()
        record=json.loads(m.RECORD.read_text()); record['owner_uid']=503; m.RECORD.write_text(json.dumps(record))
        before={str(path):path.read_bytes() for path in self.system.rglob('*') if path.is_file()}
        with patch.object(m,'ensure_system_directories') as ensure:
            with self.assertRaisesRegex(ValueError,'previous owner changed'): m.transact(self.plan,m.digest(self.plan))
            ensure.assert_not_called()
        self.assertEqual({str(path):path.read_bytes() for path in self.system.rglob('*') if path.is_file()},before)
        self.assertFalse(cli.exists()); self.assert_old_owner_untouched(home,original)

    def test_uninstall_other_owner_is_still_rejected(self):
        self.owner_handoff_fixture()
        self.plan['operation']='uninstall'
        with self.assertRaisesRegex(ValueError,'current owner'): m.edition_transition(self.plan)

    def test_dual_lifecycle_locks_are_ordered_and_root_lock_taken_once(self):
        from contextlib import contextmanager
        from types import SimpleNamespace
        order=[]
        @contextmanager
        def lifecycle(uid):
            order.append(uid); yield
        with patch.object(m,'lifecycle_lock',side_effect=lifecycle), patch.object(m.os,'fstat',return_value=SimpleNamespace(st_mode=0o100600,st_uid=0,st_nlink=1)), patch.object(m.fcntl,'flock') as flock:
            with m.locks(502,501): pass
        self.assertEqual(order,[501,502]); self.assertEqual(flock.call_count,1)

    def cli_fixture(self, existing=True):
        from contextlib import nullcontext
        from types import SimpleNamespace
        home=self.root/'owner-home'; home.mkdir()
        for patcher in [patch.object(m.pwd,'getpwuid',return_value=SimpleNamespace(pw_dir=str(home))),
                        patch.object(m,'as_owner',side_effect=lambda owner:nullcontext())]:
            patcher.start(); self.addCleanup(patcher.stop)
        path=home/'.local/bin/claude-switch.sh'
        old=b'#!/bin/bash\nCOUNTRY_TARGET="JP"\nTZ_TARGET="Asia/Tokyo"\nLOCALE_TARGET="ja_JP"\nPROXY_MODE="clash-verge"\nPROXY_GROUP="Group $Dollar"\nexit 99\n'
        if existing:
            path.parent.mkdir(parents=True); path.write_bytes(old); path.chmod(0o755)
        self.plan['cli_entry']=m.cli_snapshot(501)
        return path, old

    def test_cli_entry_preserves_all_configuration_and_rolls_back(self):
        path, old=self.cli_fixture()
        expected=m.config_literals(old)
        self.execute()
        shim=path.read_bytes()
        self.assertEqual(m.config_literals(shim),expected)
        self.assertNotIn(b'exit 99',shim)
        self.assertIn(b'/Contents/Resources/components/claude-switcher',shim)
        self.assertIn(b'--edition personal "$@"',shim)
        journal=json.loads(m.JOURNAL.read_text())
        m.cli_verify(journal)
        self.recover()
        self.assertEqual(path.read_bytes(),old)
        self.assertEqual(path.stat().st_mode & 0o777,0o755)

    def test_fresh_cli_entry_is_removed_on_transaction_rollback(self):
        path,_=self.cli_fixture(existing=False)
        self.execute(); self.assertTrue(path.is_file())
        self.recover(); self.assertFalse(path.exists())

    def test_cli_change_after_preview_never_overwrites_user_file(self):
        path,_=self.cli_fixture()
        path.write_text('changed by user')
        with self.assertRaisesRegex(ValueError,'changed since preview'): self.execute()
        self.assertEqual(path.read_text(),'changed by user')

    def test_cli_shim_quotes_dollars_and_quotes_and_forwards_arguments(self):
        import subprocess
        entry=dict(literals={'PROXY_GROUP':"Group $HOME ' quoted"})
        executable=self.root/'Application.app/Contents/Resources/components/claude-switcher'
        executable.parent.mkdir(parents=True)
        executable.write_text('#!/bin/bash\nprintf "%s\\n" "$@"\n'); executable.chmod(0o755)
        path=self.root/'shim.sh'; path.write_bytes(m.cli_shim('public',entry))
        self.assertEqual(m.config_literals(path.read_bytes()),entry['literals'])
        output=subprocess.run(['/bin/bash',str(path),'checkdetail','arg with spaces'],capture_output=True,text=True,check=True)
        self.assertEqual(output.stdout.splitlines(),['--edition','public','checkdetail','arg with spaces'])

    def fresh_plan_fixture(self):
        path,_=self.cli_fixture(existing=False)
        self.watchdog_fixture(); self.bodyguard_fixture()
        source=self.root/'Fresh.app'
        components=source/'Contents/Resources/components'; components.mkdir(parents=True)
        for name in ['switcher-helper','switcher-guardian','claude-switcher']:
            binary=components/name; binary.write_text('fixed fixture'); binary.chmod(0o755)
        (source/'Contents/Info.plist').write_bytes(m.plistlib.dumps(dict(CFBundleShortVersionString='0.2.0')))
        return path,source

    def test_production_fresh_plan_to_commit_uses_one_detected_configuration(self):
        from types import SimpleNamespace
        path,source=self.fresh_plan_fixture()
        calls=[]
        def run(*args,**kwargs):
            calls.append(args)
            output='11111111-1111-1111-1111-111111111111'
            if args[0]=='/usr/bin/curl':
                output='loc=JP\n' if 'cloudflare' in args[-1] else '{"country":"JP","timezone":"Asia/Tokyo"}'
            if args[0]=='/usr/sbin/scutil': output='(Connected) com.liguangming.Shadowrocket'
            return SimpleNamespace(returncode=0,stdout=output)
        with patch.object(m,'run',side_effect=run):
            planned=m.plan('public',source,501)
            self.assertFalse(path.exists()); self.assertFalse(m.JOURNAL.exists())
            expected=dict(COUNTRY_TARGET='JP',TZ_TARGET='Asia/Tokyo',LOCALE_TARGET='ja_JP',PROXY_MODE='shadowrocket',PROXY_GROUP='GLOBAL')
            self.assertEqual(planned['cli_entry']['install_literals'],expected)
            m.transact(planned,m.digest(planned))
            with patch.object(m,'watchdog_verify'), patch.object(m,'verify_processes',return_value=[]):
                self.assertEqual(m.finalize(501)['status'],'committed')
            self.assertEqual(m.config_literals(path.read_bytes()),expected)
            self.assertIn(b'--edition public',path.read_bytes())
            self.assertEqual(len([call for call in calls if call[0]=='/usr/bin/curl']),2)

    def test_fresh_plan_rejects_cn_missing_data_and_mismatch_without_writes(self):
        from types import SimpleNamespace
        path,source=self.fresh_plan_fixture()
        for trace,info in [('loc=CN','{}'),('','{}'),('loc=JP','{}'),('loc=JP','{"country":"US","timezone":"Asia/Tokyo"}'),('loc=JP','{"country":"JP"}')]:
            def run(*args,**kwargs):
                return SimpleNamespace(returncode=0,stdout=trace if 'cloudflare' in args[-1] else info)
            with self.subTest(trace=trace,info=info), patch.object(m,'run',side_effect=run):
                with self.assertRaises(ValueError): m.plan('personal',source,501)
                self.assertFalse(path.exists()); self.assertFalse(m.JOURNAL.exists())
                self.assertFalse(m.RECORD.exists())

    def test_existing_config_plan_never_probes_network_and_keeps_group_literal(self):
        path,source=self.fresh_plan_fixture()
        path.parent.mkdir(parents=True)
        path.write_text("COUNTRY_TARGET='JP'\nTZ_TARGET='Asia/Tokyo'\nLOCALE_TARGET='ja_JP'\nPROXY_MODE='surge'\nPROXY_GROUP='Dollar $Group'\n")
        with patch.object(m,'run',side_effect=AssertionError('no external probe')):
            planned=m.plan('personal',source,501)
        self.assertEqual(planned['cli_entry']['install_literals']['PROXY_GROUP'],'Dollar $Group')
        self.assertEqual(m.config_literals(m.cli_shim('personal',planned['cli_entry']))['PROXY_MODE'],'surge')

    def test_install_rejected_discovery_never_locks_or_stops_apps(self):
        with patch.object(m,'console_owner',return_value=501), patch.object(m.os,'geteuid',return_value=0), patch.object(m,'plan',side_effect=ValueError('首次安装需要非 CN 出口')), patch.object(m,'locks') as locks, patch.object(m,'stopped') as stopped, patch('sys.argv',['local_install.py','install','--app','/fixture.app']):
            with self.assertRaisesRegex(ValueError,'非 CN'): m.main()
            locks.assert_not_called(); stopped.assert_not_called()

    def test_v2_configuration_plan_has_priority_without_external_probe(self):
        path,source=self.fresh_plan_fixture()
        config=path.parents[2]/'.claude-switcher/v2/config.json'
        config.parent.mkdir(parents=True)
        config.write_text('{"country_target":"JP","timezone_target":"Asia/Tokyo","locale_target":"ja_JP","proxy_mode":"auto"}')
        with patch.object(m,'run',side_effect=AssertionError('no external probe')):
            planned=m.plan('personal',source,501)
        self.assertEqual(planned['cli_entry']['install_literals'],dict(COUNTRY_TARGET='JP',TZ_TARGET='Asia/Tokyo',LOCALE_TARGET='ja_JP',PROXY_MODE='auto',PROXY_GROUP='GLOBAL'))

    def test_proxy_detection_falls_back_to_old_auto_mode(self):
        from types import SimpleNamespace
        path,_=self.fresh_plan_fixture()
        with patch.object(m,'run',return_value=SimpleNamespace(stdout='',returncode=0)):
            self.assertEqual(m.detect_proxy(path.parents[2]),dict(PROXY_MODE='auto',PROXY_GROUP='GLOBAL'))

    def test_indentless_proxy_groups_stop_only_at_next_top_level_mapping(self):
        path,_=self.fresh_plan_fixture()
        home=path.parents[2]
        directory=home/'Library/Application Support/io.github.clash-verge-rev.clash-verge-rev'
        directory.mkdir(parents=True)
        config=directory/'clash-verge.yaml'
        config.write_text('proxy-groups:\n- name: "Proxy $Group"\n  type: select\nrules:\n- name: Wrong\n  type: select\n')
        self.assertEqual(m.detect_proxy(home)['PROXY_GROUP'],'Proxy $Group')
        config.write_text('proxy-groups:\n- name: NotSelect\n  type: url-test\nrules:\n- name: Wrong\n  type: select\n')
        self.assertEqual(m.detect_proxy(home)['PROXY_GROUP'],'GLOBAL')

    def test_discovery_curl_inherits_only_proxy_environment(self):
        proxies={key:value for key,value in [('http_proxy','http://127.0.0.1:17891'),('HTTP_PROXY','http://127.0.0.1:17892'),('https_proxy','http://127.0.0.1:17893'),('HTTPS_PROXY','http://127.0.0.1:17894'),('all_proxy','socks5://127.0.0.1:17895'),('ALL_PROXY','socks5://127.0.0.1:17896'),('no_proxy','localhost'),('NO_PROXY','127.0.0.1')]}
        with patch.dict(m.os.environ,dict(proxies,UNRELATED_SECRET='not-inherited'),clear=True), patch.object(m.subprocess,'run') as execute:
            REAL_RUN('/usr/bin/curl','https://ipinfo.io/json',inherit_proxy=True)
            environment=execute.call_args.kwargs['env']
            self.assertEqual({key:environment[key] for key in proxies},proxies)
            self.assertNotIn('UNRELATED_SECRET',environment)
            REAL_RUN('/bin/launchctl','print','system/fixture')
            self.assertFalse(set(proxies) & set(execute.call_args.kwargs['env']))
            with self.assertRaises(ValueError): REAL_RUN('/bin/launchctl','print',inherit_proxy=True)

    def test_geo_fallback_handles_429_bad_json_missing_fields_and_provider_shapes(self):
        from types import SimpleNamespace
        self.watchdog_fixture()
        expected=dict(COUNTRY_TARGET='JP',TZ_TARGET='Asia/Tokyo',LOCALE_TARGET='ja_JP')
        cases=[
            [m.subprocess.CalledProcessError(22,['curl']), '{"country_code":"JP","timezone":{"id":"Asia/Tokyo"}}'],
            ['bad json', '{}', '{"country_code":"JP","timezone":"Asia/Tokyo"}'],
            ['{"country":"US","timezone":"Asia/Tokyo"}', '{"country_code":"JP","timezone":{"id":"Asia/Tokyo"}}'],
        ]
        endpoints=['https://www.cloudflare.com/cdn-cgi/trace','https://ipinfo.io/json','https://ipwho.is/','https://api.ip.sb/geoip']
        for replies in cases:
            seen=[]; pending=list(replies)
            def run(*args,**kwargs):
                seen.append(args[-1]); self.assertTrue(kwargs['inherit_proxy'])
                reply='loc=JP\n' if len(seen)==1 else pending.pop(0)
                if isinstance(reply,Exception): raise reply
                return SimpleNamespace(stdout=reply)
            with self.subTest(replies=replies),patch.object(m,'run',side_effect=run):
                self.assertEqual(m.detect_region(),expected)
                self.assertEqual(seen,endpoints[:len(seen)])

    def test_exhausted_geo_fallback_fails_production_plan_without_writes(self):
        from types import SimpleNamespace
        path,source=self.fresh_plan_fixture()
        seen=[]
        def run(*args,**kwargs):
            seen.append(args[-1])
            return SimpleNamespace(stdout='loc=JP\n' if 'cloudflare' in args[-1] else '{}')
        with patch.object(m,'run',side_effect=run):
            with self.assertRaisesRegex(ValueError,'完整地区数据'): m.plan('personal',source,501)
        self.assertEqual(len(seen),4)
        self.assertFalse(path.exists()); self.assertFalse(m.JOURNAL.exists()); self.assertFalse(m.RECORD.exists())

    def test_clash_detection_reads_first_select_group_without_evaluating_dollar(self):
        path,_=self.fresh_plan_fixture()
        home=path.parents[2]
        directory=home/'Library/Application Support/io.github.clash-verge-rev.clash-verge-rev'
        directory.mkdir(parents=True)
        (directory/'clash-verge.yaml').write_text('proxy-groups:\n  - name: "Group $Dollar"\n    type: select\n  - name: Other\n    type: select\n')
        with patch.object(m,'run',side_effect=AssertionError('no external probe')):
            self.assertEqual(m.detect_proxy(home),dict(PROXY_MODE='clash-verge',PROXY_GROUP='Group $Dollar'))

    def bodyguard_fixture(self):
        self.plan['proxy_mode'] = 'auto'
        patcher=patch.object(m,'BODYGUARD_PLIST',self.root/'bodyguard.plist')
        patcher.start(); self.addCleanup(patcher.stop)

    def test_bodyguard_fixed_template_and_fresh_registration_failure_rollback(self):
        from types import SimpleNamespace
        self.bodyguard_fixture()
        def run(*args, **kwargs):
            if args[:3] == ('/bin/launchctl','bootstrap','system') and args[-1] == m.BODYGUARD_PLIST:
                raise RuntimeError('registration failed')
            return SimpleNamespace(returncode=1,stdout='11111111-1111-1111-1111-111111111111')
        with patch.object(m,'run',side_effect=run):
            with self.assertRaisesRegex(RuntimeError,'registration failed'): self.execute()
            script=self.system/'claude-bodyguard.sh'
            self.assertIn('MODE="auto"',script.read_text())
            self.assertIn("pkill -f '/Applications/Claude.app/Contents/MacOS/Claude'",script.read_text())
            self.assertEqual(script.stat().st_mode & 0o777,0o755)
            plist=m.plistlib.loads(m.BODYGUARD_PLIST.read_bytes())
            self.assertEqual(plist['StartInterval'],10)
            self.assertTrue(plist['RunAtLoad'])
            self.assertEqual(plist['ProgramArguments'],['/bin/bash',str(script)])
            self.recover()
            self.assertFalse(script.exists()); self.assertFalse(m.BODYGUARD_PLIST.exists())

    def test_bodyguard_restores_old_bytes_and_registration_after_failure(self):
        from types import SimpleNamespace
        self.bodyguard_fixture()
        script=self.system/'claude-bodyguard.sh'; script.write_text('old protected script'); script.chmod(0o755)
        m.BODYGUARD_PLIST.write_bytes(b'old protected plist'); m.BODYGUARD_PLIST.chmod(0o644)
        original=m.small_file
        def root_file(path):
            data, metadata=original(path)
            return data, SimpleNamespace(st_uid=0,st_gid=0,st_mode=metadata.st_mode)
        bootstraps=[]
        def run(*args, **kwargs):
            if args[:3] == ('/bin/launchctl','bootstrap','system') and args[-1] == m.BODYGUARD_PLIST:
                bootstraps.append(args)
                if len(bootstraps)==1: raise RuntimeError('registration failed')
            return SimpleNamespace(returncode=0,stdout='11111111-1111-1111-1111-111111111111')
        with patch.object(m,'small_file',side_effect=root_file), patch.object(m,'run',side_effect=run):
            with self.assertRaisesRegex(RuntimeError,'registration failed'): self.execute()
            self.recover()
        self.assertEqual(script.read_text(),'old protected script')
        self.assertEqual(m.BODYGUARD_PLIST.read_bytes(),b'old protected plist')
        self.assertEqual(len(bootstraps),2)

    def test_bodyguard_rejects_mode_injection_and_user_owned_script(self):
        self.bodyguard_fixture()
        for mode in ['$(id)','auto";id','../auto','AUTO','']:
            with self.subTest(mode=mode), self.assertRaises(ValueError): m.bodyguard_payload(mode)
        script=self.system/'claude-bodyguard.sh'; script.write_text('user script'); script.chmod(0o755)
        with self.assertRaisesRegex(ValueError,'unsafe existing'): self.execute()
        self.assertEqual(script.read_text(),'user script')

    def test_bodyguard_payload_matches_reviewed_legacy_template(self):
        legacy=(Path(__file__).parents[2]/'daemons/claude-bodyguard.sh').read_text()
        for mode in ['auto','clash-verge','shadowrocket','sing-box','clashx','surge']:
            self.assertEqual(m.bodyguard_payload(mode)[0][2],legacy.replace('__PROXY_MODE__',mode).encode())

    def test_proxy_mode_reads_config_then_safe_shim_literal(self):
        from types import SimpleNamespace
        home=self.root/'home'; (home/'.local/bin').mkdir(parents=True)
        (home/'.local/bin/claude-switch.sh').write_text("PROXY_MODE='surge'\n")
        with patch.object(m.pwd,'getpwuid',return_value=SimpleNamespace(pw_dir=str(home))):
            self.assertEqual(m.read_proxy_mode(501),'surge')
            config=home/'.claude-switcher/v2/config.json'; config.parent.mkdir(parents=True)
            config.write_text('{"proxy_mode":"auto"}')
            self.assertEqual(m.read_proxy_mode(501),'auto')
            config.write_text('{"proxy_mode":"$(id)"}')
            with self.assertRaises(ValueError): m.read_proxy_mode(501)

    def root_metadata_fixture(self, uid=0):
        from contextlib import contextmanager
        from types import SimpleNamespace
        original=m.os.fstat
        groups={}
        @contextmanager
        def context():
            def metadata(fd):
                value=original(fd)
                return SimpleNamespace(st_uid=uid,st_gid=groups.get(value.st_ino,80),st_mode=value.st_mode,
                                       st_size=value.st_size,st_nlink=value.st_nlink,st_dev=value.st_dev,st_ino=value.st_ino)
            def chown(fd,owner,group):
                self.assertEqual((owner,group),(0,0))
                groups[original(fd).st_ino]=group
            with patch.object(m.os,'fstat',side_effect=metadata), patch.object(m.os,'fchown',side_effect=chown) as changed:
                yield changed
        return context()

    def test_fixed_service_fd_normalizes_admin_inheritance_without_real_chown(self):
        first=self.system/'script'; first.write_bytes(b'fixed script'); first.chmod(0o755)
        second=self.system/'plist'; second.write_bytes(b'fixed plist'); second.chmod(0o644)
        with self.root_metadata_fixture() as changed:
            REAL_ROOT_SERVICE_OWNERSHIP([(first,b'fixed script',0o755),(second,b'fixed plist',0o644)])
            self.assertEqual(changed.call_count,2)
            REAL_ROOT_SERVICE_OWNERSHIP([(first,b'fixed script',0o755),(second,b'fixed plist',0o644)])
            self.assertEqual(changed.call_count,2)

    def test_pending_resume_repairs_only_gid_without_replacing_app_or_backups(self):
        self.watchdog_fixture(); self.bodyguard_fixture(); self.execute()
        stage=json.loads(m.JOURNAL.read_text())['stage']
        with self.root_metadata_fixture() as changed, patch.object(m,'root_service_ownership',side_effect=REAL_ROOT_SERVICE_OWNERSHIP), patch.object(m,'replace_component') as replace, patch.object(m,'verify_processes',return_value=[]), patch.object(m,'run',return_value=type('R',(),{'returncode':0})()):
            self.assertEqual(m.finalize(501)['status'],'committed')
            self.assertEqual(changed.call_count,4)
            replace.assert_not_called()
        self.assertEqual(json.loads(m.LAST_RESULT.read_text())['stage'],stage)

    def test_bad_pending_service_preserves_pending_without_any_chown(self):
        self.watchdog_fixture(); self.execute()
        script=self.system/'tz-watchdog.sh'
        expected=script.read_bytes()
        for fault in ['bytes','mode','symlink','hardlink','uid']:
            with self.subTest(fault=fault):
                if script.exists() or script.is_symlink(): script.unlink()
                script.write_bytes(expected); script.chmod(0o755)
                other=self.system/'other'
                if other.exists(): other.unlink()
                if fault=='bytes': script.write_bytes(b'changed')
                if fault=='mode': script.chmod(0o700)
                if fault=='symlink': script.rename(other); script.symlink_to(other)
                if fault=='hardlink': os.link(script,other)
                with self.root_metadata_fixture(uid=501 if fault=='uid' else 0) as changed, patch.object(m,'root_service_ownership',side_effect=REAL_ROOT_SERVICE_OWNERSHIP):
                    with self.assertRaises((ValueError,OSError)): m.finalize(501)
                    changed.assert_not_called()
                self.assertEqual(json.loads(m.JOURNAL.read_text())['pending'],'health')

    def test_restore_explicitly_normalizes_copied_service_target_group(self):
        from types import SimpleNamespace
        self.watchdog_fixture()
        script=self.system/'tz-watchdog.sh'; script.write_bytes(b'old root script'); script.chmod(0o755)
        original=m.small_file
        def root_file(path):
            data,meta=original(path)
            return data,SimpleNamespace(st_uid=0,st_gid=0,st_mode=meta.st_mode)
        with patch.object(m,'small_file',side_effect=root_file): self.execute()
        with patch.object(m,'root_service_ownership') as ownership:
            self.recover()
            self.assertIn(((script,b'old root script',0o755),),[tuple(call.args[0]) for call in ownership.call_args_list])
        self.assertEqual(script.read_bytes(),b'old root script')

    def test_root_service_install_explicitly_materializes_wheel(self):
        self.watchdog_fixture()
        with patch.object(m,'root_service_ownership',create=True) as ownership:
            self.execute()
            self.assertTrue(ownership.called, 'install must set root:wheel after copying into admin-group parent')

    def test_pending_health_normalizes_service_group_before_strict_verify(self):
        self.watchdog_fixture()
        self.execute()
        with patch.object(m,'root_service_ownership',create=True) as ownership, patch.object(m,'watchdog_verify'), patch.object(m,'verify_processes',return_value=[]):
            m.finalize(501)
            self.assertTrue(ownership.called, 'pending health must normalize trusted fixed service files')

    def watchdog_fixture(self):
        self.plan['timezone_target'] = 'Asia/Tokyo'
        zone = self.root/'zoneinfo/Asia'
        zone.mkdir(parents=True)
        (zone/'Tokyo').write_text('timezone fixture')
        for name, value in [('ZONEINFO', self.root/'zoneinfo'),
                            ('WATCHDOG_PLIST', self.root/'watchdog.plist')]:
            patcher = patch.object(m,name,value); patcher.start(); self.addCleanup(patcher.stop)

    def test_fresh_watchdog_is_fixed_root_template_and_recoverable(self):
        self.watchdog_fixture()
        with patch.object(m,'run',return_value=type('R',(),{'returncode':1,'stdout':'11111111-1111-1111-1111-111111111111'})()) as run:
            self.execute()
            script=self.system/'tz-watchdog.sh'
            self.assertIn('TARGET="Asia/Tokyo"',script.read_text())
            self.assertEqual(script.stat().st_mode & 0o777,0o755)
            plist=m.plistlib.loads(m.WATCHDOG_PLIST.read_bytes())
            self.assertEqual(plist['ProgramArguments'],['/bin/bash',str(script)])
            self.assertEqual(plist['StartInterval'],3600)
            self.assertTrue(plist['RunAtLoad'])
            self.assertIn(('/bin/launchctl','bootstrap','system',m.WATCHDOG_PLIST),
                          [call.args for call in run.call_args_list])
            self.recover()
            self.assertFalse(script.exists()); self.assertFalse(m.WATCHDOG_PLIST.exists())

    def test_watchdog_registration_failure_restores_previous_script(self):
        from types import SimpleNamespace
        self.watchdog_fixture()
        script=self.system/'tz-watchdog.sh'; script.write_text('old root script'); script.chmod(0o755)
        original_small_file=m.small_file
        def root_file(path):
            data, metadata=original_small_file(path)
            return data, SimpleNamespace(st_uid=0, st_gid=0, st_mode=metadata.st_mode)
        def run(*args, **kwargs):
            if args[:3] == ('/bin/launchctl','bootstrap','system') and args[-1] == m.WATCHDOG_PLIST:
                raise RuntimeError('registration failed')
            return SimpleNamespace(returncode=1,stdout='11111111-1111-1111-1111-111111111111')
        with patch.object(m,'small_file',side_effect=root_file), patch.object(m,'run',side_effect=run):
            with self.assertRaisesRegex(RuntimeError,'registration failed'): self.execute()
            self.recover()
        self.assertEqual(script.read_text(),'old root script')
        self.assertFalse(m.WATCHDOG_PLIST.exists())

    def test_watchdog_rejects_injection_and_user_owned_existing_script(self):
        self.watchdog_fixture()
        for zone in ['../../etc/passwd','Asia/Tokyo";bad','$(id)','/Asia/Tokyo','Unknown/Zone']:
            with self.subTest(zone=zone), self.assertRaises(ValueError): m.watchdog_payload(zone)
        script=self.system/'tz-watchdog.sh'; script.write_text('untrusted'); script.chmod(0o755)
        with self.assertRaisesRegex(ValueError,'unsafe existing timezone watchdog'): self.execute()
        self.assertEqual(script.read_text(),'untrusted')

    def test_read_timezone_uses_json_then_literal_legacy_without_execution(self):
        from types import SimpleNamespace
        self.watchdog_fixture()
        home=self.root/'home'; (home/'.local/bin').mkdir(parents=True)
        legacy=home/'.local/bin/claude-switch.sh'
        legacy.write_text('TZ_TARGET="Asia/Tokyo" # old target\n')
        with patch.object(m.pwd,'getpwuid',return_value=SimpleNamespace(pw_dir=str(home))):
            self.assertEqual(m.read_timezone_target(501),'Asia/Tokyo')
            legacy.write_text('TZ_TARGET="$(touch /tmp/not-executed)"\n')
            with self.assertRaises(ValueError): m.read_timezone_target(501)
            config=home/'.claude-switcher/v2/config.json'; config.parent.mkdir(parents=True)
            config.write_text('{"timezone_target":"Asia/Tokyo"}')
            self.assertEqual(m.read_timezone_target(501),'Asia/Tokyo')

    def test_main_replace_accepts_actual_public_plan_digest(self):
        from contextlib import nullcontext, redirect_stdout
        from io import StringIO
        import copy
        paths = self.managed_fixture()
        self.plan['operation'] = 'install'
        self.plan['previous'] = m.installed_state('personal')
        candidate = copy.deepcopy(self.plan)
        candidate['edition'] = 'public'
        record = json.loads(m.RECORD.read_text()); record['edition'] = 'public'
        m.RECORD.write_text(json.dumps(record)); m.RECORD.chmod(0o644)
        candidate['previous'] = m.installed_state('public')
        def make_plan(*args): return copy.deepcopy(candidate)
        output = StringIO()
        with patch.object(m, 'console_owner', return_value=501), patch.object(m,'plan',side_effect=make_plan), \
             patch.object(m, 'locks', return_value=nullcontext()), patch.object(m.os,'geteuid',return_value=0), \
             patch('sys.argv',['local_install.py','plan','--edition','public']), redirect_stdout(output):
            m.main()
        approved = json.loads(output.getvalue())['digest']
        with self.managed_root_metadata(), patch.object(m,'console_owner',return_value=501), \
             patch.object(m,'plan',side_effect=make_plan), patch.object(m,'locks',return_value=nullcontext()), \
             patch.object(m.os,'geteuid',return_value=0), patch.object(m,'finalize',return_value={'status':'test_pending'}), \
             patch('sys.argv',['local_install.py','replace','--edition','public','--digest',approved]), redirect_stdout(StringIO()):
            m.main()
        self.assertEqual(json.loads(m.JOURNAL.read_text())['plan']['operation'],'replace')
        self.assertEqual(json.loads(m.RECORD.read_text())['edition'],'public')

    def test_system_original_is_preserved_without_parsing(self):
        original=m.SYSTEM/'system-original.json'; original.write_text('{}')
        self.execute()
        self.assertEqual(original.read_text(), '{}')

    def test_pending_blocks_new_generation(self):
        m.BARRIER.write_text('active')
        with self.assertRaisesRegex(ValueError,'pending'): self.execute()

    def test_symlink_and_hardlink_artifacts_rejected(self):
        link=self.root/'link'; link.symlink_to(self.source)
        with self.assertRaises(ValueError): m.tree(link)
        link.unlink(); os.link(self.source,link)
        with self.assertRaises(ValueError): m.tree(link)

    def test_copy_rejects_link_without_reading_target(self):
        link=self.root/'link'; link.symlink_to(self.source)
        with self.assertRaises(OSError): m.copy(link,self.root/'output')
        self.assertFalse((self.root/'output').exists())

    def test_copy_rejects_symlink_ancestor(self):
        link=self.root/'link'; link.symlink_to(self.templates)
        with self.assertRaises(OSError): m.copy(link/'daemon',self.root/'output')

    def test_templates_changed_after_preview(self):
        (self.templates/'daemon').write_text('changed')
        with self.assertRaisesRegex(ValueError,'template'): self.execute()
        self.assertFalse(m.RECORD.exists())


class OwnerCredentialsTests(unittest.TestCase):
    def credentials(self, fail=None):
        stack = __import__('contextlib').ExitStack()
        self.addCleanup(stack.close)
        state = dict(uid=0, gid=0, groups=[0, 12])
        calls = []
        def change(name, value):
            calls.append((name, value))
            if name == fail and not any(c[0] == 'failed' for c in calls):
                calls.append(('failed', name))
                raise OSError('injected ' + name)
            state[name] = value
        stack.enter_context(patch.object(m.os, 'geteuid', side_effect=lambda: state['uid']))
        stack.enter_context(patch.object(m.os, 'getegid', side_effect=lambda: state['gid']))
        # This is the oversized directory membership view; production must not use it.
        directory_groups = stack.enter_context(patch.object(m.os, 'getgroups', return_value=list(range(64))))
        stack.enter_context(patch.object(m, 'process_groups', return_value=[0, 12]))
        stack.enter_context(patch.object(m.pwd, 'getpwuid', return_value=type('Account', (), dict(pw_name='owner', pw_gid=20))()))
        stack.enter_context(patch.object(m.os, 'initgroups', side_effect=lambda name, gid: change('groups', [20, 12])))
        stack.enter_context(patch.object(m.os, 'setgroups', side_effect=lambda groups: change('groups', groups)))
        stack.enter_context(patch.object(m.os, 'setegid', side_effect=lambda gid: change('gid', gid)))
        stack.enter_context(patch.object(m.os, 'seteuid', side_effect=lambda uid: change('uid', uid)))
        return state, calls, directory_groups

    def test_owner_restores_kernel_groups_not_oversized_directory_groups(self):
        state, calls, directory_groups = self.credentials()
        with m.as_owner(501):
            self.assertEqual(state, dict(uid=501, gid=20, groups=[20, 12]))
        self.assertEqual(state, dict(uid=0, gid=0, groups=[0, 12]))
        directory_groups.assert_not_called()

    def test_owner_restores_after_body_exception(self):
        state, calls, _ = self.credentials()
        with self.assertRaisesRegex(RuntimeError, 'body'):
            with m.as_owner(501):
                raise RuntimeError('body')
        self.assertEqual(state, dict(uid=0, gid=0, groups=[0, 12]))

    def test_owner_restores_after_each_drop_failure(self):
        for operation in ('groups', 'gid', 'uid'):
            with self.subTest(operation=operation):
                state, calls, _ = self.credentials(operation)
                with self.assertRaisesRegex(OSError, 'injected'):
                    with m.as_owner(501):
                        self.fail('must not yield after credential failure')
                self.assertEqual(state, dict(uid=0, gid=0, groups=[0, 12]))
                self.doCleanups()

    def test_owner_snapshot_failure_does_not_mutate_credentials(self):
        state, calls, _ = self.credentials()
        with patch.object(m, 'process_groups', side_effect=OSError('snapshot')):
            with self.assertRaisesRegex(OSError, 'snapshot'):
                with m.as_owner(501):
                    self.fail('must not yield without snapshot')
        self.assertEqual(calls, [])

    def test_owner_restore_failure_is_not_suppressed(self):
        state, calls, _ = self.credentials()
        with patch.object(m.os, 'setgroups', side_effect=OSError('restore')):
            with self.assertRaisesRegex(OSError, 'restore'):
                with m.as_owner(501):
                    pass

    def test_owner_group_restore_attempted_when_gid_restore_fails(self):
        state, calls, _ = self.credentials()
        def setegid(gid):
            if gid == 0:
                raise OSError('restore gid')
            state['gid'] = gid
        with patch.object(m.os, 'setegid', side_effect=setegid):
            with self.assertRaisesRegex(OSError, 'restore gid'):
                with m.as_owner(501):
                    pass
        self.assertEqual(state['uid'], 0)
        self.assertEqual(state['groups'], [0, 12])

    def test_kernel_groups_uses_raw_libc_symbol_and_preserves_order(self):
        from unittest.mock import Mock
        def getgroups(count, target):
            if count == 0:
                return 3
            for index, group in enumerate([12, 0, 7]):
                target[index] = group
            return 3
        libc = Mock()
        libc.getgroups.side_effect = getgroups
        with patch.object(m.ctypes, 'CDLL', return_value=libc), patch.object(m.os, 'sysconf', return_value=16):
            self.assertEqual(m.process_groups(), [12, 0, 7])
        self.assertEqual(libc.getgroups.call_count, 2)

    def test_kernel_groups_over_limit_rejected_without_truncation(self):
        from unittest.mock import Mock
        libc = Mock()
        libc.getgroups.return_value = 17
        with patch.object(m.ctypes, 'CDLL', return_value=libc), patch.object(m.os, 'sysconf', return_value=16):
            with self.assertRaisesRegex(ValueError, 'limit'):
                m.process_groups()
        self.assertEqual(libc.getgroups.call_count, 1)




class F02UninstallTests(unittest.TestCase):
    """Real installer transactions against files and a fail-closed fake launchd."""
    owner = 501

    def setUp(self):
        import contextlib
        from types import SimpleNamespace
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.home = self.root/'owner-home'; self.home.mkdir()
        self.system = self.root/'system'; self.system.mkdir()
        self.apps = self.root/'Applications'; self.apps.mkdir()
        self.templates = self.root/'templates'; self.templates.mkdir()
        self.fd_paths = {}; self.metadata_overrides = {}
        self.commands = []; self.loaded = {}; self.disabled = {}
        self.real_stat, self.real_lstat, self.real_fstat = os.stat, os.lstat, os.fstat
        self.real_open = os.open
        self.SimpleNamespace = SimpleNamespace
        names = dict(SYSTEM=self.system, HELPER=self.root/'helpers/helper',
                     GUARDIAN=self.system/'switcher-guardian',
                     DAEMON=self.root/'daemons/helper.plist', AGENT=self.root/'agents/guardian.plist',
                     RECORD=self.system/'install.json', JOURNAL=self.system/'journal.json',
                     BARRIER=self.system/'barrier', CHALLENGE=self.system/'challenge.json',
                     RECEIPT=self.system/'receipt.json', LAST_RESULT=self.system/'last-result.json',
                     WATCHDOG_PLIST=self.root/'daemons/watchdog.plist',
                     BODYGUARD_PLIST=self.root/'daemons/bodyguard.plist',
                     TEMPLATES=self.templates, ZONEINFO=self.root/'zoneinfo')
        for directory in ['helpers','daemons','agents','zoneinfo/Asia']:
            (self.root/directory).mkdir(parents=True, exist_ok=True)
        (self.root/'zoneinfo/Asia/Tokyo').write_text('synthetic zone')
        for key,value in names.items(): self.patch(m,key,value)
        self.patch(m,'app',side_effect=lambda edition:self.apps/(edition+'.app'))
        self.patch(m.pwd,'getpwuid',side_effect=lambda uid:SimpleNamespace(
            pw_dir=str(self.home if uid==self.owner else self.root/('owner-'+str(uid))),
            pw_name='fixture', pw_gid=20))
        self.patch(m,'as_owner',side_effect=lambda uid:contextlib.nullcontext())
        self.patch(m.os,'getgrouplist',return_value=[20])
        self.patch(m,'safe_parents',side_effect=self.safe_parents)
        self.patch(m,'identity',return_value='a'*40)
        self.patch(m,'stopped',return_value=None)
        self.patch(m,'run',side_effect=self.fake_run)
        self.patch(m.os,'open',side_effect=self.open)
        self.patch(m.os,'stat',side_effect=self.stat)
        self.patch(m.os,'lstat',side_effect=self.lstat)
        self.patch(m.os,'fstat',side_effect=self.fstat)
        self.patch(m.os,'chown',side_effect=self.chown)
        self.patch(m.os,'fchown',side_effect=self.fchown)
        self.patch(m,'legacy_specs',side_effect=self.legacy_specs)
        for label,path,program in [('local.claudeswitcher.helper',m.DAEMON,m.HELPER),
                                   ('local.claudeswitcher.guardian',m.AGENT,m.GUARDIAN)]:
            (self.templates/path.name).write_bytes(m.plistlib.dumps(dict(
                Label=label, ProgramArguments=[str(program)], RunAtLoad=True)))
        self.source = self.root/'Fresh.app'
        components=self.source/'Contents/Resources/components'; components.mkdir(parents=True)
        for name in ['switcher-helper','switcher-guardian','claude-switcher']:
            path=components/name; path.write_text('synthetic '+name); path.chmod(0o755)
        (self.source/'Contents/Info.plist').write_bytes(m.plistlib.dumps(
            dict(CFBundleShortVersionString='0.2.0')))
        config=self.home/'.claude-switcher/v2/config.json'; config.parent.mkdir(parents=True)
        config.write_text(json.dumps(dict(country_target='JP',timezone_target='Asia/Tokyo',
                                         locale_target='ja_JP',proxy_mode='auto')))

    def patch(self, target, name, *values, **kwargs):
        patcher=patch.object(target,name,*values,**kwargs)
        value=patcher.start(); self.addCleanup(patcher.stop); return value

    def resolve(self,path,dir_fd=None):
        if isinstance(path,int):
            if path not in self.fd_paths: raise AssertionError('untracked synthetic descriptor')
            return self.fd_paths[path]
        path=Path(path)
        if not path.is_absolute():
            path=(self.fd_paths[dir_fd] if dir_fd is not None else Path.cwd())/path
        return path.absolute()

    def meta(self, value, path):
        fields={name:getattr(value,name) for name in dir(value) if name.startswith('st_')}
        uid=self.owner if path==self.home or self.home in path.parents else 0
        fields.update(st_uid=uid,st_gid=20 if uid else 0)
        fields.update(self.metadata_overrides.get((value.st_dev,value.st_ino),{}))
        return self.SimpleNamespace(**fields)

    def open(self,path,*args,**kwargs):
        fd=self.real_open(path,*args,**kwargs)
        self.fd_paths[fd]=self.resolve(path,kwargs.get('dir_fd'))
        return fd

    def stat(self,path,*args,**kwargs):
        return self.meta(self.real_stat(path,*args,**kwargs),self.resolve(path,kwargs.get('dir_fd')))

    def lstat(self,path,*args,**kwargs):
        return self.meta(self.real_lstat(path,*args,**kwargs),self.resolve(path,kwargs.get('dir_fd')))

    def fstat(self,fd):
        return self.meta(self.real_fstat(fd),self.resolve(fd))

    def chown(self,path,uid,gid,**kwargs):
        path=self.resolve(path,kwargs.get('dir_fd'))
        self.assertIn(self.root,path.parents)
        metadata=self.real_lstat(path)
        self.metadata_overrides[(metadata.st_dev,metadata.st_ino)]=dict(st_uid=uid,st_gid=gid)

    def fchown(self,fd,uid,gid):
        self.chown(self.resolve(fd),uid,gid)

    def safe_parents(self,path):
        path=Path(path)
        self.assertTrue(path==self.root or self.root in path.parents, str(path))
        while path!=self.root:
            metadata=path.lstat()
            if not m.stat.S_ISDIR(metadata.st_mode) or metadata.st_uid or metadata.st_mode & 0o022:
                raise ValueError('unsafe synthetic root directory')
            path=path.parent

    def legacy_specs(self,owner):
        return [dict(label=label,domain=domain,path=str(path),uid=uid,interval=interval,
                     run=run,arguments=arguments) for label,domain,path,uid,interval,run,arguments in [
            ('local.claude-bodyguard','system',m.BODYGUARD_PLIST,0,10,True,['/bin/bash',str(self.system/'claude-bodyguard.sh')]),
            ('local.tz-watchdog','system',m.WATCHDOG_PLIST,0,3600,True,['/bin/bash',str(self.system/'tz-watchdog.sh')]),
            ('local.claude-update','gui/'+str(owner),self.home/'Library/LaunchAgents/local.claude-update.plist',owner,43200,False,
             ['/bin/bash',str(m.cli_path(owner)),'update','--auto'])]]

    def result(self,code=0,stdout='',stderr='',check=True):
        if check and code: raise m.subprocess.CalledProcessError(code,self.commands[-1],stdout,stderr)
        return self.SimpleNamespace(returncode=code,stdout=stdout,stderr=stderr)

    def fake_run(self,*args,check=True,**kwargs):
        args=tuple(map(str,args)); self.commands.append(args)
        if args==('/usr/sbin/sysctl','-n','kern.bootsessionuuid'):
            return self.result(stdout='11111111-1111-1111-1111-111111111111',check=check)
        if args[:3]==('/bin/ps','-ww','-p'):
            for target,item in self.loaded.items():
                if str(item['pid'])==args[3]:
                    uid=self.owner if target.startswith('gui/') else 0
                    return self.result(stdout=str(uid)+' '+item['arguments'][0],check=check)
        if args[0]!='/bin/launchctl': raise AssertionError('unexpected OS command: '+repr(args))
        action=args[1]
        if action=='print-disabled':
            domain=args[2]
            lines=['"'+target.rsplit('/',1)[1]+'" => '+str(value).lower()
                   for target,value in self.disabled.items() if target.rsplit('/',1)[0]==domain]
            return self.result(stdout='\n'.join(lines),check=check)
        if action=='print':
            target=args[2]
            if target not in self.loaded:
                return self.result(113,stderr='Could not find service '+target,check=check)
            item=self.loaded[target]
            return self.result(stdout='\n'.join(['path = '+item['path'],
                'program = '+item['arguments'][0], 'arguments = {',
                *('    '+argument for argument in item['arguments']), '}', 'pid = '+str(item['pid'])]),check=check)
        if action=='bootstrap':
            domain,path=args[2:4]; data=m.plistlib.loads(Path(path).read_bytes())
            target=domain+'/'+data['Label']
            self.loaded[target]=dict(path=path,arguments=data['ProgramArguments'],pid=100+len(self.loaded))
            return self.result(check=check)
        if action=='bootout':
            target=args[2]
            if target not in self.loaded:
                return self.result(113,stderr='Could not find service '+target,check=check)
            del self.loaded[target]; return self.result(check=check)
        if action=='kickstart' and args[2] in self.loaded: return self.result(check=check)
        raise AssertionError('unsupported launchctl command: '+repr(args))

    def footprint(self,edition):
        return [m.app(edition),m.HELPER,m.GUARDIAN,m.DAEMON,m.AGENT,m.RECORD,
                m.cli_path(self.owner),self.system/'tz-watchdog.sh',m.WATCHDOG_PLIST,
                self.system/'claude-bodyguard.sh',m.BODYGUARD_PLIST]

    def install(self,edition):
        planned=m.plan(edition,self.source,self.owner)
        m.transact(planned,m.digest(planned))
        self.assertEqual(m.finalize(self.owner)['status'],'committed')
        self.assertEqual(len(self.loaded),4)
        return planned

    def test_f02_real_plan_install_uninstall_removes_all_files_and_services(self):
        for edition in ['personal','public']:
            with self.subTest(edition=edition):
                self.install(edition)
                planned=m.plan(edition,None,self.owner,'uninstall')
                m.transact(planned,m.digest(planned))
                self.assertEqual(m.finalize(self.owner)['status'],'uninstalled')
                self.assertEqual([str(path) for path in self.footprint(edition) if m.present(path)],[])
                self.assertEqual(self.loaded,{})
                self.assertEqual(m.finalize(self.owner)['status'],'uninstalled')
                self.assertEqual(m.verify(self.owner)['status'],'uninstalled')


    def uninstall_plan(self,edition='personal'):
        return m.plan(edition,None,self.owner,'uninstall')

    def transact(self,planned):
        return m.transact(planned,m.digest(planned))

    def snapshot(self,paths,identities=False):
        result={}
        for path in paths:
            nodes=[path]
            if path.is_dir() and not path.is_symlink(): nodes+=sorted(path.rglob('*'))
            for node in nodes:
                if not m.present(node): result[str(node)]=None; continue
                meta=node.lstat()
                content=('link',str(node.readlink())) if node.is_symlink() else (
                    ('directory',) if node.is_dir() else ('file',node.read_bytes()))
                result[str(node)]=(content,m.stat.S_IMODE(meta.st_mode),meta.st_uid,meta.st_gid,
                                   *((meta.st_dev,meta.st_ino) if identities else ()))
        return result

    def service_state(self):
        return {key:{name:value for name,value in item.items() if name!='pid'}
                for key,item in self.loaded.items()}

    def state(self,identities=False):
        return self.snapshot(self.footprint('personal'),identities),self.service_state(),dict(self.disabled)

    def assert_no_mutations(self,commands):
        self.assertFalse([command for command in commands if command[0]=='/bin/launchctl'
                          and command[1] not in ['print','print-disabled']])

    def rollback_twice(self,before):
        m.recover(self.owner); m.recover(self.owner)
        self.assertEqual(self.state(),before)
        self.assertFalse(m.JOURNAL.exists()); self.assertFalse(m.BARRIER.exists())
        self.assertFalse(any(command[1] in ['enable','disable'] for command in self.commands
                             if command[0]=='/bin/launchctl'))

    def stage(self):
        return Path(m.load_journal()['stage'])

    def test_f02_uninstall_preserves_account_config_repair_and_historical_bytes(self):
        self.install('personal')
        sentinels=[self.home/'.claude-switcher/accounts/A/session',
                   self.home/'.claude-switcher/accounts/B/session',
                   self.home/'Library/Application Support/Claude/session',
                   self.home/'.claude-switcher/v2/config.json',
                   self.home/'.claude-switcher/v2/account-journal.json',
                   self.home/'.claude-switcher/v2/region-repair.json',
                   self.home/'.claude-switcher/v2/user-repairs/journal.json',
                   self.system/'system-original.json',self.system/'repair-result.json',
                   self.system/'historical-backup/secret',self.root/'owner-502/accounts/A/session']
        for path in sentinels:
            path.parent.mkdir(parents=True,exist_ok=True)
            if not path.exists(): path.write_bytes(('retain '+str(path.relative_to(self.root))).encode())
        historical=[path for path in self.system.glob('source-local-*')]
        before=self.snapshot(sentinels+historical,identities=True)
        cli=m.cli_path(self.owner).read_bytes()
        planned=self.uninstall_plan(); self.transact(planned)
        backup=self.stage()/'legacy-cli.sh'
        self.assertEqual(backup.read_bytes(),cli)
        self.assertEqual(m.finalize(self.owner)['status'],'uninstalled')
        self.assertEqual(self.snapshot(sentinels+historical,identities=True),before)
        self.assertEqual(backup.read_bytes(),cli)

    def test_f02_plan_refuses_wrong_owner_edition_and_generation(self):
        self.install('personal')
        before=self.state(identities=True)
        for owner,edition in [(502,'personal'),(501,'public')]:
            with self.subTest(owner=owner,edition=edition):
                start=len(self.commands)
                with self.assertRaises(ValueError): m.plan(edition,None,owner,'uninstall')
                self.assertEqual(self.state(identities=True),before)
                self.assert_no_mutations(self.commands[start:])
        original=m.RECORD.read_bytes()
        record=json.loads(original); record['trust']['generation']='f'*32
        m.save(m.RECORD,record,mode=0o644)
        changed=self.state(identities=True)
        with self.assertRaises((ValueError,OSError)): self.uninstall_plan()
        self.assertEqual(self.state(identities=True),changed)
        m.RECORD.write_bytes(original)

    def test_f02_ownership_uses_current_generation_instead_of_last_result(self):
        self.install('personal')
        m.save(m.LAST_RESULT,dict(owner_uid=502,status='committed',stage=str(self.root/'unrelated')))
        planned=self.uninstall_plan()
        self.transact(planned)
        self.assertEqual(m.finalize(self.owner)['status'],'uninstalled')

    def test_f02_plan_refuses_unknown_component_bytes_without_deletion(self):
        self.install('personal')
        for target in self.footprint('personal'):
            path=next(node for node in target.rglob('*') if node.is_file()) if target.is_dir() else target
            original=path.read_bytes()
            with self.subTest(path=str(target.relative_to(self.root))):
                path.write_bytes(b'unknown replacement')
                before=self.state(identities=True); start=len(self.commands)
                with self.assertRaises((ValueError,OSError)): self.uninstall_plan()
                self.assertEqual(self.state(identities=True),before)
                self.assert_no_mutations(self.commands[start:])
                path.write_bytes(original)

    def test_f02_plan_refuses_unsafe_auxiliary_owner_mode_and_link_type(self):
        self.install('personal')
        for path in self.footprint('personal')[6:]:
            original=path.read_bytes(); mode=m.stat.S_IMODE(path.stat().st_mode)
            for kind in ['owner','group','mode','symlink','hardlink','directory']:
                if kind=='group' and path==m.cli_path(self.owner): continue
                with self.subTest(path=path.name,kind=kind):
                    key=(path.stat().st_dev,path.stat().st_ino)
                    other=self.root/('retained-'+path.name)
                    if kind in ['owner','group']:
                        self.metadata_overrides[key]={'st_uid':777} if kind=='owner' else {'st_gid':777}
                    elif kind=='mode': path.chmod(0o777)
                    elif kind in ['symlink','hardlink','directory']:
                        path.rename(other)
                        if kind=='symlink': path.symlink_to(other)
                        elif kind=='hardlink': os.link(other,path)
                        else: path.mkdir()
                    before=self.state(identities=True); start=len(self.commands)
                    with self.assertRaises((ValueError,OSError)): self.uninstall_plan()
                    self.assertEqual(self.state(identities=True),before)
                    self.assert_no_mutations(self.commands[start:])
                    if kind in ['owner','group']: self.metadata_overrides.pop(key)
                    elif kind=='mode': path.chmod(mode)
                    else:
                        path.rmdir() if kind=='directory' else path.unlink()
                        other.rename(path)
                    self.assertEqual(path.read_bytes(),original)

    def test_f02_plan_refuses_unproven_loaded_program_plist_and_query_errors(self):
        self.install('personal')
        import copy
        for target in list(self.loaded):
            for kind in ['program','plist','query']:
                with self.subTest(target=target,kind=kind):
                    old=copy.deepcopy(self.loaded[target]); start=len(self.commands)
                    if kind=='program': self.loaded[target]['arguments']=['/bin/false']
                    if kind=='plist': self.loaded[target]['path']=str(self.root/'unowned.plist')
                    before=self.state(identities=True)
                    def query(*args,**kwargs):
                        if tuple(map(str,args))==('/bin/launchctl','print',target):
                            self.commands.append(tuple(map(str,args)))
                            return self.result(5,stderr='Access denied',check=kwargs.get('check',True))
                        return self.fake_run(*args,**kwargs)
                    with patch.object(m,'run',side_effect=query if kind=='query' else self.fake_run):
                        with self.assertRaises(ValueError): self.uninstall_plan()
                    self.assertEqual(self.state(identities=True),before)
                    self.assert_no_mutations(self.commands[start:]); self.loaded[target]=old

    def test_f02_execution_rejects_same_byte_replacements_after_preview(self):
        self.install('personal')
        for path in self.footprint('personal'):
            with self.subTest(path=str(path.relative_to(self.root))):
                planned=self.uninstall_plan()
                parked=self.root/('parked-'+path.name)
                path.rename(parked); m.copy(parked,path)
                before=self.state(identities=True); start=len(self.commands)
                with self.assertRaises(ValueError): self.transact(planned)
                self.assertEqual(self.state(identities=True),before)
                self.assert_no_mutations(self.commands[start:])
                m.remove(path); parked.rename(path)

    def test_f02_execution_rejects_loaded_identity_and_destination_drift(self):
        self.install('personal')
        import copy
        for target in list(self.loaded):
            planned=self.uninstall_plan(); old=copy.deepcopy(self.loaded[target])
            self.loaded[target]['arguments']=['/bin/false']
            before=self.state(identities=True); start=len(self.commands)
            with self.assertRaises(ValueError): self.transact(planned)
            self.assertEqual(self.state(identities=True),before)
            self.assert_no_mutations(self.commands[start:]); self.loaded[target]=old
        planned=self.uninstall_plan(); external=self.root/'unowned'; external.write_text('preserve')
        planned['destinations'][1]=str(external)
        before=self.state(identities=True)
        with self.assertRaises(ValueError): self.transact(planned)
        self.assertEqual(external.read_text(),'preserve'); self.assertEqual(self.state(identities=True),before)

    def test_f02_missing_auxiliary_files_and_mixed_registration_are_recorded(self):
        self.install('personal')
        self.loaded.pop('system/local.tz-watchdog')
        m.WATCHDOG_PLIST.unlink(); (self.system/'tz-watchdog.sh').unlink()
        self.disabled={'system/local.tz-watchdog':True,
                       'system/local.claude-bodyguard':False}
        before=dict(self.disabled)
        planned=self.uninstall_plan(); self.transact(planned)
        self.assertEqual(m.finalize(self.owner)['status'],'uninstalled')
        self.assertEqual(self.disabled,before); self.assertEqual(self.loaded,{})

    def test_f02_rollback_restores_exact_mixed_registration_and_disabled_overrides(self):
        self.install('personal')
        self.loaded.pop('system/local.tz-watchdog')
        self.disabled={'system/local.tz-watchdog':True,'system/local.claude-bodyguard':False}
        before=self.state()
        self.transact(self.uninstall_plan())
        self.rollback_twice(before)

    def test_f02_each_file_backup_interruption_leaves_installation_untouched(self):
        self.install('personal'); before=self.state(); original=m.copy
        for target in self.footprint('personal'):
            if target==m.cli_path(self.owner): continue
            for after in [False,True]:
                with self.subTest(path=target.name,after=after):
                    planned=self.uninstall_plan(); fired=[]
                    def fail(source,dest):
                        match=Path(source)==target and Path(dest).parent.parent==self.system
                        if match and not fired:
                            fired.append(True)
                            if after: original(source,dest)
                            raise RuntimeError('F02 backup interruption')
                        return original(source,dest)
                    with patch.object(m,'copy',side_effect=fail):
                        with self.assertRaisesRegex(RuntimeError,'F02 backup'): self.transact(planned)
                    self.assertTrue(fired); self.rollback_twice(before)

    def test_f02_cli_backup_interruption_never_removes_unbacked_files(self):
        self.install('personal'); before=self.state(); original=m.cli_backup
        for after in [False,True]:
            planned=self.uninstall_plan()
            def fail(j):
                if after: original(j)
                raise RuntimeError('F02 CLI backup interruption')
            with patch.object(m,'cli_backup',side_effect=fail):
                with self.assertRaisesRegex(RuntimeError,'F02 CLI'): self.transact(planned)
            self.rollback_twice(before)

    def removal_fault(self,path,after):
        from contextlib import ExitStack
        stack=ExitStack(); fired=[]
        original_unlink=os.unlink; original_rename=m.rename_exclusive
        def unlink(name,*args,**kwargs):
            resolved=self.resolve(name,kwargs.get('dir_fd'))
            if resolved==path and not fired:
                fired.append(True)
                if after: original_unlink(name,*args,**kwargs)
                raise RuntimeError('F02 removal interruption')
            return original_unlink(name,*args,**kwargs)
        def rename(sourcefd,source,destfd,dest):
            if self.resolve(source,sourcefd)==path and not fired:
                fired.append(True)
                if after: original_rename(sourcefd,source,destfd,dest)
                raise RuntimeError('F02 removal interruption')
            return original_rename(sourcefd,source,destfd,dest)
        stack.enter_context(patch.object(m.os,'unlink',side_effect=unlink))
        stack.enter_context(patch.object(m,'rename_exclusive',side_effect=rename))
        return stack,fired

    def test_f02_each_physical_removal_boundary_recovers_twice(self):
        self.install('personal'); before=self.state()
        for target in self.footprint('personal'):
            for after in [False,True]:
                with self.subTest(path=target.name,after=after):
                    planned=self.uninstall_plan(); stack,fired=self.removal_fault(target,after)
                    with stack:
                        with self.assertRaisesRegex(RuntimeError,'F02 removal'): self.transact(planned)
                    self.assertTrue(fired); self.rollback_twice(before)

    def test_f02_each_service_bootout_boundary_recovers_exact_registration(self):
        self.install('personal'); before=self.state()
        for target in list(self.loaded):
            for after in [False,True]:
                with self.subTest(target=target,after=after):
                    planned=self.uninstall_plan(); fired=[]
                    def fail(*args,**kwargs):
                        if tuple(map(str,args))==('/bin/launchctl','bootout',target) and not fired:
                            fired.append(True)
                            if after: self.fake_run(*args,**kwargs)
                            raise RuntimeError('F02 bootout interruption')
                        return self.fake_run(*args,**kwargs)
                    with patch.object(m,'run',side_effect=fail):
                        with self.assertRaisesRegex(RuntimeError,'F02 bootout'): self.transact(planned)
                    self.assertTrue(fired); self.rollback_twice(before)

    def test_f02_each_journal_progress_save_boundary_recovers_twice(self):
        self.install('personal'); before=self.state(); original=m.save; calls=[]
        def count(path,value,*args,**kwargs):
            if path==m.JOURNAL: calls.append(True)
            return original(path,value,*args,**kwargs)
        with patch.object(m,'save',side_effect=count): self.transact(self.uninstall_plan())
        self.rollback_twice(before)
        total=len(calls); self.assertGreater(total,10)
        for index in range(total):
            for after in [False,True]:
                with self.subTest(index=index,after=after):
                    planned=self.uninstall_plan(); seen=[]
                    def fail(path,value,*args,**kwargs):
                        if path==m.JOURNAL:
                            current=len(seen); seen.append(True)
                            if current==index:
                                if after: original(path,value,*args,**kwargs)
                                raise RuntimeError('F02 journal interruption')
                        return original(path,value,*args,**kwargs)
                    with patch.object(m,'save',side_effect=fail):
                        with self.assertRaisesRegex(RuntimeError,'F02 journal'): self.transact(planned)
                    self.rollback_twice(before)
        print('F02 progress-save physical boundaries:',total*2)

    def test_f02_corrupt_completed_backups_pause_recovery_without_overwrite(self):
        self.install('personal'); before=self.state(); original=m.save
        backup_names=['old-app','old-helper','old-guardian','old-daemon','old-agent','old-record',
                      'legacy-cli.sh','watchdog-old-script','watchdog-old-plist',
                      'bodyguard-old-script','bodyguard-old-plist']
        for name in backup_names:
            with self.subTest(backup=name):
                planned=self.uninstall_plan(); corrupt=[]
                def fail(path,value,*args,**kwargs):
                    result=original(path,value,*args,**kwargs)
                    if path==m.JOURNAL and value.get('pending')=='services' and not corrupt:
                        backup=Path(value['stage'])/name
                        target=next(node for node in backup.rglob('*') if node.is_file()) if backup.is_dir() else backup
                        data=target.read_bytes(); target.write_bytes(b'corrupt backup'); corrupt.append((target,data))
                        raise RuntimeError('F02 completed backup corrupted')
                    return result
                with patch.object(m,'save',side_effect=fail):
                    with self.assertRaisesRegex(RuntimeError,'F02 completed'): self.transact(planned)
                self.assertTrue(corrupt)
                for _ in range(2):
                    with self.assertRaises(ValueError): m.recover(self.owner)
                    self.assertEqual(self.snapshot(self.footprint('personal')),before[0]); self.assertTrue(m.JOURNAL.exists())
                corrupt[0][0].write_bytes(corrupt[0][1]); self.rollback_twice(before)

    def test_f02_external_replacements_after_removal_pause_recovery(self):
        self.install('personal'); before=self.state()
        for path in self.footprint('personal'):
            with self.subTest(path=path.name):
                planned=self.uninstall_plan(); stack,fired=self.removal_fault(path,True)
                with stack:
                    with self.assertRaisesRegex(RuntimeError,'F02 removal'): self.transact(planned)
                self.assertTrue(fired)
                path.write_bytes(b'external replacement'); path.chmod(0o755)
                replaced=self.snapshot([path],identities=True)
                for _ in range(2):
                    with self.assertRaises((ValueError,OSError)): m.recover(self.owner)
                    self.assertEqual(self.snapshot([path],identities=True),replaced)
                    self.assertTrue(m.JOURNAL.exists())
                path.unlink(); self.rollback_twice(before)

    def test_f02_terminal_result_and_cleanup_boundaries_are_idempotent(self):
        original_save=m.save
        for point in ['result','last-result','barrier','journal']:
            for after in [False,True]:
                with self.subTest(point=point,after=after):
                    self.install('personal'); self.transact(self.uninstall_plan()); fired=[]
                    def save(path,value,*args,**kwargs):
                        match=(point=='result' and path.name=='result.json') or (point=='last-result' and path==m.LAST_RESULT)
                        if match and not fired:
                            fired.append(True)
                            if after: original_save(path,value,*args,**kwargs)
                            raise RuntimeError('F02 terminal interruption')
                        return original_save(path,value,*args,**kwargs)
                    target=m.BARRIER if point=='barrier' else m.JOURNAL
                    stack,unlinked=self.removal_fault(target,after)
                    with patch.object(m,'save',side_effect=save), stack:
                        with self.assertRaisesRegex(RuntimeError,'F02 (terminal|removal)'): m.finalize(self.owner)
                    self.assertTrue(fired or unlinked)
                    self.assertEqual(m.finalize(self.owner)['status'],'uninstalled')
                    self.assertEqual(m.finalize(self.owner)['status'],'uninstalled')
                    self.assertEqual(m.verify(self.owner)['status'],'uninstalled')
                    self.assertFalse(m.JOURNAL.exists()); self.assertFalse(m.BARRIER.exists())

    def test_f02_finalization_and_archived_verify_reject_reappearing_footprint(self):
        self.install('personal'); import copy
        services=copy.deepcopy(self.loaded)
        self.transact(self.uninstall_plan())
        for path in self.footprint('personal'):
            with self.subTest(path=path.name):
                path.symlink_to(self.root/'missing-target')
                with self.assertRaises(ValueError): m.finalize(self.owner)
                self.assertTrue(path.is_symlink()); self.assertTrue(m.JOURNAL.exists()); path.unlink()
        for target,item in services.items():
            self.loaded[target]=item
            with self.assertRaises(ValueError): m.finalize(self.owner)
            self.assertTrue(m.JOURNAL.exists()); self.loaded.pop(target)
        self.assertEqual(m.finalize(self.owner)['status'],'uninstalled')
        for path in self.footprint('personal'):
            path.write_text('reappeared')
            with self.assertRaises(ValueError): m.verify(self.owner)
            self.assertEqual(path.read_text(),'reappeared'); path.unlink()
        for target,item in services.items():
            self.loaded[target]=item
            with self.assertRaises(ValueError): m.verify(self.owner)
            self.loaded.pop(target)

    def test_f02_terminal_cleanup_retry_rechecks_reappearing_files_and_services(self):
        self.install('personal'); import copy
        services=copy.deepcopy(self.loaded)
        self.transact(self.uninstall_plan())
        stack,fired=self.removal_fault(m.BARRIER,True)
        with stack:
            with self.assertRaisesRegex(RuntimeError,'F02 removal'): m.finalize(self.owner)
        self.assertTrue(fired); self.assertTrue(m.JOURNAL.exists())
        path=m.BODYGUARD_PLIST; path.symlink_to(self.root/'missing')
        with self.assertRaises(ValueError): m.finalize(self.owner)
        self.assertTrue(m.JOURNAL.exists()); path.unlink()
        target='system/local.tz-watchdog'; self.loaded[target]=services[target]
        with self.assertRaises(ValueError): m.finalize(self.owner)
        self.assertTrue(m.JOURNAL.exists()); self.loaded.pop(target)
        self.assertEqual(m.finalize(self.owner)['status'],'uninstalled')

    def test_f02_old_narrow_uninstall_receipt_cannot_hide_auxiliary_objects(self):
        self.install('personal'); self.transact(self.uninstall_plan())
        result=m.finalize(self.owner)
        archive=Path(result['backup'])/'result.json'; value=json.loads(archive.read_text())
        old_keys={'schema_version','operation','edition','owner_uid','version','payload',
                  'templates','destinations','previous','legacy','timezone_target','proxy_mode','cli_entry'}
        value['plan']={key:item for key,item in value['plan'].items() if key in old_keys}
        value['plan']['cli_entry']=None; value['plan']['timezone_target']=None; value['plan']['proxy_mode']=None
        m.save(archive,value)
        path=m.cli_path(self.owner); path.write_text('unowned object')
        with self.assertRaises(ValueError): m.verify(self.owner)
        self.assertEqual(path.read_text(),'unowned object')


    def test_f02_missing_install_record_never_guesses_removal_authority(self):
        self.install('personal'); m.RECORD.unlink()
        before=self.state(identities=True); start=len(self.commands)
        with self.assertRaises(ValueError): self.uninstall_plan()
        self.assertEqual(self.state(identities=True),before)
        self.assert_no_mutations(self.commands[start:])

    def test_f02_same_byte_external_replacements_after_removal_pause_recovery(self):
        self.install('personal'); before=self.state()
        names=['old-app','old-helper','old-guardian','old-daemon','old-agent','old-record',
               'legacy-cli.sh','watchdog-old-script','watchdog-old-plist',
               'bodyguard-old-script','bodyguard-old-plist']
        for path,name in zip(self.footprint('personal'),names):
            with self.subTest(path=path.name):
                planned=self.uninstall_plan(); mode=m.stat.S_IMODE(path.stat().st_mode)
                stack,fired=self.removal_fault(path,True)
                with stack:
                    with self.assertRaisesRegex(RuntimeError,'F02 removal'): self.transact(planned)
                self.assertTrue(fired)
                m.copy(self.stage()/name,path); path.chmod(mode)
                replaced=self.snapshot([path],identities=True)
                for _ in range(2):
                    with self.assertRaises(ValueError): m.recover(self.owner)
                    self.assertEqual(self.snapshot([path],identities=True),replaced)
                    self.assertTrue(m.JOURNAL.exists())
                m.remove(path); self.rollback_twice(before)


    def assert_restored_pin_rejects_external_change(self,kind):
        self.install('personal'); before=self.state(); planned=self.uninstall_plan()
        self.transact(planned); original=m.save; durable=[]
        def fail(path,value,*args,**kwargs):
            if path==m.JOURNAL and durable:
                raise RuntimeError('F02 later recovery step interrupted')
            result=original(path,value,*args,**kwargs)
            if path==m.JOURNAL and 'helper' in value.get('uninstall_restored',{}): durable.append(True)
            return result
        with patch.object(m,'save',side_effect=fail):
            with self.assertRaisesRegex(RuntimeError,'F02 later recovery'): m.recover(self.owner)
        self.assertTrue(durable)
        path=m.HELPER; parked=self.root/'durable-restored-helper'
        key=(path.stat().st_dev,path.stat().st_ino)
        if kind=='inode':
            path.rename(parked); m.copy(parked,path)
        else:
            self.metadata_overrides[key]={'st_gid':777}
        replaced=self.snapshot([path],identities=True)
        with patch.object(m.os,'chown',side_effect=self.chown) as chown, \
             patch.object(m.os,'fchown',side_effect=self.fchown) as fchown:
            for _ in range(2):
                with self.assertRaises(ValueError): m.recover(self.owner)
                self.assertEqual(self.snapshot([path],identities=True),replaced)
                self.assertTrue(m.JOURNAL.exists())
            chown.assert_not_called(); fchown.assert_not_called()
        if kind=='inode': m.remove(path); parked.rename(path)
        else: self.metadata_overrides.pop(key)
        self.rollback_twice(before)

    def test_f02_review_durable_restored_inode_replacement_stops_without_chown(self):
        self.assert_restored_pin_rejects_external_change('inode')

    def test_f02_review_durable_restored_gid_change_stops_without_chown(self):
        self.assert_restored_pin_rejects_external_change('gid')

    def test_f02_review_corruption_after_backup_stops_before_first_side_effect(self):
        self.install('personal'); before=self.state(identities=True); original=m.save
        names=['old-app','old-helper','old-guardian','old-daemon','old-agent','old-record',
               'legacy-cli.sh','watchdog-old-script','watchdog-old-plist',
               'bodyguard-old-script','bodyguard-old-plist']
        for name in names:
            with self.subTest(backup=name):
                planned=self.uninstall_plan(); corrupted=[]; start=len(self.commands)
                def corrupt(path,value,*args,**kwargs):
                    result=original(path,value,*args,**kwargs)
                    if path==m.JOURNAL and value.get('pending')=='services' and not corrupted:
                        backup=Path(value['stage'])/name
                        target=next(node for node in backup.rglob('*') if node.is_file()) if backup.is_dir() else backup
                        data=target.read_bytes(); target.write_bytes(b'post-backup corruption')
                        corrupted.append((target,data))
                    return result
                try:
                    with patch.object(m,'save',side_effect=corrupt):
                        with self.assertRaises(ValueError): self.transact(planned)
                    self.assertTrue(corrupted)
                    self.assertEqual(self.state(identities=True),before)
                    self.assert_no_mutations(self.commands[start:])
                finally:
                    if corrupted: corrupted[0][0].write_bytes(corrupted[0][1])
                    if m.JOURNAL.exists(): m.recover(self.owner); m.recover(self.owner)
                before=self.state(identities=True)

    def test_f02_review_missing_unregistered_root_script_can_uninstall(self):
        for label,script in [('local.tz-watchdog','tz-watchdog.sh'),
                             ('local.claude-bodyguard','claude-bodyguard.sh')]:
            with self.subTest(label=label):
                self.install('personal'); self.loaded.pop('system/'+label)
                (self.system/script).unlink()
                self.transact(self.uninstall_plan())
                self.assertEqual(m.finalize(self.owner)['status'],'uninstalled')
                self.assertEqual(m.verify(self.owner)['status'],'uninstalled')
                self.assertEqual(self.loaded,{})
                self.assertFalse(any(m.present(path) for path in self.footprint('personal')))

    def test_f02_review_registered_services_with_missing_required_files_refuse_preview(self):
        self.install('personal')
        for path in [m.HELPER,m.DAEMON,m.GUARDIAN,m.AGENT,self.system/'tz-watchdog.sh',
                     m.WATCHDOG_PLIST,self.system/'claude-bodyguard.sh',m.BODYGUARD_PLIST]:
            with self.subTest(path=path.name):
                parked=self.root/('missing-'+path.name); path.rename(parked)
                before=self.state(identities=True); start=len(self.commands)
                try:
                    with self.assertRaises(ValueError): self.uninstall_plan()
                    self.assertEqual(self.state(identities=True),before)
                    self.assert_no_mutations(self.commands[start:])
                finally: parked.rename(path)

    def test_f02_review_partial_root_restore_copy_can_resume(self):
        self.install('personal'); before=self.state(); original=m.copy
        for name in ['watchdog-old-script','watchdog-old-plist','bodyguard-old-script','bodyguard-old-plist']:
            with self.subTest(backup=name):
                self.transact(self.uninstall_plan()); stage=self.stage(); backup=stage/name; fired=[]
                live={'watchdog-old-script':self.system/'tz-watchdog.sh',
                      'watchdog-old-plist':m.WATCHDOG_PLIST,
                      'bodyguard-old-script':self.system/'claude-bodyguard.sh',
                      'bodyguard-old-plist':m.BODYGUARD_PLIST}[name]
                def partial(source,target):
                    if Path(source)==backup and not fired:
                        self.assertEqual(Path(target).parent,stage)
                        self.assertTrue(Path(target).name.startswith('restore-'))
                        self.assertFalse(m.present(live))
                        fired.append(True); Path(target).write_bytes(b'partial restore')
                        Path(target).chmod(0o600)
                        raise RuntimeError('F02 partial restore copy')
                    return original(source,target)
                with patch.object(m,'copy',side_effect=partial):
                    with self.assertRaisesRegex(RuntimeError,'F02 partial restore'): m.recover(self.owner)
                self.assertTrue(fired); self.assertTrue(m.JOURNAL.exists())
                self.assertFalse(m.present(live))
                self.rollback_twice(before)

    def interrupted_after_durable_restore(self,key):
        self.install('personal'); before=self.state(); self.transact(self.uninstall_plan())
        original=m.save; durable=[]
        def fail(path,value,*args,**kwargs):
            if path==m.JOURNAL and durable:
                raise RuntimeError('F02 durable restore boundary')
            result=original(path,value,*args,**kwargs)
            if path==m.JOURNAL and key in value.get('uninstall_restored',{}): durable.append(True)
            return result
        with patch.object(m,'save',side_effect=fail):
            with self.assertRaisesRegex(RuntimeError,'F02 durable restore'): m.recover(self.owner)
        self.assertTrue(durable)
        return before

    def test_f02_review_recovery_check_to_open_replacement_is_rejected(self):
        for key in ['helper','cli']:
            with self.subTest(component=key):
                before=self.interrupted_after_durable_restore(key)
                path=m.HELPER if key=='helper' else m.cli_path(self.owner)
                parked=self.root/('checked-'+key); changed=[]; chowned=[]
                original=m.check_uninstall_recovery
                def swap(j,only=None):
                    result=original(j,only)
                    if only==key and not changed:
                        path.rename(parked); m.copy(parked,path)
                        changed.append(self.snapshot([path],identities=True))
                    return result
                def chown(fd,uid,gid):
                    if self.resolve(fd)==path: chowned.append(True)
                    return self.fchown(fd,uid,gid)
                try:
                    with patch.object(m,'check_uninstall_recovery',side_effect=swap), \
                         patch.object(m.os,'fchown',side_effect=chown):
                        for _ in range(2):
                            with self.assertRaises(ValueError): m.recover(self.owner)
                            self.assertEqual(self.snapshot([path],identities=True),changed[0])
                    self.assertFalse(chowned)
                finally:
                    if parked.exists(): m.remove(path); parked.rename(path)
                    if m.JOURNAL.exists(): self.rollback_twice(before)

    def test_f02_review_fchown_to_final_snapshot_replacement_is_rejected(self):
        for key in ['helper','cli']:
            with self.subTest(component=key):
                before=self.interrupted_after_durable_restore(key)
                path=m.HELPER if key=='helper' else m.cli_path(self.owner)
                parked=self.root/('chowned-'+key); changed=[]
                def swap(fd,uid,gid):
                    self.fchown(fd,uid,gid)
                    if self.resolve(fd)==path and not changed:
                        path.rename(parked); m.copy(parked,path)
                        changed.append(self.snapshot([path],identities=True))
                try:
                    with patch.object(m.os,'fchown',side_effect=swap):
                        for _ in range(2):
                            with self.assertRaises(ValueError): m.recover(self.owner)
                            self.assertEqual(self.snapshot([path],identities=True),changed[0])
                finally:
                    if parked.exists(): m.remove(path); parked.rename(path)
                    if m.JOURNAL.exists(): self.rollback_twice(before)

    def test_f02_review_durable_restored_external_deletion_is_rejected(self):
        for key in ['helper','cli']:
            with self.subTest(component=key):
                before=self.interrupted_after_durable_restore(key)
                path=m.HELPER if key=='helper' else m.cli_path(self.owner)
                parked=self.root/('deleted-'+key); path.rename(parked)
                try:
                    for _ in range(2):
                        with self.assertRaises(ValueError): m.recover(self.owner)
                        self.assertFalse(m.present(path)); self.assertTrue(m.JOURNAL.exists())
                finally:
                    if m.present(path): m.remove(path)
                    parked.rename(path)
                    if m.JOURNAL.exists(): self.rollback_twice(before)

    def test_f02_review_two_megabyte_binaries_install_and_uninstall(self):
        components=self.source/'Contents/Resources/components'
        for name in ['switcher-helper','switcher-guardian']:
            (components/name).write_bytes((name.encode()+b'\0')*(2*1024*1024//(len(name)+1)+1))
        self.install('personal')
        self.assertGreater(m.HELPER.stat().st_size,2*1024*1024)
        self.assertGreater(m.GUARDIAN.stat().st_size,2*1024*1024)
        self.transact(self.uninstall_plan())
        self.assertEqual(m.finalize(self.owner)['status'],'uninstalled')
        self.assertEqual(m.verify(self.owner)['status'],'uninstalled')
        self.assertFalse(any(m.present(path) for path in self.footprint('personal')))
        self.assertEqual(self.loaded,{})

    def reset_isolated_case(self):
        # Each injected conflict retains its journal; the next case gets a new HOME.
        self.doCleanups(); self.setUp()

    def assert_retained_identity(self,expected):
        dev,ino,data,mode,uid,gid=expected
        found=[]
        for node in self.root.rglob('*'):
            metadata=node.lstat()
            if (metadata.st_dev,metadata.st_ino)==(dev,ino): found.append(node)
        self.assertEqual(len(found),1,'unexpected object must survive at live or holding path')
        node=found[0]; metadata=node.lstat()
        self.assertEqual(m.tree(node),data)
        self.assertEqual((m.stat.S_IMODE(metadata.st_mode),metadata.st_uid,metadata.st_gid),(mode,uid,gid))
        return node

    def identity_evidence(self,path):
        metadata=path.lstat()
        return (metadata.st_dev,metadata.st_ino,m.tree(path),m.stat.S_IMODE(metadata.st_mode),
                metadata.st_uid,metadata.st_gid)

    def test_f02_iter2_replacement_at_physical_removal_survives(self):
        for key in ['cli','helper','app']:
            for different in [False,True]:
                with self.subTest(component=key,different_bytes=different):
                    try:
                        self.install('personal'); planned=self.uninstall_plan()
                        path={'cli':m.cli_path(self.owner),'helper':m.HELPER,'app':m.app('personal')}[key]
                        parked=self.root/'approved-original'; injected=[]
                        unlink=m.os.unlink; rename=m.rename_exclusive
                        def replace():
                            path.rename(parked); m.copy(parked,path)
                            if different:
                                changed=next(node for node in path.rglob('*') if node.is_file()) if path.is_dir() else path
                                changed.write_bytes(b'F02 external user content')
                            injected.append(self.identity_evidence(path))
                        def delete(name,*args,**kwargs):
                            if self.resolve(name,kwargs.get('dir_fd'))==path and not injected: replace()
                            return unlink(name,*args,**kwargs)
                        def move(src_fd,src,dst_fd,dst):
                            if self.resolve(src,src_fd)==path and not injected: replace()
                            return rename(src_fd,src,dst_fd,dst)
                        with patch.object(m.os,'unlink',side_effect=delete),patch.object(m,'rename_exclusive',side_effect=move):
                            with self.assertRaises(ValueError): self.transact(planned)
                        self.assertTrue(injected); self.assert_retained_identity(injected[0])
                        for _ in range(2):
                            with self.assertRaises(ValueError): m.recover(self.owner)
                            self.assert_retained_identity(injected[0]); self.assertTrue(m.JOURNAL.exists())
                    finally: self.reset_isolated_case()

    def interrupt_restore_intent(self,key):
        original=m.save; fired=[]
        def fail(path,value,*args,**kwargs):
            result=original(path,value,*args,**kwargs)
            if (path==m.JOURNAL and key in value.get('uninstall_restoring',[]) and
                    key not in value.get('uninstall_restored',{}) and not fired):
                fired.append(True); raise RuntimeError('F02 intent before publication')
            return result
        with patch.object(m,'save',side_effect=fail):
            with self.assertRaisesRegex(RuntimeError,'F02 intent'): m.recover(self.owner)
        self.assertTrue(fired)

    def test_f02_iter2_intent_only_external_candidate_is_not_adopted(self):
        for key in ['cli','helper']:
            for wrong_gid in [False,True]:
                with self.subTest(component=key,wrong_gid=wrong_gid):
                    try:
                        self.install('personal'); self.transact(self.uninstall_plan())
                        path=m.cli_path(self.owner) if key=='cli' else m.HELPER
                        backup=self.stage()/('legacy-cli.sh' if key=='cli' else 'old-helper')
                        self.interrupt_restore_intent(key)
                        m.copy(backup,path); path.chmod(0o755)
                        if wrong_gid:
                            metadata=path.lstat(); self.metadata_overrides[(metadata.st_dev,metadata.st_ino)]={'st_gid':777}
                        external=self.identity_evidence(path); journal=m.JOURNAL.read_bytes()
                        with patch.object(m.os,'chown',side_effect=self.chown) as chown, \
                             patch.object(m.os,'fchown',side_effect=self.fchown) as fchown:
                            for _ in range(2):
                                with self.assertRaises(ValueError): m.recover(self.owner)
                                self.assert_retained_identity(external)
                                self.assertEqual(m.JOURNAL.read_bytes(),journal)
                            chown.assert_not_called(); fchown.assert_not_called()
                    finally: self.reset_isolated_case()

    def interrupt_restore_publication(self,path):
        original=m.rename_exclusive; fired=[]
        def fail(src_fd,src,dst_fd,dst):
            result=original(src_fd,src,dst_fd,dst)
            if self.resolve(dst,dst_fd)==path and not fired:
                fired.append(True); raise RuntimeError('F02 published before final identity save')
            return result
        with patch.object(m,'rename_exclusive',side_effect=fail):
            with self.assertRaisesRegex(RuntimeError,'F02 published'): m.recover(self.owner)
        self.assertTrue(fired); self.assertTrue(m.present(path))

    def test_f02_iter2_genuine_publication_without_final_save_recovers_twice(self):
        for index in range(11):
            with self.subTest(component=index):
                try:
                    self.install('personal'); before=self.state(); path=self.footprint('personal')[index]
                    self.transact(self.uninstall_plan()); self.interrupt_restore_publication(path)
                    self.rollback_twice(before)
                finally: self.reset_isolated_case()

    def test_f02_iter2_published_candidate_external_change_is_not_adopted(self):
        for index in range(11):
            for change in ['inode','gid']:
                with self.subTest(component=index,change=change):
                    try:
                        self.install('personal'); path=self.footprint('personal')[index]
                        self.transact(self.uninstall_plan()); self.interrupt_restore_publication(path)
                        if change=='inode':
                            parked=self.root/'published-original'; path.rename(parked); m.copy(parked,path)
                        else:
                            metadata=path.lstat(); self.metadata_overrides[(metadata.st_dev,metadata.st_ino)]={'st_gid':777}
                        external=self.identity_evidence(path); journal=m.JOURNAL.read_bytes()
                        with patch.object(m.os,'chown',side_effect=self.chown) as chown, \
                             patch.object(m.os,'fchown',side_effect=self.fchown) as fchown:
                            for _ in range(2):
                                with self.assertRaises(ValueError): m.recover(self.owner)
                                self.assert_retained_identity(external)
                                self.assertEqual(m.JOURNAL.read_bytes(),journal)
                            chown.assert_not_called(); fchown.assert_not_called()
                    finally: self.reset_isolated_case()

    def interrupt_terminal_restore(self):
        original=m.save; fired=[]
        def fail(path,value,*args,**kwargs):
            result=original(path,value,*args,**kwargs)
            if path==m.JOURNAL and value.get('pending')=='restored' and not fired:
                fired.append(True); raise RuntimeError('F02 restored terminal save before cleanup')
            return result
        with patch.object(m,'save',side_effect=fail):
            with self.assertRaisesRegex(RuntimeError,'F02 restored terminal'): m.recover(self.owner)
        self.assertTrue(fired); self.assertTrue(m.JOURNAL.exists())

    def test_f02_iter2_terminal_restore_rechecks_external_file_change(self):
        for key in ['cli','helper','watchdog']:
            for change in ['delete','inode','bytes']:
                with self.subTest(component=key,change=change):
                    try:
                        self.install('personal'); self.transact(self.uninstall_plan()); self.interrupt_terminal_restore()
                        path={'cli':m.cli_path(self.owner),'helper':m.HELPER,'watchdog':self.system/'tz-watchdog.sh'}[key]
                        if change=='delete': path.unlink()
                        elif change=='inode':
                            parked=self.root/'terminal-original'; path.rename(parked); m.copy(parked,path)
                        else: path.write_bytes(b'F02 external terminal content')
                        before=self.state(identities=True); journal=m.JOURNAL.read_bytes()
                        for _ in range(2):
                            with self.assertRaises(ValueError): m.recover(self.owner)
                            self.assertEqual(self.state(identities=True),before)
                            self.assertEqual(m.JOURNAL.read_bytes(),journal)
                    finally: self.reset_isolated_case()

    def test_f02_iter2_terminal_restore_rechecks_service_registration(self):
        for index in range(4):
            with self.subTest(service=index):
                try:
                    self.install('personal'); self.transact(self.uninstall_plan()); self.interrupt_terminal_restore()
                    self.loaded.pop(list(self.loaded)[index]); before=self.state(identities=True)
                    journal=m.JOURNAL.read_bytes()
                    for _ in range(2):
                        with self.assertRaises(ValueError): m.recover(self.owner)
                        self.assertEqual(self.state(identities=True),before)
                        self.assertEqual(m.JOURNAL.read_bytes(),journal)
                finally: self.reset_isolated_case()

    def test_f02_iter2_terminal_restore_unchanged_finishes_twice(self):
        self.install('personal'); before=self.state(); self.transact(self.uninstall_plan())
        self.interrupt_terminal_restore(); self.rollback_twice(before)

if __name__=='__main__': unittest.main()
