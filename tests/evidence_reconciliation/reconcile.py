"""Fail-closed libtest reconciliation with explicitly declared nested harnesses."""
from collections import Counter
from pathlib import Path
import hashlib
import json
import re

PARENT = 'accounts::legacy::tests::f06_missing_switch_boundaries_recover_in_fresh_process'
CHILD = 'accounts::legacy::tests::f06_fresh_recovery_child_fixture'
RUN = re.compile(r'Running (.+ \(.*\))')
START = re.compile(r'running (\d+) tests?')
RESULT = re.compile(r'test (\S+) \.\.\. (ok|FAILED|ignored)')
SUMMARY = re.compile(r'test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out; finished in \d+(?:\.\d+)?s')


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def parse_tests(text):
    harnesses = []
    active = None
    pending = None
    child = None
    pids = set()
    nested = 0
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        context = f'line {number}: {line}'
        if not line:
            continue
        header = RUN.fullmatch(line)
        if header:
            require(active is None and pending is None and child is None, context)
            active = {'key': header[1], 'count': None, 'names': []}
            continue
        if active is None:
            require(not line.startswith(('running ', 'test ', 'ok', 'FAILED')), context)
            continue  # Cargo diagnostics outside a test harness.
        start = START.fullmatch(line)
        if start:
            if active['count'] is None:
                active['count'] = int(start[1])
            else:
                require(pending == PARENT and child is None and nested < 3 and start[1] == '1', context)
                child = {'names': [], 'pid': None}
            continue
        require(active['count'] is not None, context)
        if child is not None:
            proof = re.fullmatch(r'F06 fresh recovery pid=(\d+) recoveries=2 metadata/stages/journal=ok', line)
            if proof:
                require(child['pid'] is None and proof[1] not in pids, context)
                child['pid'] = proof[1]
                pids.add(proof[1])
                continue
            result = RESULT.fullmatch(line)
            if result:
                require(result[1] == CHILD and result[2] == 'ok' and not child['names'], context)
                child['names'].append(result[1])
                continue
            summary = SUMMARY.fullmatch(line)
            require(summary is not None and summary[1] == 'ok' and tuple(map(int, summary.group(2, 3, 4, 5))) == (1, 0, 0, 0), context)
            require(child['names'] == [CHILD] and child['pid'] is not None, context)
            child = None
            nested += 1
            continue
        if pending:
            require(line == 'ok' and nested == 3, context)
            active['names'].append(pending)
            pending = None
            continue
        result = RESULT.fullmatch(line)
        if result:
            require(result[2] == 'ok', context)
            active['names'].append(result[1])
            continue
        if line == f'test {PARENT} ...':
            require(nested == 0 and PARENT not in active['names'], context)
            pending = PARENT
            continue
        summary = SUMMARY.fullmatch(line)
        require(summary is not None and summary[1] == 'ok', context)
        require(tuple(map(int, summary.group(3, 4, 5))) == (0, 0, 0), context)
        require(int(summary[2]) == active['count'] == len(active['names']), context)
        require(len(set(active['names'])) == len(active['names']), context)
        harnesses.append(active)
        active = None
    require(active is None and pending is None and child is None and harnesses, 'truncated or empty test log')
    parents = sum(h['names'].count(PARENT) for h in harnesses)
    require((parents, nested) in [(0, 0), (1, 3)], 'nested contract mismatch')
    return harnesses, nested


def parse_list(text):
    harnesses = []
    active = None
    for raw in text.splitlines():
        line = raw.strip()
        if not line:
            continue
        header = RUN.fullmatch(line)
        if header:
            require(active is None, 'unfinished list harness')
            active = {'key': header[1], 'names': []}
        elif active is not None:
            name = re.fullmatch(r'(\S+): test', line)
            if name:
                active['names'].append(name[1])
            else:
                end = re.fullmatch(r'(\d+) tests?, 0 benchmarks?', line)
                require(end is not None and int(end[1]) == len(active['names']), 'bad list summary')
                require(len(set(active['names'])) == len(active['names']), 'duplicate list name')
                harnesses.append(active)
                active = None
        else:
            require(not line.endswith(': test'), 'list entry outside harness')
    require(active is None and harnesses, 'truncated or empty list log')
    return harnesses


def reconcile(test_text, list_text):
    actual, nested = parse_tests(test_text)
    listed = parse_list(list_text)
    require(len(actual) == len(listed), 'harness count mismatch')
    for run, inventory in zip(actual, listed):
        require(run['key'] == inventory['key'], 'harness identity mismatch')
        require(Counter(run['names']) == Counter(inventory['names']), 'per-harness list mismatch')
    names = [n for h in actual for n in h['names']]
    return {'top_level_passed_records': len(names), 'distinct_top_level_names': len(set(names)),
            'harness_count': len(actual), 'nested_child_passed_records': nested,
            'per_harness_multiset_equal': True}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def reconcile_receipts(directory):
    directory = Path(directory)
    bindings = {}
    sources = None
    for label in ['tests', 'list', 'fmt', 'clippy']:
        path = directory / f'{label}-result.json'
        receipt = json.loads(path.read_text())
        require(receipt['exit'] == 0 and receipt['source_unchanged'] is True, f'{label} failed or changed inputs')
        require(receipt['source_before'] == receipt['source_after'], f'{label} source mismatch')
        require(isinstance(receipt['source_before'], dict) and receipt['source_before'] and all(isinstance(k, str) and isinstance(v, str) and re.fullmatch(r'[a-f0-9]{64}', v) for k, v in receipt['source_before'].items()), f'{label} invalid source hash map')
        require(receipt['log_sha256'] == sha(directory / f'{label}.log'), f'{label} log hash mismatch')
        if sources is None:
            sources = receipt['source_before']
        require(sources == receipt['source_before'], f'{label} input snapshot differs')
        bindings[path.name] = sha(path)
        bindings[f'{label}.log'] = receipt['log_sha256']
    summary_path = directory / 'summary.json'
    summary = json.loads(summary_path.read_text())
    require(summary['source_before'] == summary['source_after'] == sources, 'summary input mismatch')
    result = reconcile((directory / 'tests.log').read_text(), (directory / 'list.log').read_text())
    listed_receipt = json.loads((directory / 'list-result.json').read_text())
    parsed_names = [n for h in parse_list((directory / 'list.log').read_text()) for n in h['names']]
    require(parsed_names == listed_receipt['listed_names'], 'list receipt disagrees with raw log')
    result.update(status='PASS_STRICT_RECONCILIATION', input_sha256=bindings,
                  original_summary_sha256=sha(summary_path), original_summary_passed=summary['passed'],
                  raw_summary_accounting={key: summary[key] for key in ['actual_execution_count', 'passed_execution_count', 'listed_count', 'run_list_multiset_equal']},
                  source_snapshot=sources,
                  source_snapshot_sha256=hashlib.sha256(json.dumps(sources, sort_keys=True, separators=(',', ':')).encode()).hexdigest())
    return result


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    result = reconcile_receipts(args.directory)
    with args.output.open('x') as stream:
        json.dump(result, stream, indent=2)
        stream.write('\n')
    print(json.dumps({k: v for k, v in result.items() if k not in ['source_snapshot', 'input_sha256']}, indent=2))
