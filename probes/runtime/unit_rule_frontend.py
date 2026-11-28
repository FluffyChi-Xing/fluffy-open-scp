"""Research frontend for expanded H&V rules, separate from raw meta syntax.

Known commands outside this corpus remain unsupported. Unknown commands are
preserved with diagnostics, never silently accepted for package generation.
"""
import argparse
from collections import Counter
import json
from pathlib import Path
import re

from argscript_syntax import diagnostic, scan
from hero_villain_source_audit import fnv1
from unit_rule_domain import ContractError, lower

SCOPES = {'local', 'agent', 'nearby', 'connected', 'map', 'global'}
VERBS = {'in', 'out', 'is', 'atleast', 'atmost'}
KNOWN = SCOPES | {'applycount', 'maxapplycount', 'onsuccess', 'onfail', 'chain',
                  'options', 'successevent', 'flags', 'globalflags', 'timetrigger',
                  'rate', 'priority', 'testagent'}
IDENTIFIER = r'[A-Za-z_][A-Za-z0-9_:]*'


def quantity(text):
    if re.fullmatch(r'[+-]?\d+', text):
        return dict(kind='integer', value=int(text), text=text)
    if re.fullmatch(r'[+-]?0[xX][0-9a-fA-F]+', text):
        return dict(kind='integer', value=int(text, 0), text=text)
    match = re.fullmatch(r'(!?)(' + IDENTIFIER + r')@(\w+)', text)
    if match:
        if match[3].lower() not in SCOPES:
            return dict(kind='unsupported', text=text)
        return dict(kind='binRead', resource=match[2], resourceId=f'{fnv1(match[2]):08X}',
                    scope=match[3].lower(), remainingCapacity=bool(match[1]), text=text)
    if re.fullmatch(r'\$(?:\{' + IDENTIFIER + r'\}|' + IDENTIFIER + r')', text):
        return dict(kind='externalVariable', text=text)
    return dict(kind='unsupported', text=text)


def parse_expanded(rows):
    rules, diagnostics, seen = [], [], set()
    current = None
    generated_offset = 0
    for ordinal, origin in enumerate(rows, 1):
        row_offset = generated_offset
        generated_offset += len(origin['text']) + 1
        # Generated text columns are not columns in a macro definition/call site.
        document = scan(origin['text'], '<expanded>')
        for error in document['diagnostics']:
            diagnostics.append(dict(error, line=ordinal, origin=origin))
        if not document['lines']:
            continue
        line = document['lines'][0]
        if len(document['lines']) != 1:
            diagnostics.append(diagnostic('UR001', 'expanded row must contain one physical line', '<expanded>', ordinal, 1, origin=origin))
            continue
        line['number'] = ordinal
        line['start'] += row_offset
        line['positions'] = [p + row_offset for p in line['positions']]
        for comment in line['comments']:
            comment['start'] += row_offset; comment['end'] += row_offset
        for token in line['tokens']:
            token['span']['line'] = ordinal
            token['span']['start'] += row_offset; token['span']['end'] += row_offset
            for segment in token['sourceSegments']:
                segment['start'] += row_offset; segment['end'] += row_offset
        view = line['arguments']
        main = [t['value'] for t in view['main']]
        if not main:
            continue
        command = main[0].lower()
        node = dict(command=command, text=origin['text'], origin=origin,
                    generatedLine=ordinal, syntax=line)

        def error(code, message):
            diagnostics.append(diagnostic(code, message, '<expanded>', ordinal,
                                          line['tokens'][0]['span']['column'], origin=origin))

        if command in {'unitrule', 'globalrule'}:
            if current:
                error('UR002', 'nested rule in expanded stream')
                continue
            if len(main) != 2 or view['options'] or not re.fullmatch(IDENTIFIER, main[1]):
                error('UR003', 'rule header requires one identifier')
                continue
            key = (command, main[1].lower())
            if key in seen:
                error('UR004', 'duplicate rule name within rule kind')
            seen.add(key)
            current = dict(kind=command, name=main[1], ruleIdHex=f'{fnv1(main[1]):08X}',
                           origin=origin, header=node, body=[], edges=[])
            rules.append(current)
            continue
        if command == 'end':
            if len(main) != 1 or view['options']:
                error('UR005', 'end does not accept arguments')
            if current is None:
                error('UR006', 'end without open rule')
            else:
                current['end'] = node
                current = None
            continue
        if current is None:
            error('UR007', 'unsupported top-level or unexpanded meta command')
            continue
        current['body'].append(node)
        if command not in KNOWN:
            error('UR008', 'unsupported rule command retained for review')
            node['kind'] = 'unsupportedCommand'
            continue
        node['kind'] = 'command'
        if command in SCOPES:
            if len(main) != 4 or view['options'] or not re.fullmatch(IDENTIFIER, main[1]) or main[2].lower() not in VERBS:
                error('UR009', 'resource command requires resource, verb and quantity')
                continue
            value = quantity(main[3])
            node.update(kind='resourceOperation', scope=command, resource=main[1],
                        resourceId=f'{fnv1(main[1]):08X}', verb=main[2].lower(), quantity=value)
            if value['kind'] == 'unsupported':
                error('UR010', 'quantity syntax outside verified subset')
        elif command in {'onsuccess', 'onfail', 'chain'}:
            if len(main) > 2 or (len(main) == 2 and not re.fullmatch(IDENTIFIER, main[1])):
                error('UR011', 'rule edge has invalid target arguments')
                continue
            node['kind'] = 'edge'
            if len(main) == 2:
                target = main[1]
                for branch in (['success', 'fail'] if command == 'chain' else ['success' if command == 'onsuccess' else 'fail']):
                    current['edges'].append(dict(branch=branch, target=target, targetId=f'{fnv1(target):08X}', origin=origin))
            if len(main) == 1 and not view['options']:
                error('UR011', 'rule edge requires a target or event option')
        elif command in {'applycount', 'maxapplycount'}:
            count = len(main) - 1
            if (command == 'applycount' and count not in [1, 2]) or view['options']:
                error('UR012', 'unsupported apply count form')
            node['argumentsRaw'] = main[1:]
        try:
            domain = lower(node)
            if domain is not None:
                node['domain'] = domain
        except ContractError as exc:
            error(exc.code, str(exc))
    if current:
        diagnostics.append(diagnostic('UR013', 'unclosed expanded rule', '<expanded>', len(rows), 1, origin=current['origin']))
    symbols = {(r['kind'], r['name'].lower()) for r in rules}
    for rule in rules:
        for edge in rule['edges']:
            edge['resolution'] = 'local' if ('unitrule', edge['target'].lower()) in symbols else 'external-unresolved'
    return dict(schema='openscp.unit-rule.frontend/0', rules=rules, diagnostics=diagnostics,
                success=not diagnostics, domainValidation='partial', packageCompilationAllowed=False,
                unresolvedVariables=sorted(set(re.findall(r'\$\{' + IDENTIFIER + r'\}|\$' + IDENTIFIER,
                                                          '\n'.join(r['text'] for r in rows)))),
                limitations=['Expanded H&V dialect only; no general macro evaluator or include loader',
                             'Known command retention does not validate all native argument grammars',
                             'No binary encoding, runtime attachment or production UI integration'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('input', type=Path)
    parser.add_argument('--expanded-json', action='store_true')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    text = args.input.read_text(encoding='utf-8-sig')
    result = parse_expanded(json.loads(text)) if args.expanded_json else scan(text, str(args.input))
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(result, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    print(json.dumps(dict(diagnostics=len(result['diagnostics']),
                          rules=len(result.get('rules', [])),
                          kinds=dict(Counter(r['kind'] for r in result.get('rules', []))))))
    return int(bool(result['diagnostics']))


if __name__ == '__main__':
    raise SystemExit(main())
