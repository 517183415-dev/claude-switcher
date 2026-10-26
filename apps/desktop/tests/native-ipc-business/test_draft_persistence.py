"""Behavioral and filesystem negatives for the five-process draft oracle."""
import copy
import json
import os
from pathlib import Path
import tempfile
import subprocess
import sys
import runner
import unittest
import draft
import draft_persistence as dp


class DraftRestartTests(unittest.TestCase):
    def test_cli_records_missing_build_without_launching_fixture(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary).resolve()
            result = subprocess.run([
                sys.executable, '-B', str(runner.HERE / 'runner.py'),
                'draft-persistence', '--output', str(output), '--build-attempt', 'r999',
            ], capture_output=True, text=True, timeout=20)
            self.assertEqual(result.returncode, 1, result.stderr)
            receipts = list(output.glob('run-*-draft-persistence-*/result.json'))
            self.assertEqual(len(receipts), 1, result.stderr)
            receipt = json.loads(receipts[0].read_text())
            self.assertEqual(receipt['status'], 'BLOCKED_OR_FAILED')
            self.assertEqual(receipt['fixture_app_launch'], 'NOT_RUN')
            self.assertEqual(receipt['phases'], [])

    def test_runner_requires_attempt_for_draft_restart(self):
        with tempfile.TemporaryDirectory()as out:
            with self.assertRaises(SystemExit):runner.parse_args(['draft-persistence','--output',str(Path(out).resolve())])

    def test_runner_rejects_caller_selected_edition(self):
        with tempfile.TemporaryDirectory()as out:
            with self.assertRaises(SystemExit):runner.parse_args(['draft-persistence','--output',str(Path(out).resolve()),'--build-attempt','r1','--edition','personal'])

    def test_runner_accepts_explicit_restart_attempt(self):
        with tempfile.TemporaryDirectory()as out:
            self.assertEqual(runner.parse_args(['draft-persistence','--output',str(Path(out).resolve()),'--build-attempt','r1']).action,'draft-persistence')

    def rejected_js_phase(self,edition,phase):
        script="import {runDraftPersistence} from './draft-persistence.mjs';let calls=0;try{await runDraftPersistence(async()=>{calls++;},"+json.dumps(edition)+","+json.dumps(phase)+",{});process.exit(2);}catch(error){if(calls!==0)process.exit(3);}"
        result=subprocess.run(['/opt/homebrew/bin/node','--input-type=module','-e',script],cwd=Path(__file__).parent,capture_output=True,text=True,timeout=20)
        self.assertEqual(result.returncode,0,result.stderr)

    def test_unknown_js_phase_rejects_before_business_commands(self):self.rejected_js_phase('personal','unknown')
    def test_public_js_save_rejects_before_business_commands(self):self.rejected_js_phase('public','save')
    def test_personal_js_probe_rejects_before_business_commands(self):self.rejected_js_phase('personal','probe')

    def saved(self):
        initial=draft.initial_example();saved=draft.filled(initial)
        saved.update(content_revision=2,release_id='1'*32,issued_at=1001,expires_at=1001+7776000)
        return initial,saved

    def disk(self,value):
        generation='a'*32
        state=dict(draft=value,assets=[],history=[],pending=None,recovery_required=False,last_resolution=None,cleanup=None)
        base='v2/personal/publisher/'
        return dict(directories=['v2','v2/personal',base[:-1],base+generation,base+generation+'/assets'],files={
            base+'active.json':json.dumps(dict(schema=1,generation=generation)).encode(),
            base+'lifecycle.lock':b'',base+generation+'/state.json':json.dumps(state).encode()})

    def test_accepts_production_omitted_empty_cleanup_field(self):
        _, saved = self.saved()
        snapshot = self.disk(saved)
        state_path = next(name for name in snapshot['files'] if name.endswith('/state.json'))
        state = json.loads(snapshot['files'][state_path])
        del state['cleanup']
        snapshot['files'][state_path] = json.dumps(state).encode()
        self.assertEqual(dp.disk_value(snapshot), saved)

    def test_rejects_nonempty_cleanup_state(self):
        _, saved = self.saved()
        snapshot = self.disk(saved)
        state_path = next(name for name in snapshot['files'] if name.endswith('/state.json'))
        state = json.loads(snapshot['files'][state_path])
        state['cleanup'] = {'schema': 1}
        snapshot['files'][state_path] = json.dumps(state).encode()
        with self.assertRaises(AssertionError):
            dp.disk_value(snapshot)

    def test_accepts_saved_six_campaigns_matching_independent_disk(self):
        initial,saved=self.saved()
        result=dp.validate_phase('save',dict(phase='save',initial=initial,saved=saved),dp.empty(),self.disk(saved),None)
        self.assertEqual(result,saved)

    def test_accepts_new_reader_without_changing_any_bytes(self):
        _,saved=self.saved();files=self.disk(saved)
        self.assertEqual(dp.validate_phase('read',dict(phase='read',value=saved),files,files,saved),saved)

    def test_rejects_read_process_changing_file_bytes(self):
        _,saved=self.saved();before=self.disk(saved);after=copy.deepcopy(before)
        after['files']['v2/personal/publisher/lifecycle.lock']=b'changed'
        with self.assertRaises(AssertionError):dp.validate_phase('read',dict(phase='read',value=saved),before,after,saved)

    def test_accepts_public_permission_denials_without_writes(self):
        _,saved=self.saved();files=self.disk(saved)
        response=dict(phase='probe',denials=[dict(command=c,kind='permission')for c in draft.COMMANDS])
        self.assertEqual(dp.validate_phase('probe',response,files,files,saved),saved)

    def test_rejects_public_missing_handler_as_permission_proof(self):
        _,saved=self.saved();files=self.disk(saved)
        response=dict(phase='probe',denials=[dict(command=c,kind='missing')for c in draft.COMMANDS])
        with self.assertRaises(AssertionError):dp.validate_phase('probe',response,files,files,saved)

    def test_rejects_public_creating_an_edition_directory(self):
        _,saved=self.saved();before=self.disk(saved);after=copy.deepcopy(before);after['directories'].append('v2/public')
        response=dict(phase='probe',denials=[dict(command=c,kind='permission')for c in draft.COMMANDS])
        with self.assertRaises(AssertionError):dp.validate_phase('probe',response,before,after,saved)

    def paused(self):
        _,saved=self.saved();single=copy.deepcopy(saved);single['slots'][0]['enabled']=False
        single.update(content_revision=3,release_id='2'*32)
        final=copy.deepcopy(single)
        for slot in final['slots']:slot.update(enabled=False,campaigns=[])
        final.update(content_revision=4,release_id='3'*32)
        return saved,dict(phase='pause',before=saved,single=single,single_read=single,all=final,all_read=final)

    def test_accepts_single_then_all_pause_and_disk_commit(self):
        saved,response=self.paused()
        self.assertEqual(dp.validate_phase('pause',response,self.disk(saved),self.disk(response['all']),saved),response['all'])

    def test_rejects_single_pause_disabling_another_slot(self):
        saved,response=self.paused();response['single']['slots'][1]['enabled']=False
        with self.assertRaises(AssertionError):dp.validate_phase('pause',response,self.disk(saved),self.disk(response['all']),saved)

    def test_rejects_all_pause_retaining_campaigns(self):
        saved,response=self.paused();response['all']['slots'][0]['campaigns']=saved['slots'][0]['campaigns']
        with self.assertRaises(AssertionError):dp.validate_phase('pause',response,self.disk(saved),self.disk(response['all']),saved)

    def test_rejects_disk_draft_differing_from_ipc(self):
        _,saved=self.saved();bad=copy.deepcopy(saved);bad['slots'][0]['enabled']=False
        with self.assertRaises(AssertionError):dp.validate_phase('read',dict(phase='read',value=saved),self.disk(bad),self.disk(bad),saved)

    def test_rejects_wrong_phase_receipt(self):
        _,saved=self.saved();files=self.disk(saved)
        with self.assertRaises(AssertionError):dp.validate_phase('verify',dict(phase='read',value=saved),files,files,saved)

    def test_rejects_revision_not_incremented_after_save(self):
        initial,saved=self.saved();saved['content_revision']=initial['content_revision']
        with self.assertRaises(AssertionError):dp.validate_phase('save',dict(phase='save',initial=initial,saved=saved),dp.empty(),self.disk(saved),None)

    def test_rejects_reused_process_id(self):
        with self.assertRaises(AssertionError):dp.validate_execution(dict(pid=42,exit=0,timeout=False),{42})

    def test_rejects_process_timeout(self):
        with self.assertRaises(AssertionError):dp.validate_execution(dict(pid=42,exit=0,timeout=True),set())

    def test_rejects_nonzero_process_exit(self):
        with self.assertRaises(AssertionError):dp.validate_execution(dict(pid=42,exit=1,timeout=False),set())

    def test_accepts_new_successful_process_id(self):
        seen=set();dp.validate_execution(dict(pid=42,exit=0,timeout=False),seen);self.assertEqual(seen,{42})

    def root(self):
        temp=tempfile.TemporaryDirectory();self.addCleanup(temp.cleanup)
        root=Path(temp.name);root.chmod(0o700)
        _,saved=self.saved();snapshot=self.disk(saved)
        for name in snapshot['directories']:(root/name).mkdir(mode=0o700)
        for name,data in snapshot['files'].items():(root/name).write_bytes(data);(root/name).chmod(0o600)
        return root,saved

    def test_accepts_private_disk_tree(self):
        root,saved=self.root();self.assertEqual(dp.disk_value(dp.files(root)),saved)

    def test_rejects_nonprivate_disk_file(self):
        root,_=self.root();(root/'v2/personal/publisher/active.json').chmod(0o644)
        with self.assertRaises(AssertionError):dp.files(root)

    def test_rejects_symlink_before_reading_target(self):
        root,_=self.root();path=root/'v2/personal/publisher/active.json';path.unlink();path.symlink_to(root/'never-read')
        with self.assertRaises(AssertionError):dp.files(root)

    def test_rejects_unexpected_asset_file(self):
        root,_=self.root();path=next(root.glob('v2/personal/publisher/*/assets'))/'unexpected';path.write_bytes(b'only synthetic');path.chmod(0o600)
        with self.assertRaises(AssertionError):dp.disk_value(dp.files(root))


if __name__=='__main__':unittest.main()
