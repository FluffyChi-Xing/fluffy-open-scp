"""Offline native-derived command pipeline with explicit host callbacks.

No built-in Eco commands, macro/if semantics, filesystem access or game hooks.
Callbacks implement execute(parser, invocation); meta objects implement
parse(parser, clean_text, origin)->bool and end(parser, error_occurred).
"""
import re
from dataclasses import replace

from argscript_syntax import argument_view, split_arguments, strip_line
from argscript_variables import ArgScriptError, Variables, ascii_key, substitute


class Parser:
    def __init__(self, variables=None):
        self.variables = Variables() if variables is None else variables
        self.commands = {}
        self.blocks = []
        self.meta = []
        self.comment_stack = []
        self.total_lines = 0
        self.trace = []

    def register(self, name, execute):
        # Native RegisterCommand replaces the existing binding. Reference
        # counting and OnRegister/state pointers are outside this Python host.
        self.commands[ascii_key(name)] = execute

    def command(self, name):
        key = ascii_key(name)
        for commands, _ in reversed(self.blocks):
            if key in commands:
                return commands[key]
        return self.commands.get(key)

    def start_block(self, commands, end=None):
        self.blocks.append(({ascii_key(k): v for k, v in commands.items()}, end))

    def end_block(self, error=False):
        if not self.blocks:
            raise ArgScriptError('ASI004', 'Block stack underflow (host guard)')
        _, callback = self.blocks[-1]
        if callback is not None:
            callback(self, error)
        self.blocks.pop()

    def start_meta(self, handler, end_token):
        self.meta.append((handler, ascii_key(end_token)))

    def parse_arguments(self, tokens, origin=None):
        """Native ParseArguments bypasses comments, meta and substitution."""
        args = argument_view(tokens)
        if not args['main']:
            return None
        first = args['main'][0]
        execute = self.command(first['value'])
        if execute is None:
            span = first.get('generatedSpan', first.get('span'))
            raise ArgScriptError('ASP001', 'Unknown command', span['start'], span['end'])
        args['main'] = args['main'][1:]  # cArguments::Chomp only removes main arg 0.
        # AsArgV exposes the original storage, including the command even after
        # Chomp. cArgumentSpec reparses these values, not argument_view.
        invocation = dict(command=first['value'], arguments=args, origin=origin,
                          argv=list(tokens))
        try:
            execute(self, invocation)
        except ArgScriptError as error:
            if not hasattr(error, 'invocation'):
                error.invocation = invocation
            error.call_invocations = getattr(error, 'call_invocations', []) + [invocation]
            raise
        return invocation

    def parse_line(self, raw, file='<input>', line=1, offset=0):
        self.total_lines += 1
        body = raw.rstrip('\r\n')
        clean, positions, comments, errors = strip_line(
            body, offset, self.comment_stack, file, line)
        origin = dict(file=file, line=line, start=offset, raw=raw)
        trace = dict(origin=origin, clean=clean, comments=comments)
        self.trace.append(trace)
        if errors:
            error = errors[0]
            raise ArgScriptError(error['code'], error['message'],
                                 offset + error['column'] - 1)
        # StripCommentsAndWhiteSpace trims for whole-line meta end matching.
        left = len(clean) - len(clean.lstrip())
        clean = clean.strip()
        positions = positions[left:left + len(clean)]
        trace['clean'] = clean
        if not clean:
            trace['phase'] = 'empty'
            return trace
        if self.meta:
            trace['phase'] = 'meta'
            handler, end_token = self.meta[-1]
            # Native compares the entire clean line before variable substitution.
            if clean.isascii() and clean.lower() == end_token:
                handler.end(self, False)
                self.meta.pop()
                trace['phase'] = 'meta-end'
                return trace
            if handler.parse(self, clean, origin):
                trace['phase'] = 'meta-consumed'
                return trace
        trace['phase'] = 'substitution'
        try:
            expanded = substitute(self.variables, clean, bracket=True)
        except ArgScriptError as error:
            # Translate input-relative failure to an original source offset.
            start = positions[min(error.start, len(positions) - 1)]
            end = positions[min(max(error.end - 1, error.start), len(positions) - 1)] + 1
            raise ArgScriptError(error.code, str(error), start, end) from error
        trace['expanded'] = expanded
        trace['phase'] = 'arguments'
        value = expanded['text']
        tokens, errors = split_arguments(value, list(range(len(value))), value, 0, file, line)
        # Keep generated spans explicitly separate from original spans.
        for token in tokens:
            token['generatedSpan'] = token.pop('span')
            token['generatedSegments'] = token.pop('sourceSegments')
            token['sourceReferences'] = self.source_references(
                token['generatedSpan'], expanded['pieces'], positions)
        if errors:
            error = errors[0]
            refs = self.source_references(
                dict(start=error['column'] - 1, end=error['column']),
                expanded['pieces'], positions)
            start = refs[0]['start'] if refs else offset
            raise ArgScriptError(error['code'], error['message'], start)
        trace['phase'] = 'dispatch'
        try:
            trace['invocation'] = self.parse_arguments(tokens, origin)
        except ArgScriptError as error:
            if hasattr(error, 'invocation'):
                trace['invocation'] = error.call_invocations[-1]
            if error.code == 'ASP001' and not hasattr(error, 'file'):
                refs = self.source_references(dict(start=error.start, end=error.end),
                                              expanded['pieces'], positions)
                if refs:
                    raise ArgScriptError(error.code, str(error), refs[0]['start'],
                                         refs[-1]['end']) from error
            raise
        trace['phase'] = 'executed'
        return trace

    @staticmethod
    def source_references(span, pieces, positions):
        refs = []
        for piece in pieces:
            lo, hi = max(span['start'], piece['start']), min(span['end'], piece['end'])
            if lo >= hi:
                continue
            if piece['kind'] == 'literal':
                start = piece['inputStart'] + lo - piece['start']
                end = start + hi - lo
            else:
                start, end = piece['inputStart'], piece['inputEnd']
            # Comments may have split even a literal token into disjoint spans.
            for index in range(start, end):
                position = positions[index]
                if refs and refs[-1]['end'] == position and refs[-1]['kind'] == piece['kind']:
                    refs[-1]['end'] += 1
                else:
                    refs.append(dict(start=position, end=position + 1, kind=piece['kind']))
        return refs

    def parse_stream(self, text, file='<input>'):
        """Fail fast, preserving partial trace; caller controls error unwinding."""
        block_depth, meta_depth = len(self.blocks), len(self.meta)
        offset = 0
        for number, match in enumerate(re.finditer(r'[^\r\n]*(?:\r\n|\r|\n|$)', text), 1):
            raw = match.group()
            if raw:
                trace_index = len(self.trace)
                try:
                    self.parse_line(raw, file, number, offset)
                except ArgScriptError as error:
                    trace = self.trace[trace_index]
                    if not hasattr(error, 'file'):
                        error.file, error.line = file, number
                        error.phase = trace.get('phase', 'comments')
                    trace['failure'] = dict(code=error.code, message=str(error),
                                                   start=error.start, end=error.end,
                                                   phase=error.phase, file=error.file, line=error.line)
                    raise
                offset += len(raw)
        if len(self.blocks) != block_depth:
            raise ArgScriptError('ASP002', 'Unbalanced block command', offset, offset)
        if len(self.meta) != meta_depth:
            raise ArgScriptError('ASP003', 'Unbalanced meta command', offset, offset)
        return self.trace

    def finish(self, error=False):
        """ParseEnd reset/unwind subset; does not clear variable bindings."""
        if error:
            while self.meta:
                self.meta[-1][0].end(self, True)
                self.meta.pop()
            while self.blocks:
                self.end_block(True)
        self.variables = replace(self.variables, scope='', offsets=())
        self.comment_stack.clear()
        return self.total_lines
