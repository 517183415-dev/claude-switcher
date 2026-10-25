#!/usr/bin/env python3
"""Independent PyYAML verification of actual Rust propose receipts; no generation."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

import yaml

BEGIN = '# >>> claude-switcher pinned rules >>>'
END = '# <<< claude-switcher pinned rules <<<'
DOMAINS = {'claude.ai', 'claude.com', 'anthropic.com', 'claudeusercontent.com',
           'challenges.cloudflare.com', 'datadoghq.com', 'segment.io', 'stripe.com',
           'stripe.network', 'stripecdn.com', 'intercom.io', 'intercomcdn.com',
           'intercomassets.com'}


class UniqueLoader(yaml.SafeLoader):
    pass


def mapping(loader, node, deep=False):
    keys = [loader.construct_object(key, deep=deep) for key, _ in node.value]
    if len(set(keys)) != len(keys):
        raise ValueError('duplicate YAML mapping key')
    return yaml.SafeLoader.construct_mapping(loader, node, deep=deep)


UniqueLoader.add_constructor(yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG, mapping)


def parse(text, output=False):
    nodes = list(yaml.compose_all(text, Loader=yaml.SafeLoader))
    values = list(yaml.load_all(text, Loader=UniqueLoader))
    if not nodes and not output:
        return {}, None
    if len(nodes) != 1:
        raise ValueError(f'expected one YAML document; got {len(nodes)}')
    value = values[0]
    if value is None and not output:
        return {}, nodes[0]
    if not isinstance(value, dict):
        raise ValueError('YAML document root is not a mapping')
    return value, nodes[0]


def marker_span(text, required=False):
    lines = text.splitlines(keepends=True)
    starts = [index for index, line in enumerate(lines) if line.strip() == BEGIN]
    ends = [index for index, line in enumerate(lines) if line.strip() == END]
    if not starts and not ends and not required:
        return lines, None
    if len(starts) != 1 or len(ends) != 1 or starts[0] >= ends[0]:
        raise ValueError('managed marker pair is missing, duplicated, or reversed')
    return lines, (starts[0], ends[0])


def without_managed(text, remove_header=False):
    lines, span = marker_span(text)
    remove = set(range(span[0], span[1] + 1)) if span else set()
    if remove_header:
        node = list(yaml.compose_all(text, Loader=yaml.SafeLoader))[0]
        headers = [key.start_mark.line for key, _ in node.value
                   if key.tag == 'tag:yaml.org,2002:str' and key.value == 'prepend-rules']
        if len(headers) != 1:
            raise ValueError('cannot identify the unique inserted top-level header')
        remove.add(headers[0])
        # The new section includes one separating blank line. Existing sections
        # do not take this branch, so user-owned blank lines remain exact.
        if span and span[1] + 1 < len(lines) and lines[span[1] + 1] in ('\n', '\r\n'):
            remove.add(span[1] + 1)
    return ''.join(line for index, line in enumerate(lines) if index not in remove)


def verify(case):
    expected = case.get('expected')
    if expected not in ('accept', 'reject'):
        raise ValueError('receipt must specify expected accept/reject')
    if expected == 'reject':
        if case.get('accepted') is not False or case.get('output') is not None:
            raise ValueError('rejected fixture contains an accepted result or output')
        if case.get('second_output') is not None:
            raise ValueError('rejected fixture contains second-pass output')
        return {'status': 'PASS', 'accepted': False}
    if case.get('accepted') is not True:
        raise ValueError('accepted fixture was rejected by Rust')
    original = case.get('original') or ''
    output = case.get('output')
    if not isinstance(original, str) or not isinstance(output, str):
        raise ValueError('accepted input/output must be actual UTF-8 text')
    current, node = parse(output, output=True)
    keys = [key.value for key, _ in node.value if key.tag == 'tag:yaml.org,2002:str']
    if keys.count('prepend-rules') != 1:
        raise ValueError('output does not have exactly one top-level prepend-rules')
    if not isinstance(current.get('prepend-rules'), list):
        raise ValueError('top-level prepend-rules is not a sequence')
    lines, span = marker_span(output, required=True)
    managed = yaml.load('prepend-rules:\n' + ''.join(lines[span[0]:span[1] + 1]), Loader=UniqueLoader)['prepend-rules']
    if not isinstance(managed, list) or len(managed) != 13:
        raise ValueError('managed block does not contain exactly 13 YAML rules')
    parsed = [rule.split(',', 2) if isinstance(rule, str) else [] for rule in managed]
    if any(len(rule) != 3 or rule[0] != 'DOMAIN-SUFFIX' for rule in parsed):
        raise ValueError('managed rule has an invalid scalar/type/prefix')
    if {rule[1] for rule in parsed} != DOMAINS:
        raise ValueError('managed domains differ from the 13-domain contract')
    groups = {rule[2] for rule in parsed}
    if len(groups) != 1 or (case.get('group') is not None and groups != {case['group']}):
        raise ValueError('managed proxy group differs')
    if current['prepend-rules'][:13] != managed:
        raise ValueError('managed rules do not precede user rules')
    original_clean = without_managed(original)
    # Legacy migration intentionally removes a historical managed root key;
    # parsing the untouched legacy document would reject that duplicate first.
    baseline, _ = parse(original_clean)
    output_clean = without_managed(output, remove_header='prepend-rules' not in baseline)
    before, _ = parse(original_clean)
    after, _ = parse(output_clean)
    if before != after:
        raise ValueError('user top-level, nested, alias, or scalar content changed')
    if output_clean != original_clean and not (
            not original_clean.endswith(('\n', '\r')) and output_clean in (original_clean + '\n', original_clean + '\r\n')):
        raise ValueError('non-managed user bytes changed')
    if type(case.get('changed')) is not bool or case['changed'] != (output != original):
        raise ValueError('first-pass changed receipt differs from actual bytes')
    if case.get('second_output') != output or case.get('second_changed') is not False:
        raise ValueError('actual Rust second-pass receipt is missing or not idempotent')
    return {'status': 'PASS', 'accepted': True, 'managed_rules': 13,
            'output_sha256': hashlib.sha256(output.encode()).hexdigest()}


def read_cases(path):
    text = path.read_text()
    try:
        value = json.loads(text)
    except json.JSONDecodeError:
        value = [json.loads(line.split('F10_FIXTURE ', 1)[1]) for line in text.splitlines() if 'F10_FIXTURE ' in line]
    if isinstance(value, dict):
        value = value.get('cases', value.get('fixtures', [value]))
    if not isinstance(value, list) or not value:
        raise ValueError('no Rust fixture receipts found')
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('artifact', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    cases = read_cases(args.artifact)
    results = []
    for index, case in enumerate(cases):
        row = {'index': index, 'name': case.get('name', f'case-{index}')}
        try:
            row.update(verify(case))
        except yaml.constructor.ConstructorError as error:
            row.update(status='BLOCKED', reason=str(error))
        except (ValueError, yaml.YAMLError, TypeError, RecursionError) as error:
            row.update(status='FAIL', reason=str(error))
        results.append(row)
    counts = {status: sum(row['status'] == status for row in results) for status in ('PASS', 'FAIL', 'BLOCKED')}
    receipt = {'python': sys.executable, 'pyyaml': yaml.__version__, 'yaml_module': yaml.__file__,
               'artifact': str(args.artifact.resolve()), 'artifact_sha256': hashlib.sha256(args.artifact.read_bytes()).hexdigest(),
               'script_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(), 'counts': counts, 'cases': results}
    args.report.write_text(json.dumps(receipt, indent=2, ensure_ascii=False) + '\n')
    print(json.dumps(counts))
    return 1 if counts['FAIL'] else 2 if counts['BLOCKED'] else 0


if __name__ == '__main__':
    raise SystemExit(main())
