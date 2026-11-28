"""Native expression registration + selected parser meta extensions.

Opt-in, separate from the established integer/real converter entry points.
Formatting and floating arithmetic retain the documented portable-model limits.
"""
from dataclasses import dataclass
import math

from argscript_expression import Expression, bounded
from argscript_numbers import int32
from argscript_preprocessor import main_values
from argscript_real import RealExpression, float32
from argscript_variables import ArgScriptError, ascii_key, set_variable_raw, variable


@dataclass(frozen=True)
class ExprFunction:
    integer: object
    real: object = None
    boolean: object = None


@bounded
def eval_string(reader):
    """EvalString delimiters differ from SplitIntoArguments (ASCII space only)."""
    reader.space()
    if reader.peek() == '(':
        reader.cursor += 1
        value = eval_string(reader)
        reader.expect(')')
        return value
    quoted = reader.peek() == '"'
    if quoted:
        reader.cursor += 1
    start = reader.cursor
    while reader.peek() and (reader.peek() != '"' if quoted else reader.peek() not in (')', ' ', ',')):
        reader.cursor += 1
    value = reader.text[start:reader.cursor]
    if quoted:
        reader.expect('"')
    return value


def wildcard_match(text, pattern):
    """ASCII MatchPattern wildcard semantics without regex/fnmatch expansion."""
    if not text.isascii() or not pattern.isascii():
        raise ArgScriptError('ASF001', 'Non-ASCII wildcard compatibility is unresolved')
    text, pattern = text.lower(), pattern.lower()
    if (len(text) + 1) * (len(pattern) + 1) > 1000000:
        raise ArgScriptError('ASF007', 'Wildcard work limit (host guard)')
    # Translate native branch order: only a single final '*' succeeds before
    # checking empty input. Collapsing '**' would change observable behavior.
    next_row = [False] * (len(pattern) + 1)
    for i in range(len(text), -1, -1):
        current = [False] * (len(pattern) + 1)
        current[-1] = i == len(text)
        for j in range(len(pattern) - 1, -1, -1):
            char = pattern[j]
            if char == '*' and j == len(pattern) - 1:
                current[j] = True
            elif i == len(text):
                current[j] = False
            elif char == '*':
                current[j] = current[j + 1] or next_row[j]
            else:
                current[j] = (char == '?' or char == text[i]) and next_row[j + 1]
        next_row = current
    return next_row[0]


class RegisteredExpression(Expression):
    def __init__(self, text, environment):
        super().__init__(text)
        self.environment = environment

    @bounded
    def number(self, flags):
        start = self.cursor
        try:
            return super().number(flags)
        except ArgScriptError as error:
            if error.code != 'ASX005':
                raise
            self.cursor = start
            name = self.identifier()
            # Native built-ins precede the registry. Ambiguous constant branches
            # stay unresolved and cannot be bypassed with a host registration.
            if any(value.startswith(name) for value in ('abs', 'floor', 'ceil', 'round', 'sqr',
                                                       'true', 'on', 'false', 'off')):
                raise error
            function = self.environment.functions.get(name)
            if function is None:
                raise error
            return int32(function.integer(self))

    @bounded
    def bool_atom(self):
        self.space()
        start = self.cursor
        name = self.identifier()
        if name:
            for constant, value in (('true', 1), ('on', 1), ('false', 0), ('off', 0)):
                if constant.startswith(name):
                    return value
            function = self.environment.functions.get(name)
            if function is not None:
                callback = function.boolean or function.integer
                return int(callback(self) != 0)
        self.cursor = start
        return self.integer(2)


class RegisteredReal(RealExpression):
    def __init__(self, text, environment):
        super().__init__(text)
        self.environment = environment

    @bounded
    def real_number(self):
        start = self.cursor
        try:
            return super().real_number()
        except ArgScriptError as error:
            if error.code != 'ASR003':
                raise
            self.cursor = start
            name = self.identifier()
            native = ('pi', 'e', 'sqrt', 'exp', 'log', 'abs', 'sign', 'sin', 'cos', 'tan',
                      'asin', 'acos', 'atan', 'sind', 'cosd', 'tand', 'dasin', 'dacos',
                      'datan', 'floor', 'ceil', 'round', 'wrap', 'fmod', 'frac',
                      'remainder', 'sqr', 'pow', 'atan2', 'datan2')
            if any(value.startswith(name) for value in native):
                raise error
            function = self.environment.functions.get(name)
            if function is None:
                raise error
            if function.real is None:
                self.fail('ASF002', 'boolean function')
            # EvalRealNumber stores the registered callback result in float y.
            return float32(function.real(self))


class FunctionEnvironment:
    def __init__(self):
        self.functions = {}

    def register(self, name, function):
        # Native RegisterFunction retains the exact key; no lowercasing.
        if not name.isascii():
            raise ArgScriptError('ASF003', 'Non-ASCII function registration is unresolved')
        if function is None:
            self.functions.pop(name, None)
        else:
            self.functions[name] = function

    def parse(self, text, kind='int'):
        if kind not in ('int', 'uint', 'bool', 'real'):
            raise ArgScriptError('ASF004', 'Unsupported expression kind (host guard)')
        reader = RegisteredReal(text, self) if kind == 'real' else RegisteredExpression(text, self)
        if not text:
            reader.fail('ASX008', 'empty expression')
        value = reader.real() if kind == 'real' else (
            reader.boolean() if kind == 'bool' else reader.integer(int(kind == 'uint')))
        if reader.cursor != len(text):
            label = 'real' if kind == 'real' else 'bool' if kind == 'bool' else 'integer'
            reader.fail('ASF005', f'Garbage at end of {label} expression')
        return bool(value) if kind == 'bool' else value & 0xffffffff if kind == 'uint' else value


def string_call(reader, count):
    reader.expect('(')
    values = []
    for index in range(count):
        if index:
            reader.expect(',')
        values.append(eval_string(reader))
    reader.expect(')')
    return values


def install_meta_extensions(host, versions=None):
    """Explicit installation; existence functions read the live host state."""
    environment = FunctionEnvironment()
    if versions is not None and len(versions) != 2:
        raise ArgScriptError('ASF008', 'Both version bounds must be supplied explicitly')
    environment.register('match', ExprFunction(lambda r: int(wildcard_match(*string_call(r, 2)))))
    def equal(reader):
        lhs, rhs = string_call(reader, 2)
        return int(ascii_key(lhs) == ascii_key(rhs))
    environment.register('eq', ExprFunction(equal))
    environment.register('varExists', ExprFunction(lambda r: int(
        variable(host.parser.variables, string_call(r, 1)[0]) is not None)))
    environment.register('defExists', ExprFunction(lambda r: int(
        ascii_key(string_call(r, 1)[0]) in host.macros)))
    environment.register('commandExists', ExprFunction(lambda r: int(
        ascii_key(string_call(r, 1)[0]) in host.parser.commands)))
    if versions is not None:
        for name, value in zip(('minVersion', 'maxVersion'), versions):
            def callback(reader, value=value):
                string_call(reader, 0)
                return int32(value)
            environment.register(name, ExprFunction(callback))
    host.boolean = lambda text: environment.parse(text, 'bool')

    def setter(kind):
        def execute(parser, invocation):
            name, text = main_values(invocation, 2)
            try:
                value = environment.parse(text, kind)
                if kind == 'real':
                    if not math.isfinite(value):
                        raise ArgScriptError('ASF006', 'Nonfinite native printf spelling is unresolved')
                    stored = format(value, '.6f')[:31]  # %f, char[32], portable formatting model.
                else:
                    stored = str(int(value))
            except ArgScriptError as error:
                error.expression_relative_span = dict(start=error.start, end=error.end)
                refs = invocation['arguments']['main'][1].get('sourceReferences', [])
                if refs:
                    error.start, error.end = refs[0]['start'], refs[-1]['end']
                raise
            parser.variables = set_variable_raw(parser.variables, name, stored)
        return execute
    for name, kind in (('seti', 'int'), ('setu', 'uint'), ('setb', 'bool'), ('setf', 'real')):
        host.parser.register(name, setter(kind))
    return environment
