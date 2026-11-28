"""Bounded H&V source inventory and literal UnitRule/AEB comparison.

This is NOT an ArgScript parser or macro preprocessor. Macro bodies/calls are
inventoried without expansion. Only ordinary top-level UnitRule blocks are
selected for binary comparison. No game process or package writes.
"""
import argparse
import hashlib
import json
import re
import struct
from collections import Counter
from pathlib import Path

from school_rule_schedule import sim_info
from unit_rule_resource_bridge import decode


def fnv1(text):
    value = 0x811C9DC5
    for byte in text.lower().encode('utf-8'):
        value = ((value * 0x01000193) & 0xFFFFFFFF) ^ byte
    return value


def inventory(source_dir):
    files, rules, macros, calls = [], [], [], []
    commands, symbols = Counter(), {}
    for path in sorted(source_dir.glob('HeroesAndVillains-Scripts_257365692_*.aec')):
        raw = path.read_bytes()
        files.append(dict(path=str(path), sha256=hashlib.sha256(raw).hexdigest()))
        macro, rule = None, None
        for number, line in enumerate(raw.decode('utf-8-sig').splitlines(), 1):
            # Restricted inventory corpus; not quote-aware comment lexing.
            text = line.split('#', 1)[0].strip()
            if not text:
                continue
            head = text.split()[0].lower()
            commands[head] += 1
            for token in re.findall(r'\b[A-Za-z_]\w*\b', text):
                symbols.setdefault(fnv1(token), token)
            if head == 'define':
                macro = dict(path=str(path), line=number, declaration=text)
                macros.append(macro)
                continue
            if head == 'enddef':
                macro = None
                continue
            if macro:
                continue
            if head == 'create':
                calls.append(dict(path=str(path), line=number, text=text))
            match = re.fullmatch(r'unitRule\s+(\w+)', text, re.I)
            if match:
                if rule:
                    raise ValueError('nested literal rule requires real parser')
                rule = dict(name=match[1], id=fnv1(match[1]), path=str(path),
                            startLine=number, lines=[])
            elif head == 'end' and rule:
                rule['endLine'] = number
                rules.append(rule)
                rule = None
            elif rule:
                rule['lines'].append(dict(line=number, text=text))
        if macro or rule:
            raise ValueError(f'unclosed restricted block: {path}')
    return files, rules, macros, calls, commands, symbols


def audit(source_dir, binary):
    files, literals, macros, calls, commands, symbols = inventory(source_dir)
    data = binary.read_bytes()
    report = decode(data, {rule['id'] for rule in literals})
    matched = {row['ruleId']: row for row in report['matches'] if row['scope'] == 'unit'}
    names = {rule['id']: rule['name'] for rule in literals}
    pool = data[report['poolOffset']:report['poolOffset'] + report['poolSize']]
    for rule in literals:
        row = matched.get(rule['id'])
        rule['ruleIdHex'] = f"{rule.pop('id'):08X}"
        if row is None:
            rule['binary'] = None
            continue
        word = struct.unpack_from('>33I', data, row['offset'])
        row['rawWords'] = [f'{value:08X}' for value in word]
        offset = struct.unpack_from('>i', data, row['offset'] + 8)[0]
        row['schedule'] = sim_info(pool, offset)
        row['minApply'], row['maxApply'] = word[15:17]
        row['edges'] = {
            side: dict(count=word[index - 1], target=f'{word[index]:08X}',
                       literalTarget=names.get(word[index]), rawDelay=word[index + 1])
            for side, index in [('success', 19), ('failOrChain', 23)]
        }
        for ops in row['resourcePrefix'].values():
            for op in ops['values']:
                op['sourceTokenHashCandidate'] = symbols.get(int(op['resourceId'], 16))
        for op in row['fromBinOps']['values']:
            op['sourceTokenHashCandidate'] = symbols.get(int(op['sourceResourceId'], 16))
        rule['binary'] = row
    report.pop('matches')
    report.update(sourceFiles=files, literalRules=literals, macroDeclarations=macros,
                  macroCalls=calls, commandFrequency=dict(commands),
                  counts=dict(files=len(files), literalRules=len(literals),
                              matchedLiteralRules=len(matched), macros=len(macros), calls=len(calls)))
    report['limitations'].extend([
        'Early AEC and later AEB: hash match is identity evidence, not version equivalence',
        'Restricted line inventory, not a lexer/parser; no macro expansion or linking',
        'Resource token names are hash candidates, not runtime observations',
        'Edges expose decoded storage; scheduling/chain interpretation needs engine evidence',
    ])
    return report


def early_identity(path, rules):
    """Layout-13 identity only; do not decode its pool with retail offsets."""
    data = path.read_bytes()
    if data[:13] != struct.pack('>III', 4, 2, 13) + b'\0':
        raise ValueError('unsupported early identity layout')
    count = struct.unpack_from('>I', data, 13)[0]
    offset, ids = 17, set()
    for _ in range(count):
        ids.add(struct.unpack_from('>I', data, offset)[0])
        transports = struct.unpack_from('>I', data, offset + 148)[0]
        offset += 160 + 12 * transports
        if offset > len(data):
            raise ValueError('early unit section bounds')
    if struct.unpack_from('>I', data, offset)[0] != 4:
        raise ValueError('early map section tag')
    return dict(path=str(path), sha256=hashlib.sha256(data).hexdigest(),
                unitCount=count, unitSectionEnd=offset, nextMapTag=4,
                matchedLiteralRules=sum(int(row['ruleIdHex'], 16) in ids for row in rules),
                limitation='Unit-section identity only, not complete layout-13 decoding')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-dir', type=Path, required=True)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--early-binary', type=Path)
    args = parser.parse_args()
    result = audit(args.source_dir, args.binary)
    if args.early_binary:
        result['earlyIdentity'] = early_identity(args.early_binary, result['literalRules'])
    args.out.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(exact=result['exact'], sha256=result['sha256'], **result['counts'])))
