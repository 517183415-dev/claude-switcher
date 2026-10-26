"""Five native processes sharing only one newly created synthetic draft root."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
import stat
import draft
import runner
from support import inputs, output_root

PHASES=(('personal','save'),('personal','read'),('public','probe'),('personal','pause'),('personal','verify'))


def empty():
    return dict(directories=[],files={})


def files(root):
    """Read bounded regular files only after owner/mode/link checks at each layer."""
    snapshot=empty()
    def visit(directory,depth):
        assert depth<=6 and len(snapshot['directories'])<=16,'draft directory bound'
        runner.private(directory,0o700,True)
        for path in sorted(directory.iterdir()):
            name=str(path.relative_to(root))
            if stat.S_ISDIR(path.lstat().st_mode):
                snapshot['directories'].append(name);visit(path,depth+1)
            else:
                runner.private(path,0o600)
                assert len(snapshot['files'])<32,'draft file count bound'
                with path.open('rb')as handle:data=handle.read(65537)
                assert len(data)<=65536,'draft file byte bound'
                snapshot['files'][name]=data
    visit(root,0)
    return snapshot


def disk_value(snapshot):
    base='v2/personal/publisher/'
    active=json.loads(snapshot['files'][base+'active.json'])
    assert set(active)=={'schema','generation'} and type(active['schema'])is int and active['schema']==1
    generation=active['generation'];assert isinstance(generation,str)and re.fullmatch('[0-9a-f]{32}',generation)
    assert set(snapshot['directories'])=={'v2','v2/personal',base[:-1],base+generation,base+generation+'/assets'},'unexpected edition/asset directory'
    assert len(snapshot['directories'])==5
    assert set(snapshot['files'])=={base+'active.json',base+'lifecycle.lock',base+generation+'/state.json'},'unexpected persistent file or asset'
    assert snapshot['files'][base+'lifecycle.lock']==b'','lock contents changed'
    state=json.loads(snapshot['files'][base+generation+'/state.json'])
    assert state['assets']==state['history']==[] and state['pending']is None,'unexpected publication/asset state'
    assert state['recovery_required']is False and state.get('cleanup')is None and state['last_resolution']is None
    return state['draft']


def same(left,right):
    assert json.dumps(left,sort_keys=True)==json.dumps(right,sort_keys=True),'draft restart value mismatch'


def transition(previous,value,command,request):
    same(draft.semantic(value),draft.semantic(draft.expected_value(previous,command,request)))
    assert type(value['content_revision'])is int and value['content_revision']==previous['content_revision']+1,'revision did not advance once'
    assert re.fullmatch('[0-9a-f]{32}',value['release_id'])and value['release_id']!=previous['release_id'],'release did not change'
    assert type(value['issued_at'])is int and value['issued_at']>=previous['issued_at']
    assert value['expires_at']-value['issued_at']==90*24*60*60


def validate_phase(phase,response,before,after,previous):
    assert isinstance(response,dict)and response.get('phase')==phase,'wrong draft phase'
    if phase=='save':
        assert set(response)=={'phase','initial','saved'} and before==empty()and previous is None,'save must start empty'
        initial=response['initial'];same(draft.semantic(initial),draft.semantic(draft.initial_example()))
        assert initial['content_revision']==1
        value=response['saved'];transition(initial,value,'ad_draft_save',{'manifest':draft.filled(initial)})
    elif phase in ('read','verify'):
        assert set(response)=={'phase','value'}and before==after,'reader changed persistent tree'
        same(response['value'],previous);value=previous
    elif phase=='probe':
        assert set(response)=={'phase','denials'}and before==after,'Public modified persistent tree'
        draft.validate_public(response['denials']);value=previous
    else:
        assert phase=='pause'and set(response)=={'phase','before','single','single_read','all','all_read'}
        same(response['before'],previous)
        transition(previous,response['single'],'pause_ad',{'slot':draft.SLOTS[0]})
        same(response['single_read'],response['single'])
        transition(response['single'],response['all'],'pause_ad',{'slot':None})
        same(response['all_read'],response['all']);value=response['all']
    if before!=empty():same(disk_value(before),previous)
    same(disk_value(after),value)
    return value


def validate_execution(execution,pids):
    assert execution['exit']==0 and execution['timeout']is False,'draft fixture failed or timed out'
    pid=execution['pid'];assert type(pid)is int and pid>0 and pid not in pids,'process reused'
    pids.add(pid)


def run(attempt, output=None):
    if output is not None:
        runner.OUT=output_root(str(output))
    runner.attempt_value(attempt)
    folder=runner.new_run('draft-persistence');root=folder/'tmp/business-root'
    evidence=dict(status='RUNNING',fixture_app_launch='NOT_RUN',phases=[],root=str(root))
    try:
        bindings={edition:runner.build_binding(edition,attempt)for edition in ('personal','public')}
        current=inputs();assert all(binding[3]==current for binding in bindings.values())
        profile_hash=runner.sha(folder/'runtime.sb')
        evidence.update(source_hashes=current,profile_sha256=profile_hash,bindings={edition:dict(receipt_sha256=runner.sha(binding[0]),binary_sha256=runner.sha(binding[2]))for edition,binding in bindings.items()})
        pids=set();previous=None
        for edition,phase in PHASES:
            assert runner.prepare(),'isolation controls failed'
            assert inputs()==current and runner.sha(folder/'runtime.sb')==profile_hash,'input/profile changed before phase'
            binary=bindings[edition][2];assert runner.sha(binary)==bindings[edition][1]['signed_sha256'],'binary changed'
            before=files(root);label=edition+'-'+phase
            execution=runner.execute([binary,'--draft-persistence-'+phase],folder,label=label)
            evidence['fixture_app_launch']='RUN'
            phase_record=dict(edition=edition,phase=phase,execution=execution)
            evidence['phases'].append(phase_record)
            validate_execution(execution,pids)
            after=files(root)
            envelope=runner.envelope(runner.bounded_text(folder/(label+'-stdout.log')),execution['exit'],edition,False)
            assert set(envelope['responses'])=={'draft_persistence'},'unexpected draft response branch'
            value=validate_phase(phase,envelope['responses']['draft_persistence'],before,after,previous)
            phase_record.update(validation=dict(status='PASS',denials=envelope['denials'],disk_equals_ipc=True,read_only_tree_unchanged=phase in ('read','probe','verify')),before_file_sha256={name:hashlib.sha256(data).hexdigest()for name,data in before['files'].items()},file_sha256={name:hashlib.sha256(data).hexdigest()for name,data in after['files'].items()})
            previous=value
            assert inputs()==current and runner.sha(binary)==bindings[edition][1]['signed_sha256']and runner.sha(folder/'runtime.sb')==profile_hash,'input changed during phase'
        assert len(pids)==len(evidence['phases'])==5
        evidence.update(status='PASS',ordinary_app='NOT_RUN',keychain='NOT_RUN',object_store='NOT_RUN',independent_disk='before and after each process; private exact tree, full bytes and draft values',single_pause_intermediate='IPC response and immediate real readback; final all-pause independently checked on disk')
    except (OSError,ValueError,KeyError,TypeError,AssertionError)as error:
        evidence.update(status='BLOCKED_OR_FAILED',error=str(error))
    runner.write(folder/'result.json',evidence);print(folder)
    return evidence['status']=='PASS'


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',required=True,type=output_root)
    parser.add_argument('--build-attempt',required=True,type=runner.attempt_value)
    args=parser.parse_args();runner.OUT=args.output
    sys.exit(0 if run(args.build_attempt)else 1)
