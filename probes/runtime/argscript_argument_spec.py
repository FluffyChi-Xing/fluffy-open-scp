"""Native-derived cArgumentSpec argument consumption, with explicit conversion.

Numeric/expression conversions are requests unless a host supplies their actual
implementation. Structural success alone is never advertised as full conversion.
No native varargs ABI, memory writes, help renderer or ER2 compilation.
"""
from dataclasses import dataclass, replace

from argscript_syntax import split_arguments
from argscript_variables import ArgScriptError, ascii_key


VECTOR = 0x4000
REPEAT = 0x8000
BASE = 0x3fff


@dataclass(frozen=True)
class ArgInfo:
    name: str
    type_id: int = 6
    dependent: bool = True
    destination: str | None = None
    flag: int = -1


@dataclass(frozen=True)
class OptionSpec:
    name: str
    arguments: tuple = ()
    flag: int = -1


@dataclass(frozen=True)
class EnumSpec:
    name: str
    values: tuple = ()
    help_values: tuple = ()


@dataclass(frozen=True)
class ArgumentSpec:
    arguments: tuple = ()
    options: tuple = ()
    enums: tuple = ()
    respect_help: bool = True


@dataclass(frozen=True)
class ConversionRequest:
    route: str
    text: str
    type_id: int


@dataclass(frozen=True)
class SpecResult:
    native_code: int
    message: str
    flags: int
    writes: tuple
    cursor: int
    unresolved: tuple

    @property
    def fully_converted(self):
        return self.native_code == 0 and not self.unresolved


def is_option(value):
    return len(value) > 1 and value[0] == '-' and value[1].isascii() and value[1].isalpha()


def dependency_slots(rows):
    """Recover ConstructSpec bracket dependency from explicit (token, ArgInfo).

    Token types/destinations are supplied, not inferred from native varargs.
    One leading '[' and all trailing ']' are handled as in ConstructSpec.
    Returns ArgInfos; '...' sets VECTOR|REPEAT on the preceding slot.
    """
    result, depth, previous = [], 0, 0
    for index, (token, info) in enumerate(rows):
        if token.startswith('['):
            depth += 1
            token = token[1:]
        dependent = depth == previous
        while token.endswith(']'):
            depth -= 1
            token = token[:-1]
        if token == '...':
            if not result or index != len(rows) - 1:
                raise ArgScriptError('ASC002', 'kSpecEllipsisError')
            result[-1] = replace(result[-1], type_id=result[-1].type_id | VECTOR | REPEAT)
        else:
            if info is None or not token:
                raise ArgScriptError('ASC003', 'Explicit ArgInfo required (host guard)')
            result.append(replace(info, dependent=dependent))
            previous = depth
    if depth != 0:
        raise ArgScriptError('ASC004', 'kSpecUnbalancedBrackets')
    return tuple(result)


def parse_argv(spec, argv, *, parser_present=True, converters=None):
    """Consume raw values including command at argv[0], retaining partial writes.

    converters is a route -> callable(text) map. Missing converter produces a
    ConversionRequest, never Python eval/int/float standing in for native code.
    """
    if not argv:
        raise ArgScriptError('ASC005', 'Empty argv is outside the native API precondition')
    converters = {} if converters is None else converters
    flags, writes, unresolved = 0, [], []

    def flag(bit):
        nonlocal flags
        if bit >= 0:
            if bit > 31:
                raise ArgScriptError('ASC006', 'Flag index outside modeled 32-bit range')
            flags |= 1 << bit

    def result(code, message, cursor):
        return SpecResult(code, message, flags, tuple(writes), cursor, tuple(unresolved))

    def convert(info, value, append, index):
        base = info.type_id & BASE if append else info.type_id
        route = None
        if base in (1, 2, 3, 4):
            if parser_present and base in (1, 2, 3):
                route = {1: 'ParseBool', 2: 'ParseInt', 3: 'ParseFloat'}[base]
            elif base in (1, 2):
                route = 'StrtoI32.bool' if append and base == 1 else (
                    'StrtoI32' if append else ('AtoI64.bool' if base == 1 else 'AtoI64'))
            else:
                route = 'StrtodEnglish.float32' if base == 3 else 'StrtodEnglish.float64'
        elif parser_present and base in (7, 8, 9, 10, 11):
            route = {7: 'ParseVector2', 8: 'ParseVector3', 9: 'ParseVector4',
                     10: 'ParseRGB', 11: 'ParseRGBA'}[base]
        elif base not in (5, 6) and not 12 <= base < 12 + len(spec.enums):
            label = 'Unknown stl::vector arg type' if append else 'Unknown arg type'
            return 4, f'{label} {base}'
        if base >= 12 and base - 12 < len(spec.enums):
            enum = spec.enums[base - 12]
            key = ascii_key(value)
            matches = [v for k, v in enum.help_values + enum.values if ascii_key(k) == key]
            if not matches:
                return 6, f"Unknown enum '{value}' of type {enum.name}"
            converted = matches[0]
            if info.destination is None and not append:
                raise ArgScriptError('ASC007', 'Scalar enum requires destination (host guard)')
        elif info.destination is None:
            return 0, 'no error'  # Native scalar/array conversion is skipped.
        elif route is not None:
            if route in converters:
                try:
                    converted = converters[route](value)
                except ArgScriptError as error:
                    error.argv_index = index
                    error.conversion_route = route
                    error.partial_spec_state = dict(flags=flags, writes=tuple(writes),
                                                    cursor=index, unresolved=tuple(unresolved))
                    raise
            else:
                converted = ConversionRequest(route, value, base)
                unresolved.append(converted)
        else:
            converted = value
        if info.destination is not None:
            writes.append(dict(destination=info.destination, value=converted,
                               append=append, argvIndex=index))
        return 0, 'no error'

    def argument(info, value, index, repeat=False):
        if repeat:
            # Direct ParseVectorArgument does not set mFlagToSet.
            return convert(info, value, True, index)
        flag(info.flag)
        if info.type_id & VECTOR:
            tokens, errors = split_arguments(value, list(range(len(value))), value, 0,
                                             '<vector-argument>', 1)
            if errors:
                error = errors[0]
                raise ArgScriptError(error['code'], error['message'])
            # NumArguments/operator[] enumerate the full storage, including
            # tokens classified as options by this nested split.
            for token in tokens:
                code, message = convert(info, token['value'], True, index)
                if code:
                    return code, message
            return 0, 'no error'
        return convert(info, value, False, index)

    def option_arguments(infos, cursor):
        slot, repeated = 0, False
        while cursor < len(argv) and slot < len(infos):
            if is_option(argv[cursor]):
                break
            repeat = slot == len(infos) - 1 and bool(infos[slot].type_id & REPEAT)
            code, message = argument(infos[slot], argv[cursor], cursor, repeat)
            if code:
                return code, message, cursor
            if repeat:
                repeated = True
            else:
                slot += 1
            cursor += 1
        if repeated or slot >= len(infos) or not infos[slot].dependent:
            return 0, 'no error', cursor
        needed = 0
        while slot + needed < len(infos) and infos[slot + needed].dependent:
            needed += 1
        return 2, f'Not enough arguments: expecting at least {needed} more', cursor

    cursor, slot = 1, 0
    while cursor < len(argv):
        value = argv[cursor]
        if is_option(value):
            name = value[1:]
            key = ascii_key(name)
            cursor += 1
            if spec.respect_help and key in ('h', 'help'):
                # Native renders CreateHelpString here. Rendering is not modeled.
                return result(1, 'help requested (rendering deferred)', cursor)
            option = next((o for o in spec.options if ascii_key(o.name) == key), None)
            if option is None:
                return result(5, f"Unknown option '{name}'", cursor)
            flag(option.flag)
            code, message, cursor = option_arguments(option.arguments, cursor)
            if code:
                return result(code, message + ' in -' + name, cursor)
        else:
            if slot >= len(spec.arguments):
                return result(3, f'Too many default arguments (expecting at most {len(spec.arguments)})\n', cursor)
            info = spec.arguments[slot]
            repeat = slot == len(spec.arguments) - 1 and bool(info.type_id & REPEAT)
            code, message = argument(info, value, cursor, repeat)
            if code:
                return result(code, message, cursor)
            if not repeat:
                slot += 1
            cursor += 1
    if slot < len(spec.arguments):
        info = spec.arguments[slot]
        if info.dependent and not info.type_id & REPEAT:
            return result(2, 'Not enough arguments', cursor)
    return result(0, 'no error', cursor)


def parse_invocation(spec, invocation, **kwargs):
    """AsArgV + MarkAllOptionsHandled wrapper, preserving raw native status."""
    invocation['allOptionsHandled'] = True  # Native marks before parsing, even on error.
    values = [token['value'] for token in invocation['argv']]
    try:
        parsed = parse_argv(spec, values, **kwargs)
    except ArgScriptError as error:
        invocation['argumentSpecFailure'] = dict(
            code=error.code, message=str(error), route=getattr(error, 'conversion_route', None),
            argumentIndex=getattr(error, 'argv_index', None),
            partial=getattr(error, 'partial_spec_state', None))
        raise
    invocation['argumentSpecResult'] = parsed
    return parsed


def checked_command(spec, execute, **kwargs):
    """Opt-in host adapter: stop before consumer on errors or deferred values.

    Native Parse(cArguments*) throws on nonzero status, including help. Missing
    converters are a separate research-host failure, not native bad syntax.
    """
    def checked(parser, invocation):
        try:
            parsed = parse_invocation(spec, invocation, **kwargs)
        except ArgScriptError as error:
            if hasattr(error, 'argv_index'):
                token = invocation['argv'][error.argv_index]
                error.argument_relative_span = dict(start=error.start, end=error.end)
                refs = token.get('sourceReferences', [])
                if refs:
                    error.start, error.end = refs[0]['start'], refs[-1]['end']
                invocation['argumentSpecFailure']['argumentRelativeSpan'] = error.argument_relative_span
            raise
        if parsed.native_code or parsed.unresolved:
            index = parsed.cursor - 1 if parsed.native_code in (1, 5) else parsed.cursor
            if not parsed.native_code:
                index = next(write['argvIndex'] for write in parsed.writes
                             if isinstance(write['value'], ConversionRequest))
            tokens = invocation['argv']
            references = tokens[index].get('sourceReferences', []) if index < len(tokens) else []
            origin = invocation.get('origin') or {}
            eof = origin.get('start', 0) + len(origin.get('raw', '').rstrip('\r\n'))
            start = references[0]['start'] if references else eof
            end = references[-1]['end'] if references else eof
            code = 'ASC008' if parsed.native_code else 'ASC009'
            message = parsed.message if parsed.native_code else 'Native conversion implementation required'
            error = ArgScriptError(code, message, start, end)
            error.native_code = parsed.native_code
            error.spec_result = parsed
            raise error
        execute(parser, invocation, parsed)
    return checked
