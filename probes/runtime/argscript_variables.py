"""Pure ASCII contracts from development EA::ArgScript::cParser.

This is parser infrastructure, not the ER2 compiler. See the evidence ledger in
docs/re/argscript-parser-infrastructure-20261128.md. No expression evaluation.
"""
from dataclasses import dataclass, replace


class ArgScriptError(ValueError):
    def __init__(self, code, message, start=0, end=None):
        super().__init__(message)
        self.code = code
        self.start = start
        self.end = start + 1 if end is None else end


def ascii_key(name):
    if not name.isascii():
        raise ArgScriptError('ASI001', 'Non-ASCII identifier compatibility is unresolved')
    return name.lower()


@dataclass(frozen=True)
class Variables:
    scope: str = ''
    offsets: tuple = ()
    local: tuple = ()
    global_values: tuple = ()
    external_values: tuple = ()


def push_namespace(state, name):
    ascii_key(name)
    return replace(state, scope=state.scope + name + ':',
                   offsets=state.offsets + (len(state.scope),))


def pop_namespace(state):
    # Native API assumes a nonempty stack. This explicit error is a host guard.
    if not state.offsets:
        raise ArgScriptError('ASI002', 'Namespace stack underflow (host guard)')
    return replace(state, scope=state.scope[:state.offsets[-1]], offsets=state.offsets[:-1])


def variable(state, name):
    key = ascii_key(name)
    local = dict(state.local)
    if name.startswith(':'):
        candidates = [key[1:]]
    else:
        candidates = [ascii_key(state.scope) + key]
        candidates.extend(ascii_key(state.scope[:offset]) + key
                          for offset in reversed(state.offsets))
    for candidate in candidates:
        if candidate in local:
            return local[candidate]
    # SetGlobalVariable and this fallback do not apply MakeCaseInsensitive.
    for bindings in (state.global_values, state.external_values):
        values = dict(bindings)
        if name in values:
            return values[name]
    return None


def set_variable_raw(state, name, value):
    values = dict(state.local)
    # Unlike lookup, the native setter always prefixes mScope, even for ':x'.
    values[ascii_key(state.scope + name)] = value
    return replace(state, local=tuple(values.items()))


def set_global_variable(state, name, value):
    values = dict(state.global_values)
    if value is None:
        values.pop(name, None)
    else:
        values[name] = value
    return replace(state, global_values=tuple(values.items()))


def substitute(state, text, bracket=True):
    """Single pass over original input; inserted values are never rescanned.

    Pieces map generated intervals to input intervals. A substituted piece maps
    to the whole reference, not a fabricated character location in its value.
    """
    if '$' in text:
        if not text.isascii():
            raise ArgScriptError('ASI003', 'Non-ASCII substitution encoding is unresolved')
        if len(text) > 2047:
            raise ArgScriptError('ASV001', 'buffer overflow', 0, len(text))
    output, pieces = [], []
    generated = 0

    def emit(value, start, end, name=None):
        nonlocal generated
        output.append(value)
        pieces.append(dict(start=generated, end=generated + len(value),
                           inputStart=start, inputEnd=end,
                           kind='variable' if name is not None else 'literal', name=name))
        generated += len(value)

    cursor = 0
    while cursor < len(text):
        dollar = text.find('$', cursor)
        if dollar < 0:
            emit(text[cursor:], cursor, len(text))
            break
        if cursor < dollar:
            emit(text[cursor:dollar], cursor, dollar)
        start = dollar + 1
        braces = start < len(text) and text[start] == '{'
        if braces:
            start += 1
        if start >= len(text):
            raise ArgScriptError('ASV002', 'missing variable name', dollar, len(text))
        if not (text[start].isalpha() or text[start] in '_:'):
            raise ArgScriptError('ASV003', 'bad variable name', start)
        end = start + 1
        while end < len(text) and (text[end].isalnum() or text[end] in '_:'):
            end += 1
        if braces and (end >= len(text) or text[end] != '}'):
            raise ArgScriptError('ASV004', "missing '}'", dollar, end)
        name = text[start:end]
        value = variable(state, name)
        if value is None:
            raise ArgScriptError('ASV005', f"Unknown variable: '{name}'", dollar,
                                 end + int(braces))
        if not braces and bracket:
            value = '(' + value + ')'
        cursor = end + int(braces)
        emit(value, dollar, cursor, name)
    return dict(text=''.join(output), pieces=pieces)


def set_variable(state, name, value):
    return set_variable_raw(state, name, substitute(state, value, bracket=False)['text'])
