"""Development-derived real conversion model with explicit float32 stores.

Host libm/x87 precision is not claimed bit-identical. No eval or native runtime.
Raw decimal reads and expression reads deliberately have different termination.
"""
from dataclasses import dataclass
import math
import struct

from argscript_expression import Expression, bounded, integer_expression_converters
from argscript_numbers import SPACE, int32
from argscript_variables import ArgScriptError


def float32(value):
    try:
        return struct.unpack('<f', struct.pack('<f', value))[0]
    except OverflowError:
        return math.copysign(math.inf, value)


def ieee_divide(lhs, rhs):
    if rhs == 0.0:
        if lhs == 0.0 or math.isnan(lhs):
            return math.nan
        return math.copysign(math.inf, math.copysign(1.0, lhs) * math.copysign(1.0, rhs))
    return lhs / rhs


def portable_pow(lhs, rhs):
    try:
        return math.pow(lhs, rhs)
    except OverflowError:
        negative = lhs < 0 and math.isfinite(rhs) and rhs.is_integer() and int(rhs) % 2
        return -math.inf if negative else math.inf
    except ValueError:
        if lhs == 0 and rhs < 0:
            negative = math.copysign(1.0, lhs) < 0 and rhs.is_integer() and int(rhs) % 2
            return -math.inf if negative else math.inf
        return math.nan


@dataclass(frozen=True)
class RealRead:
    value: float
    end: int


def read_real(text, start=0):
    """StrtodEnglish decimal accumulation order, including incomplete exponent."""
    if not text.isascii() or '\0' in text or not 0 <= start <= len(text):
        raise ArgScriptError('ASR001', 'Unsupported encoding or numeric cursor (host guard)')
    cursor, value = start, 0.0
    while cursor < len(text) and text[cursor] in SPACE:
        cursor += 1
    negative = text[cursor:cursor + 1] == '-'
    if text[cursor:cursor + 1] in ('+', '-'):
        cursor += 1
    while cursor < len(text) and text[cursor].isdigit():
        value = (ord(text[cursor]) - 48) + value * 10.0
        cursor += 1
    if text[cursor:cursor + 1] == '.':
        cursor += 1
        factor = 1.0
        while cursor < len(text) and text[cursor].isdigit():
            factor *= 0.1
            value = (ord(text[cursor]) - 48) * factor + value
            cursor += 1
    if text[cursor:cursor + 1] in ('e', 'E'):
        cursor += 1
        exponent_negative = text[cursor:cursor + 1] == '-'
        if text[cursor:cursor + 1] in ('+', '-'):
            cursor += 1
        exponent = 0
        while cursor < len(text) and text[cursor].isdigit():
            exponent = int32(exponent * 10 + ord(text[cursor]) - 48)
            cursor += 1
        power = portable_pow(10.0, float(exponent))
        if exponent_negative:
            power = ieee_divide(1.0, power)
        value *= power
    return RealRead(-value if negative else value, cursor)


def sse_remainder(lhs, rhs):
    """Translate the EvalRealTerm divss/magic-round/cmpnless instruction path."""
    quotient = float32(ieee_divide(lhs, rhs))
    magic = math.copysign(8388608.0, quotient)
    rounded = float32(float32(quotient + magic) - magic)
    delta = float32(rounded - quotient)
    threshold = math.copysign(0.0, quotient)
    correction = 1.0 if math.isnan(delta) or delta > threshold else 0.0
    floor_value = float32(rounded - correction)
    return float32(lhs - float32(floor_value * rhs))


class RealExpression(Expression):
    @bounded
    def real(self):
        value = self.real_term()
        while True:
            lhs = float32(value)  # fst m32, retains the prior x87 value on exit.
            self.space()
            op = self.peek()
            if op not in ('+', '-'):
                return value
            self.cursor += 1
            rhs = self.real_term()
            value = lhs + rhs if op == '+' else lhs - rhs

    @bounded
    def real_term(self):
        value = float32(self.real_factor())
        while True:
            self.space()
            op = self.peek()
            if op not in ('*', '/', '%'):
                return value
            self.cursor += 1
            rhs = self.real_factor()
            if op == '*':
                value = float32(value * rhs)
            elif op == '/':
                value = float32(ieee_divide(value, rhs))
            else:
                value = sse_remainder(value, float32(rhs))

    @bounded
    def real_factor(self):
        value = self.real_signed()
        self.space()
        if self.peek() == '^':
            self.cursor += 1
            return portable_pow(value, self.real_factor())
        return value

    @bounded
    def real_signed(self):
        self.space()
        while self.peek() == '+':
            self.cursor += 1
            self.space()
        if self.peek() == '-':
            self.cursor += 1
            return -self.real_signed()
        return self.real_number()

    def real_parentheses(self):
        self.expect('(')
        value = float32(self.real())
        self.expect(')')
        return value

    @bounded
    def real_number(self):
        self.space()
        if self.peek().isdigit() or self.peek() == '.':
            parsed = read_real(self.text, self.cursor)
            self.cursor = parsed.end
            return parsed.value
        if self.peek() == '(':
            return self.real_parentheses()
        name = self.identifier()
        if not name:
            self.fail('ASR002', 'Bad real expression')
        # Keep original order: prefixes may match an earlier unsupported name.
        builtins = ('pi', 'e', 'sqrt', 'exp', 'log', 'abs', 'sign', 'sin', 'cos', 'tan',
                    'asin', 'acos', 'atan', 'sind', 'cosd', 'tand', 'dasin', 'dacos',
                    'datan', 'floor', 'ceil', 'round', 'wrap', 'fmod', 'frac',
                    'remainder', 'sqr', 'pow', 'atan2', 'datan2')
        matched = next((item for item in builtins if item.startswith(name)), None)
        if matched not in ('sqrt', 'abs', 'floor', 'ceil', 'round', 'fmod', 'frac',
                           'remainder', 'sqr', 'pow'):
            self.fail('ASR003', f"Real function/constant branch not recovered: '{name}'")
        self.expect('(')
        lhs = float32(self.real())
        rhs = None
        if matched in ('fmod', 'remainder', 'pow'):
            self.expect(',')
            rhs = float32(self.real())
        self.expect(')')
        if matched == 'sqrt':
            return math.sqrt(lhs) if lhs >= 0 else math.nan
        if matched == 'abs':
            return abs(lhs)
        if matched == 'sqr':
            return lhs * lhs
        if matched == 'pow':
            return portable_pow(lhs, rhs)
        if matched == 'fmod':
            try:
                return math.fmod(lhs, rhs)
            except ValueError:
                return math.nan
        if matched == 'remainder':
            quotient = ieee_divide(lhs, rhs)
            return lhs - math.floor(quotient) * rhs if math.isfinite(quotient) else math.nan
        if matched == 'frac':
            return lhs - math.floor(lhs) if math.isfinite(lhs) else math.nan
        if not math.isfinite(lhs):
            return lhs
        if matched == 'floor':
            return float(math.floor(lhs))
        if matched == 'ceil':
            return float(math.ceil(lhs))
        return float(math.floor(lhs + 0.5))


def parse_real(text):
    reader = RealExpression(text)
    if not text:
        reader.fail('ASX008', 'empty expression')
    value = reader.real()
    if reader.cursor != len(text):
        reader.fail('ASR004', 'Garbage at end of real expression')
    return float32(value)  # Conversion route models the receiving float store.


def parse_vector(text, size=3, rgba=False):
    if size not in (2, 3, 4) or (rgba and size != 4):
        raise ArgScriptError('ASR005', 'Unsupported vector shape (host guard)')
    reader = RealExpression(text)
    values = [float32(reader.real())]
    if rgba:
        for _ in range(2):
            reader.expect(',')
            values.append(float32(reader.real()))
    reader.space()
    if not reader.peek():
        return tuple(values + ([1.0] if rgba else values * (size - 1)))
    reader.expect(',')
    while len(values) < size:
        values.append(float32(reader.real()))
        if len(values) < size:
            reader.expect(',')
    # Native vector entry points do not enforce EOF after the last component.
    return tuple(values)


def numeric_converters():
    """Opt-in portable model, preserving old integer-only converter behavior."""
    return {**integer_expression_converters(),
            'StrtodEnglish.float32': lambda text: float32(read_real(text).value),
            'StrtodEnglish.float64': lambda text: read_real(text).value,
            'ParseFloat': parse_real,
            'ParseVector2': lambda text: parse_vector(text, 2),
            'ParseVector3': lambda text: parse_vector(text, 3),
            'ParseVector4': lambda text: parse_vector(text, 4),
            'ParseRGBA': lambda text: parse_vector(text, 4, rgba=True)}
