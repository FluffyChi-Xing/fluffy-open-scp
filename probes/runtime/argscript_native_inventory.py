"""Pin development decompiler evidence for the standalone parser prototype.

Copies verbatim function files and emits their identity, addresses and selected
Init registrations. Does not execute native code or inspect retail packages.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re


FUNCTIONS = (
    'StripCommentsAndWhiteSpace.c', 'MakeCaseInsensitive.c',
    'cArguments/SplitIntoArguments.c', 'cArguments/Chomp.c',
    'cParser/ParseCommandInternal.c', 'cParser/ParseArguments.c',
    'cParser/SubstituteVariables.c', 'cParser/Variable.c',
    'cParser/SetVariable.c', 'cParser/SetVariableNoSubstitution.c',
    'cParser/SetGlobalVariable.c', 'cParser/PushNamespace.c',
    'cParser/PopNamespace.c', 'cParser/RegisterCommand.c',
    'cParser/CommandLookup.c', 'cParser/StartBlock.c',
    'cParser/StartMetaBlock.c', 'cParser/ParseStream.c',
    'cParser/ParseEnd.c', 'cParser/Init.c', 'cParser/InstantiateMacro.c',
)

ARGUMENT_FUNCTIONS = (
    'cArgumentSpec/cArgumentSpec.c', 'cArgumentSpec/ConstructSpec.c',
    'cArgumentSpec/Parse.c', 'cArgumentSpec/Parse__2.c',
    'cArgumentSpec/ParseOption.c', 'cArgumentSpec/ParseOptionArgs.c',
    'cArgumentSpec/ParseArgument.c', 'cArgumentSpec/ParseArgument__2.c',
    'cArgumentSpec/ParseVectorArgument.c', 'cArgumentSpec/ParseVectorArgument__2.c',
    'cArgumentSpec/FindNameAndTypeFromOption.c', 'cArgumentSpec/ArgTypeFromName.c',
    'cArguments/AsArgV.c', 'cArguments/MarkAllOptionsHandled.c',
    'cArguments/NumArguments.c', 'cArguments/operator.c', 'ParseEnum.c', 'ParseEnum__2.c',
    'cArguments/SplitIntoArguments.c',
)

NUMBER_FUNCTIONS = tuple('cExpression/' + name + '.c' for name in (
    'ParseInt', 'ParseUint', 'ParseBool', 'ParseReal',
    'EvalIntExpression', 'EvalIntTerm', 'EvalIntFactor', 'EvalSignedIntNumber',
    'EvalIntNumber', 'EvalIntParentheses', 'EvalBoolExpression', 'EvalBoolTerm',
    'EvalBoolFactor', 'EvalRelExpression', 'EvalBool',
    'EvalIdentifier', 'EvalIdentifier__2', 'EvalChar',
    'EvalRealNumber', 'EvalSignedRealNumber', 'EvalRealFactor', 'EvalRealTerm',
    'EvalRealExpression',
)) + ('cParser/ParseInt.c', 'cParser/ParseBool.c', 'cParser/ParseFloat.c')

REAL_FUNCTIONS = tuple('cExpression/' + name + '.c' for name in (
    'ParseReal', 'EvalRealExpression', 'EvalRealTerm', 'EvalRealFactor',
    'EvalSignedRealNumber', 'EvalRealNumber', 'EvalRealParentheses',
    'EvalRealParentheses2', 'EvalContinueChar', 'EvalChar',
)) + tuple('cParser/' + name + '.c' for name in (
    'ParseFloat', 'ParseVector2', 'ParseVector3', 'ParseVector4', 'ParseRGBA',
))

PREPROCESSOR_FUNCTIONS = tuple('cParser/' + name + '.c' for name in (
    'Init', 'InstantiateMacro', 'InstantiateMacroWithResult', 'ParseStream',
    'PushDir', 'PopDir', 'DefinitionExists',
)) + ('cArguments/MainArguments.c', 'cArguments/MainArguments__2.c',
      'cArguments/SplitIntoArguments.c', 'cArguments/NumArguments.c', 'cArguments/operator.c')
PREPROCESSOR_HANDLERS = (
    'cDefineCommand/Execute.c', 'cDefineCommand/Parse.c', 'cDefineCommand/OnEndParse.c',
    'cCreateCommand/Execute.c', 'cScreateCommand/Execute.c',
    'cIfCommand/Execute.c', 'cIfCommand/Parse.c', 'cIfCommand/OnEndParse.c',
    'cNamespaceCommand/Execute.c', 'cNamespaceCommand/OnEndBlock.c',
    'cEndCommand/Execute.c', 'cSetCommand/Execute.c',
    'cIncludeCommand/Execute.c', 'cSincludeCommand/Execute.c',
    'cPushDirCommand/Execute.c', 'cPopDirCommand/Execute.c',
    'cUndefineCommand/Execute.c', 'SubstituteParameters.c', 'FindParentDir.c',
)
FUNCTION_CORE = ('cExpression/RegisterFunction.c', 'cExpression/EvalString.c',
                 'cExpression/EvalIntNumber.c', 'cExpression/EvalBool.c',
                 'cExpression/EvalRealNumber.c', 'cExprFunction/EvalBool.c',
                 'cParser/CommandExists.c', 'cParser/Init.c')
FUNCTION_HANDLERS = tuple(name + '/Execute.c' for name in (
    'cSetICommand', 'cSetUCommand', 'cSetFCommand', 'cSetBCommand',
)) + tuple(name + '/EvalInt.c' for name in (
    'cMatchFunction', 'cEqFunction', 'cVariableExistsFunction',
    'cDefinitionExistsFunction', 'cCommandExistsFunction',
    'cMinVersionFunction', 'cMaxVersionFunction',
)) + ('cVariableExistsFunction/EvalReal.c',)


def inventory(root, output, scope='pipeline'):
    # Read/validate all inputs before writing an incomplete evidence bundle.
    names = {'pipeline': FUNCTIONS, 'arguments': ARGUMENT_FUNCTIONS,
             'numbers': NUMBER_FUNCTIONS, 'real': REAL_FUNCTIONS,
             'preprocessor': PREPROCESSOR_FUNCTIONS, 'functions': FUNCTION_CORE, 'all': ()}[scope]
    if scope == 'all':
        names = tuple(dict.fromkeys(FUNCTIONS + ARGUMENT_FUNCTIONS + NUMBER_FUNCTIONS
                                    + REAL_FUNCTIONS + PREPROCESSOR_FUNCTIONS + FUNCTION_CORE))
    sources = [(name, root / name) for name in names]
    if scope in ('preprocessor', 'all'):
        sources.extend(('handlers/' + name, root.parents[1] / 'anonymous' / name)
                       for name in PREPROCESSOR_HANDLERS)
    if scope in ('functions', 'all'):
        sources.extend(('function-handlers/' + name, root.parents[1] / 'anonymous' / name)
                       for name in FUNCTION_HANDLERS)
        sources.extend(('dependencies/StdC/' + name, root.parent / 'StdC' / name)
                       for name in ('WildcardMatch.c', 'MatchPattern.c'))
    if scope in ('arguments', 'all'):
        header_root = root.resolve().parents[2] / 'include/EA/ArgScript'
        sources.extend(('types/' + name, header_root / name) for name in (
            'tArgError_ord272.h', 'tArgType_ord44.h', 'cArgInfo_ord6562.h'))
    if scope in ('numbers', 'all'):
        sources.extend(('dependencies/StdC/' + name, root.parent / 'StdC' / name)
                       for name in ('AtoI64.c', 'StrtoI32.c', 'StrtoU64.c',
                                    'StrtoU64Common.c', 'StrtodEnglish.c'))
        sources.append(('dependencies/Compare.c', root.parents[1] / 'anonymous/Compare.c'))
    if scope == 'real':
        sources.append(('dependencies/StdC/StrtodEnglish.c', root.parent / 'StdC/StrtodEnglish.c'))
        sources.append(('dependencies/Compare.c', root.parents[1] / 'anonymous/Compare.c'))
    inputs = [(name, path, path.read_bytes()) for name, path in sources]
    rows = []
    for name, source, data in inputs:
        target = output / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        text = data.decode('utf-8-sig')
        address = re.search(r'// addr: (0x[0-9a-fA-F]+)', text)
        signature = re.search(r'// demangled-sig: (.*)', text)
        rows.append(dict(path=name, kind='type-header' if name.startswith('types/') else 'function',
                         sourcePath=str(source.resolve()),
                         sha256=hashlib.sha256(data).hexdigest(),
                         address=address.group(1) if address else None,
                         signature=signature.group(1) if signature else None))
    init = next((data.decode('utf-8-sig') for name, _, data in inputs
                 if name == 'cParser/Init.c'), '')
    # Only direct RegisterCommand statements; not an inferred complete grammar.
    registrations = re.findall(r'RegisterCommand\)?\([^;]*?a2: "([^"]+)"[^;]*;', init, re.S)
    report = dict(schema='openscp.argscript.native-evidence/1', scope=scope,
                  sourceRoot=str(root.resolve()),
                  evidenceKind='development-decompilation-static',
                  functions=[r for r in rows if r['kind'] == 'function'],
                  typeHeaders=[r for r in rows if r['kind'] == 'type-header'],
                  directInitCommandRegistrations=registrations,
                  limitations=['Decompiler output is not original C++ source',
                               'Addresses are development-only, not retail probe addresses',
                               'Registration inventory does not establish command semantics',
                               'No native execution, retail comparison or runtime capture'])
    (output / 'manifest.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n',
                                        encoding='utf-8')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--scope', choices=['pipeline', 'arguments', 'numbers', 'real',
                                          'preprocessor', 'functions', 'all'], default='pipeline')
    args = parser.parse_args()
    report = inventory(args.source, args.out, args.scope)
    print(json.dumps(dict(functions=len(report['functions']),
                         typeHeaders=len(report['typeHeaders']),
                         registrations=report['directInitCommandRegistrations'],
                         manifest=str(args.out / 'manifest.json')), ensure_ascii=False))
