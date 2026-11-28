"""Offline H&V AEC preprocessing audit, never opens AEB/ERZ or the game.

Explicit include aliases bind extracted instance IDs to literal source requests.
Domain callbacks observe a bounded command set; they are not native compilers.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path, PureWindowsPath
import re

from argscript_functions import install_meta_extensions
from argscript_preprocessor import Preprocessor, main_values
from argscript_variables import ArgScriptError, Variables


DOMAIN = frozenset(('local', 'agent', 'nearby', 'connected', 'map', 'global',
                    'applycount', 'maxapplycount', 'onsuccess', 'onfail', 'chain',
                    'options', 'successevent', 'flags', 'globalflags', 'timetrigger',
                    'rate', 'priority', 'testagent'))


def instance_id(name):
    value = 0x811c9dc5
    for byte in name.lower().encode('ascii'):
        value = ((value * 0x01000193) & 0xffffffff) ^ byte
    return f'{value:08X}'


def load_corpus(directory):
    sources, files, by_id = {}, [], {}
    for path in sorted(directory.glob('HeroesAndVillains-Scripts_257365692_*.aec')):
        data = path.read_bytes()
        identity = path.stem.rsplit('_', 1)[-1]
        text = data.decode('utf-8-sig')
        by_id[identity] = text
        sources['corpus/' + identity + '.er2'] = text
        files.append(dict(path=str(path.resolve()), instance=identity,
                          sha256=hashlib.sha256(data).hexdigest(), bytes=len(data)))
    aliases = []
    for identity, text in by_id.items():
        for requested in re.findall(r'^\s*include\s+"([^"]+)"', text, re.M | re.I):
            target = instance_id(PureWindowsPath(requested).stem)
            if target not in by_id:
                raise ValueError(f'Unresolved source alias: {requested} -> {target}')
            key = 'corpus/' + requested
            if key in sources and sources[key] != by_id[target]:
                raise ValueError('Conflicting include source alias: ' + key)
            sources[key] = by_id[target]
            aliases.append(dict(fromInstance=identity, request=requested, targetInstance=target,
                                effectivePath=key, policy='explicit instance-name FNV1 binding'))
    return sources, files, aliases


def attempt(sources, roots, bindings):
    host = Preprocessor(sources=sources)
    host.parser.variables = Variables(external_values=tuple(bindings.items()))
    install_meta_extensions(host)
    rows, rule_stack = [], []

    def observe(parser, invocation):
        rows.append(dict(text=' '.join(t['text'] for t in invocation['argv']),
                         origin=dict(invocation['origin']), context=parser.trace[-1]))

    def header(parser, invocation):
        main_values(invocation, 1)
        if rule_stack:
            raise ArgScriptError('ASA001', 'Nested domain rule (audit observer guard)')
        rule_stack.append(invocation['command'])
        observe(parser, invocation)

        def end(parser, error):
            rule_stack.pop()
            if not error:
                origin = parser.trace[-1]['origin']
                rows.append(dict(text='end', origin=dict(origin), context=parser.trace[-1]))
        parser.start_block({}, end)

    def body(parser, invocation):
        if not rule_stack:
            raise ArgScriptError('ASA002', 'Domain command outside rule (audit observer guard)')
        observe(parser, invocation)

    for name in ('unitRule', 'globalRule'):
        host.parser.register(name, header)
    for name in DOMAIN:
        host.parser.register(name, body)
    failure = None
    try:
        for root in roots:
            host.parse_source('corpus/' + root + '.er2')
    except ArgScriptError as error:
        failure = dict(code=error.code, message=str(error), file=getattr(error, 'file', None),
                       line=getattr(error, 'line', None), phase=getattr(error, 'phase', None),
                       includeStack=getattr(error, 'include_stack', []),
                       macroFrames=getattr(error, 'macro_frames', []))
    expanded = []
    for row in rows:
        origin, context = row['origin'], row['context']
        frame = context.get('macroExpansion')
        expanded.append(dict(text=row['text'], file=origin['file'], line=origin['line'],
                             calls=[frame] if frame else [],
                             generated=origin.get('generated', False) or frame is not None))
    return dict(complete=failure is None, failure=failure, rows=expanded,
                expansions=host.expansions, files=host.files,
                macros=sorted(host.macros),
                macroCalls=[dict(name=row['invocation']['arguments']['main'][0]['value'],
                                 origin=row['origin']) for row in host.parser.trace
                            if row.get('phase') == 'executed'
                            and row.get('invocation', {}).get('command', '').lower() == 'create'],
                commands=dict(Counter(row['text'].split()[0].lower() for row in expanded)))


def audit(directory, roots):
    sources, files, aliases = load_corpus(directory)
    strict = attempt(sources, roots, {})
    bindings, symbolic, attempts = {}, strict, []
    while symbolic['failure'] is not None and len(bindings) < 64:
        failure = symbolic['failure']
        if failure['code'] != 'ASV005':
            break
        match = re.fullmatch(r"Unknown variable: '([^']+)'", failure['message'])
        if not match or match[1] in bindings:
            break
        name = match[1]
        # This preserves an unresolved symbol through single-pass substitution.
        # It is not a real external value, numeric success or compile permission.
        bindings[name] = '${' + name + '}'
        attempts.append(dict(variable=name, reason=failure))
        symbolic = attempt(sources, roots, bindings)
    return dict(schema='openscp.argscript.corpus-audit/1', roots=roots, sourceFiles=files,
                includeAliases=aliases, strict=strict,
                symbolic=dict(symbolic, externalBindings=bindings, discovery=attempts),
                packageCompilationAllowed=False,
                limitations=['Source preprocessing and domain observation only',
                             'Include aliases are explicitly reconstructed, not recovered disk layout',
                             'Symbolic bindings are placeholders, never real numeric values',
                             'No AEB/ERZ comparison, native execution or runtime capture'])


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sources', type=Path, required=True)
    parser.add_argument('--roots', nargs='+', default=['8F06F716', 'F6ED959D'])
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--ast-out', type=Path)
    args = parser.parse_args()
    report = audit(args.sources, args.roots)
    if args.ast_out is not None and report['symbolic']['complete']:
        from unit_rule_frontend import parse_expanded
        ast = parse_expanded(report['symbolic']['rows'])
        args.ast_out.parent.mkdir(parents=True, exist_ok=True)
        args.ast_out.write_text(json.dumps(ast, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
        report['domain'] = dict(rules=len(ast['rules']), diagnostics=ast['diagnostics'],
                               kinds=dict(Counter(row['kind'] for row in ast['rules'])),
                               packageCompilationAllowed=False, path=str(args.ast_out))
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(dict(files=len(report['sourceFiles']), strict=report['strict']['failure'],
                          symbolicComplete=report['symbolic']['complete'],
                          symbolicFailure=report['symbolic']['failure'],
                          rows=len(report['symbolic']['rows']),
                          macroCalls=len(report['symbolic']['macroCalls']),
                          domain=report.get('domain'),
                          bindings=report['symbolic']['externalBindings']), ensure_ascii=False))
