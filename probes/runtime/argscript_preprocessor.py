"""Native-derived macro/condition host; include I/O is an explicit source map.

No filesystem or game access. Raw directory-root detection and silent wrappers
with incomplete exception decompilation are intentionally not guessed.
"""
from dataclasses import dataclass
import hashlib

from argscript_expression import parse_expression
from argscript_parser import Parser
from argscript_syntax import argument_view, split_arguments
from argscript_variables import (ArgScriptError, ascii_key, pop_namespace,
                                push_namespace, set_variable_raw, substitute)


def split_values(text):
    tokens, errors = split_arguments(text, list(range(len(text))), text, 0, '<meta-argument>', 1)
    if errors:
        error = errors[0]
        raise ArgScriptError(error['code'], error['message'], error['column'] - 1)
    return tokens


def main_values(invocation, minimum, maximum=None):
    maximum = minimum if maximum is None else maximum
    values = [t['value'] for t in invocation['arguments']['main']]
    if len(values) < minimum:
        raise ArgScriptError('ASM001', f'Expecting at least {minimum} arguments')
    if len(values) > maximum:
        raise ArgScriptError('ASM002', f'Expecting at most {maximum} arguments')
    return values


def substitute_parameters(parameters, values, text):
    if len(parameters) != len(values):
        raise ArgScriptError('ASM003', 'Wrong number of arguments')
    if '&' in text and not text.isascii():
        raise ArgScriptError('ASM004', 'Non-ASCII parameter substitution is unresolved')
    pieces, output, cursor, generated = [], [], 0, 0

    def emit(value, start, end, name=None):
        nonlocal generated
        output.append(value)
        pieces.append(dict(start=generated, end=generated + len(value),
                           inputStart=start, inputEnd=end, parameter=name))
        generated += len(value)

    while cursor < len(text):
        amp = text.find('&', cursor)
        if amp < 0:
            emit(text[cursor:], cursor, len(text)); break
        if cursor < amp:
            emit(text[cursor:amp], cursor, amp)
        start = amp + 1
        braces = text[start:start + 1] == '{'
        start += int(braces)
        if start >= len(text):
            raise ArgScriptError('ASM005', 'missing parameter name', amp, len(text))
        if not (text[start].isalpha() or text[start] == '_'):
            raise ArgScriptError('ASM006', 'bad parameter name', start)
        end = start + 1
        while end < len(text) and (text[end].isalnum() or text[end] == '_'):
            end += 1
        if braces and text[end:end + 1] != '}':
            raise ArgScriptError('ASM007', "missing '}'", amp, end)
        name = text[start:end]
        if name not in parameters:
            raise ArgScriptError('ASM008', 'Unknown parameter: ' + name, amp, end + int(braces))
        value = values[parameters.index(name)]  # Native uses first exact match.
        if not braces:
            value = '(' + value + ')'
        cursor = end + int(braces)
        emit(value, amp, cursor, name)
    return dict(text=''.join(output), pieces=pieces)


@dataclass(frozen=True)
class Macro:
    name: str
    parameters: tuple
    body: tuple
    origin: dict


class Definition:
    def __init__(self, host, name, parameters, origin):
        self.host, self.name, self.parameters, self.origin = host, name, parameters, origin
        self.body = []

    def parse(self, parser, text, origin):
        self.body.append(dict(text=text, origin=dict(origin)))
        return True

    def end(self, parser, error):
        if not error:
            self.host.macros[ascii_key(self.name)] = Macro(
                self.name, self.parameters, tuple(self.body), self.origin)


class Conditions:
    def __init__(self, host):
        self.host, self.frames = host, []

    def execute(self, parser, invocation):
        condition = self.evaluate(invocation)
        parent_active = not self.frames or self.frames[-1]['active']
        self.frames.append(dict(active=condition and parent_active,
                                triggered=condition or not parent_active))
        parser.start_meta(self, 'endif')

    def evaluate(self, invocation):
        text = main_values(invocation, 1)[0]
        try:
            return self.host.boolean(text)
        except ArgScriptError as error:
            error.expression_relative_span = dict(start=error.start, end=error.end)
            refs = invocation['arguments']['main'][0].get('sourceReferences', [])
            origin = invocation.get('origin') or {}
            if refs:
                error.start, error.end = refs[0]['start'], refs[-1]['end']
            else:
                error.start = origin.get('start', 0)
                error.end = error.start + len(origin.get('raw', '').rstrip('\r\n'))
            raise

    def parse(self, parser, text, origin):
        frame = self.frames[-1]
        if text.startswith('if'):
            return False
        if text == 'else':
            frame['active'] = not frame['triggered']
            frame['triggered'] = True
            return True
        if text.startswith('elseif'):
            if frame['triggered']:
                frame['active'] = False
                return True
            try:
                expanded = substitute(parser.variables, text)['text']
                view = argument_view(split_values(expanded))
                view['main'] = view['main'][1:]
                condition = self.evaluate(dict(arguments=view, origin=origin))
            except ArgScriptError as error:
                # This meta branch has no full comment/substitution source map.
                # Attribute to the whole raw line rather than fake a column.
                error.start = origin['start']
                error.end = error.start + len(origin['raw'].rstrip('\r\n'))
                raise
            frame['active'] = frame['triggered'] = condition
            return True
        return not frame['active']

    def end(self, parser, error):
        if self.frames:
            self.frames.pop()


def parent_directory(path):
    last = max(path.rfind('/'), path.rfind('\\'))
    return './' if last < 0 else path[:last + 1]


class Preprocessor:
    def __init__(self, parser=None, sources=None, boolean=None):
        self.parser = Parser() if parser is None else parser
        self.sources = {} if sources is None else dict(sources)
        self.boolean = boolean or (lambda text: parse_expression(text, 'bool'))
        self.macros, self.expansions, self.files = {}, [], []
        self.base, self.directories, self.active_files = '', [], []
        self.expansion_depth, self.expansion_lines = 0, 0
        self.conditions = Conditions(self)
        commands = {'define': self.define, 'redefine': self.redefine, 'create': self.create,
                    'if': self.conditions.execute, 'namespace': self.namespace,
                    'end': self.end_block, 'set': self.set_text,
                    'include': self.include, 'sinclude': self.sinclude,
                    'pushDir': self.push_dir_command, 'popDir': self.pop_dir_command,
                    'undefine': self.undefine, 'sundefine': self.sundefine}
        for name, execute in commands.items():
            self.parser.register(name, execute)
        self.parser.register('screate', self.unsupported_screate)

    def unsupported_screate(self, parser, invocation):
        raise ArgScriptError('ASM019', 'screate exception suppression is not recovered')

    def define(self, parser, invocation, replace_existing=False):
        name, parameters = main_values(invocation, 2)
        if not replace_existing and ascii_key(name) in self.macros:
            raise ArgScriptError('ASM009', 'Already defined')
        params = tuple(t['value'] for t in split_values(parameters))
        parser.start_meta(Definition(self, name, params, dict(invocation['origin'] or {})), 'enddef')

    def redefine(self, parser, invocation):
        self.define(parser, invocation, True)

    def undefine(self, parser, invocation, silent=False):
        name = main_values(invocation, 1)[0]
        key = ascii_key(name)
        if key not in self.macros and not silent:
            raise ArgScriptError('ASM010', 'Unknown definition ' + name)
        self.macros.pop(key, None)

    def sundefine(self, parser, invocation):
        self.undefine(parser, invocation, True)

    def create(self, parser, invocation):
        name, argument = main_values(invocation, 2)
        values = tuple(t['value'] for t in split_values(argument))
        macro = self.macros.get(ascii_key(name))
        if macro is None:
            raise ArgScriptError('ASM011', f"Unknown definition: '{name}'")
        if len(values) != len(macro.parameters):
            raise ArgScriptError('ASM003', 'Wrong number of arguments')
        if self.expansion_depth >= 32:
            raise ArgScriptError('ASM099', 'Macro recursion limit (host guard)')
        self.expansion_depth += 1
        try:
            for body in macro.body:
                self.expansion_lines += 1
                if self.expansion_lines > 10000:
                    raise ArgScriptError('ASM099', 'Macro line budget exceeded (host guard)')
                frame = dict(macro=name, definition=macro.origin,
                             body=body['origin'], call=invocation['origin'])
                generated_file = f"<macro {name}@{body['origin']['file']}:{body['origin']['line']}>"
                try:
                    expanded = substitute_parameters(macro.parameters, values, body['text'])
                    frame['substitution'] = expanded
                    self.expansions.append(frame)
                    # Generated coordinates are explicit, never claimed to be
                    # character positions in the original definition text.
                    before = len(parser.trace)
                    try:
                        parser.parse_line(expanded['text'], generated_file,
                                          body['origin']['line'], 0)
                    finally:
                        if len(parser.trace) > before:
                            parser.trace[before]['macroExpansion'] = frame
                            parser.trace[before]['origin']['generated'] = True
                except ArgScriptError as error:
                    if not hasattr(error, 'file'):
                        error.file = generated_file
                        error.line = body['origin']['line']
                        error.phase = 'macro-replay'
                        error.generated = True
                    error.macro_frames = getattr(error, 'macro_frames', []) + [frame]
                    raise
        finally:
            self.expansion_depth -= 1

    def namespace(self, parser, invocation):
        # Native pushes first; this host validates first to avoid undefined
        # namespace-pop cleanup after a missing/invalid name.
        name = main_values(invocation, 1)[0]
        ascii_key(name)
        parser.start_block({}, lambda p, error: setattr(p, 'variables', pop_namespace(p.variables)))
        parser.variables = push_namespace(parser.variables, name)

    def end_block(self, parser, invocation):
        main_values(invocation, 0)
        if not parser.blocks:
            raise ArgScriptError('ASM012', 'Not inside a block')
        _, end = parser.blocks.pop()  # Native normal end pops before callback.
        if end is not None:
            end(parser, False)

    def set_text(self, parser, invocation):
        values = main_values(invocation, 0, 2)
        if len(values) != 2:
            raise ArgScriptError('ASM013', 'set inspection/output forms are not implemented')
        parser.variables = set_variable_raw(parser.variables, values[0], values[1])

    def push_dir(self, directory):
        self.directories.append(self.base)
        self.base = directory  # Native replaces; does not join or normalize.

    def pop_dir(self):
        if not self.directories:
            raise ArgScriptError('ASM014', 'Directory stack underflow (host guard)')
        self.base = self.directories.pop()

    def push_dir_command(self, parser, invocation):
        self.push_dir(main_values(invocation, 1)[0])

    def pop_dir_command(self, parser, invocation):
        main_values(invocation, 0)
        self.pop_dir()

    def include(self, parser, invocation, silent=False):
        path = main_values(invocation, 1)[0]
        if path.startswith(('/', '\\')):
            raise ArgScriptError('ASM015', 'Root/UNC include detection is not recovered')
        drive = len(path) >= 2 and path[0].isascii() and path[0].isalpha() and path[1] == ':'
        effective = path if drive else self.base + path
        if effective not in self.sources:
            if silent:
                return
            raise ArgScriptError('ASM016', "Couldn't open " + effective)
        self.parse_source(effective)

    def sinclude(self, parser, invocation):
        self.include(parser, invocation, True)

    def parse_source(self, path):
        if path in self.active_files or len(self.active_files) >= 32:
            raise ArgScriptError('ASM017', 'Include cycle/depth limit (host guard)')
        if path not in self.sources:
            raise ArgScriptError('ASM018', f"File not found: '{path}'")
        text = self.sources[path]
        self.files.append(dict(path=path, sha256=hashlib.sha256(text.encode('utf-8')).hexdigest()))
        self.active_files.append(path)
        self.push_dir(parent_directory(path))
        try:
            return self.parser.parse_stream(text, path)
        except ArgScriptError as error:
            if not hasattr(error, 'file'):
                error.file, error.line, error.phase = path, text.count('\n') + 1, 'stream-balance'
            if not hasattr(error, 'include_stack'):
                error.include_stack = list(self.active_files)
            raise
        finally:
            self.pop_dir()
            self.active_files.pop()
