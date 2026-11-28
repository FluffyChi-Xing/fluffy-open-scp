"""ASCII integer reads from development EA::StdC::StrtoU64Common.

The shared reader clamps at 64-bit bounds; int32 consumers use the low word.
No locale, process errno, or native function execution is involved.
"""
from dataclasses import dataclass

from argscript_variables import ArgScriptError


SPACE = ' \t\n\r\v\f'
U64_MAX = (1 << 64) - 1
I64_MAX = (1 << 63) - 1


def int32(value):
    value &= 0xffffffff
    return value if value < 0x80000000 else value - 0x100000000


@dataclass(frozen=True)
class IntegerRead:
    bits: int
    end: int
    overflow: bool
    digit_read: bool


def read_integer(text, start=0, base=10, unsigned=False):
    if not text.isascii() or '\0' in text:
        raise ArgScriptError('ASN001', 'Non-ASCII or embedded-NUL numeric input is unsupported')
    if not 0 <= start <= len(text):
        raise ArgScriptError('ASN002', 'Invalid input cursor (host guard)')
    if base < 0 or base == 1 or base > 36:
        return IntegerRead(0, start, False, False)
    cursor = start
    while cursor < len(text) and text[cursor] in SPACE:
        cursor += 1
    negative = cursor < len(text) and text[cursor] == '-'
    if cursor < len(text) and text[cursor] in '+-':
        cursor += 1
    if base == 0:
        base = 8 if text[cursor:cursor + 1] == '0' else 10
        if text[cursor:cursor + 2].lower() == '0x':
            base = 16
    if base == 16 and text[cursor:cursor + 2].lower() == '0x':
        cursor += 2
    value, overflow, digit_read = 0, False, False
    while cursor < len(text):
        char = text[cursor]
        digit = ord(char) - ord('0') if '0' <= char <= '9' else (
            ord(char.upper()) - ord('A') + 10 if char.isalpha() else 99)
        if digit >= base:
            break
        digit_read = True
        candidate = value * base + digit
        if candidate > U64_MAX:
            overflow = True
        else:
            value = candidate
        cursor += 1
    if not digit_read:
        return IntegerRead(0, start, False, False)
    if unsigned:
        if overflow:
            value = U64_MAX
    elif overflow or value > (1 << 63 if negative else I64_MAX):
        overflow = True
        value = 1 << 63 if negative else I64_MAX
    if negative:
        value = (-value) & U64_MAX
    return IntegerRead(value, cursor, overflow, True)


def read_int32(text):
    return int32(read_integer(text).bits)


def raw_integer_converters():
    """These routes all observe int32 values at the modeled consumer boundary."""
    return {'AtoI64': read_int32, 'StrtoI32': read_int32,
            'AtoI64.bool': lambda text: read_int32(text) != 0,
            'StrtoI32.bool': lambda text: read_int32(text) != 0}
