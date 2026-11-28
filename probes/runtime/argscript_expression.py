"""Native-derived integer and boolean recursive descent, never Python eval.

Floating point, native registered callbacks and ambiguous decompiler branches
remain unsupported. Source contracts and limits are documented in docs/re.
"""
from functools import wraps

from argscript_numbers import SPACE, int32, raw_integer_converters, read_integer
from argscript_variables import ArgScriptError


def bounded(function):
    @wraps(function)
    def checked(self, *args):
        self.depth += 1
        self.steps += 1
        try:
            if self.depth > 96 or self.steps > 10000:
                self.fail('ASX099', 'Expression complexity limit (host guard)')
            return function(self, *args)
        finally:
            self.depth -= 1
    return checked


class Expression:
    def __init__(self, text):
        if not text.isascii() or '\0' in text:
            raise ArgScriptError('ASX001', 'Non-ASCII or embedded-NUL expression is unsupported')
        self.text, self.cursor, self.depth, self.steps = text, 0, 0, 0

    def fail(self, code, message):
        raise ArgScriptError(code, message, self.cursor,
                             min(self.cursor + 1, len(self.text)))

    def peek(self):
        return self.text[self.cursor:self.cursor + 1]

    def space(self):
        while self.peek() and self.peek() in SPACE:
            self.cursor += 1

    def expect(self, char):
        self.space()
        if self.peek() != char:
            self.fail('ASX002', f"Expected '{char}'")
        self.cursor += 1

    def literal(self, value):
        self.space()
        if self.text.startswith(value, self.cursor):
            self.cursor += len(value)
            return True
        return False

    def identifier(self):
        self.space()
        start = self.cursor
        if self.peek().isalpha():
            self.cursor += 1
            while self.peek() and (self.peek().isalnum() or self.peek() == '_'):
                self.cursor += 1
        return self.text[start:self.cursor]

    @bounded
    def integer(self, flags=0):
        value = self.term(flags)
        while True:
            self.space()
            op = self.peek()
            if op not in ('+', '-'):
                return value
            self.cursor += 1
            rhs = self.term(flags)
            value = int32(value + rhs if op == '+' else value - rhs)

    @bounded
    def term(self, flags):
        value = self.factor(flags)
        while True:
            self.space()
            op = self.peek()
            if op not in ('*', '/', '%'):
                return value
            self.cursor += 1
            rhs = self.factor(flags)
            if op == '*':
                value = int32(value * rhs)
            else:
                if rhs == 0 or (value == -0x80000000 and rhs == -1):
                    self.fail('ASX098', 'Native integer division fault (host guard)')
                quotient = abs(value) // abs(rhs)
                if (value < 0) != (rhs < 0):
                    quotient = -quotient
                value = quotient if op == '/' else value - quotient * rhs

    @bounded
    def factor(self, flags):
        value = self.signed(flags)
        self.space()
        if self.peek() != '^':
            return value
        self.cursor += 1
        exponent = self.factor(flags)
        result = 1
        while exponent > 0:
            if exponent & 1:
                result = int32(result * value)
            exponent >>= 1
            value = int32(value * value)
        return result

    @bounded
    def signed(self, flags):
        self.space()
        while self.peek() == '+':
            self.cursor += 1
            self.space()
        if self.peek() == '-':
            self.cursor += 1
            return int32(-self.signed(flags))
        return self.number(flags)

    @bounded
    def number(self, flags):
        self.space()
        if self.peek().isdigit():
            # Native skips leading zeros only while followed by another digit.
            while self.peek() == '0' and self.text[self.cursor + 1:self.cursor + 2].isdigit():
                self.cursor += 1
            parsed = read_integer(self.text, self.cursor, base=0, unsigned=True)
            self.cursor = parsed.end
            if parsed.bits > (0xffffffff if flags & 1 else 0x7fffffff):
                self.fail('ASX003', 'Out of range')
            return int32(parsed.bits)
        if self.peek() == '(':
            return self.parentheses(flags)
        name = self.identifier()
        if not name:
            self.fail('ASX004', 'Bad integer expression')
        # Compare uses prefix matching, but several int constant call sites
        # have reversed decompiler bounds. Do not guess those branches.
        if 'abs'.startswith(name):
            return int32(abs(self.parentheses(flags)))
        if 'sqr'.startswith(name):
            value = self.parentheses(flags)
            return int32(value * value)
        self.fail('ASX005', f"Integer function/constant branch not recovered: '{name}'")

    @bounded
    def parentheses(self, flags):
        self.expect('(')
        value = self.boolean() if flags & 2 else self.integer(flags)
        self.expect(')')
        return value

    @bounded
    def boolean(self):
        value = self.bool_term()
        while self.literal('or'):
            rhs = self.bool_term()  # Native evaluates RHS even when LHS is true.
            if rhs != 0 or value != 0:
                value = 1
        return value

    @bounded
    def bool_term(self):
        value = self.bool_factor()
        while self.literal('and'):
            rhs = self.bool_factor()
            value = int(rhs != 0 and value != 0)
        return value

    @bounded
    def bool_factor(self):
        if self.literal('!') or self.literal('not'):
            return int(self.bool_factor() == 0)
        return self.relation()

    @bounded
    def relation(self):
        value = self.bool_atom()
        self.space()
        op = self.peek()
        if op not in ('!', '<', '=', '>'):
            return value
        self.cursor += 1
        equal = self.peek() == '='
        if equal:
            self.cursor += 1
        elif op in ('!', '='):
            self.fail('ASX006', f'Illegal operator {op}{self.peek()}')
        rhs = self.relation()  # Native relation chaining is right associative.
        if op == '!':
            return int(value != rhs)
        if op == '=':
            return int(value == rhs)
        if op == '<':
            return int(value <= rhs if equal else value < rhs)
        return int(value >= rhs if equal else value > rhs)

    @bounded
    def bool_atom(self):
        self.space()
        start = self.cursor
        name = self.identifier()
        if name:
            for constant, value in (('true', 1), ('on', 1), ('false', 0), ('off', 0)):
                if constant.startswith(name):
                    return value
        self.cursor = start
        return self.integer(2)


def parse_expression(text, kind='int'):
    if kind not in ('int', 'uint', 'bool'):
        raise ArgScriptError('ASX007', 'Expression kind is not implemented')
    reader = Expression(text)
    if not text:
        reader.fail('ASX008', 'empty expression')
    value = reader.boolean() if kind == 'bool' else reader.integer(int(kind == 'uint'))
    if reader.cursor != len(text):
        label = 'bool' if kind == 'bool' else 'integer'
        reader.fail('ASX009', f'Garbage at end of {label} expression')
    return bool(value) if kind == 'bool' else value & 0xffffffff if kind == 'uint' else value


def integer_expression_converters():
    return {**raw_integer_converters(),
            'ParseInt': lambda text: parse_expression(text),
            'ParseBool': lambda text: parse_expression(text, 'bool')}
