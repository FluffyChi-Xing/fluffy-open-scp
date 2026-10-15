"""Decode the verified GlassBox delta action header format.

The payload format is action-specific and is intentionally left to a schema
layer. This module only consumes the common header and never guesses payload
lengths.
"""

from dataclasses import dataclass
import struct
from typing import Optional


class DeltaDecodeError(ValueError):
    """Raised when bytes cannot represent a valid action header."""


@dataclass(frozen=True)
class ActionHeader:
    offset: int
    action_id: int
    data1: int
    data2: int
    header: int
    header_size: int


def _read_i32_be(data: bytes, offset: int) -> int:
    if offset + 4 > len(data):
        raise DeltaDecodeError("truncated extended action field")
    return struct.unpack_from(">i", data, offset)[0]


def decode_action_header(data: bytes, offset: int = 0) -> ActionHeader:
    """Decode one ``EncodeActionHeader`` value from *data*.

    The retail/development implementation writes the 32-bit header in big
    endian. ``0xfffe`` and ``0x3e`` are extension markers; ``0xffff`` and
    ``0x3f`` encode -1. The returned ``header_size`` includes extensions.
    """
    if offset < 0 or offset + 4 > len(data):
        raise DeltaDecodeError("truncated action header")
    header = struct.unpack_from(">I", data, offset)[0]
    if header & 0xC0000000 != 0xC0000000:
        raise DeltaDecodeError("invalid action header marker")

    # EncodeActionHeader lays out bytes as [id|0xc0, data2, data1_hi,
    # data1_lo] before writing the uint32 in big endian.
    action_id = (header >> 24) & 0x3F
    if action_id >= 0x2F:
        raise DeltaDecodeError(f"unsupported action id: {action_id}")

    raw_data1 = header & 0xFFFF
    raw_data2 = (header >> 16) & 0xFF
    cursor = offset + 4

    if raw_data1 == 0xFFFF:
        data1 = -1
    elif raw_data1 == 0xFFFE:
        data1 = _read_i32_be(data, cursor)
        cursor += 4
    else:
        data1 = raw_data1

    if raw_data2 == 0x3F:
        data2 = -1
    elif raw_data2 == 0x3E:
        data2 = _read_i32_be(data, cursor)
        cursor += 4
    else:
        data2 = raw_data2

    return ActionHeader(offset, action_id, data1, data2, header, cursor - offset)


def iter_action_headers(data: bytes, *, offset: int = 0, limit: Optional[int] = None):
    """Yield headers until bytes end or *limit* records are reached.

    This is useful for probing a raw stream. It stops at the first byte that
    is not a valid action header because action-specific payloads must be
    skipped by a schema-aware decoder.
    """
    count = 0
    cursor = offset
    while cursor + 4 <= len(data) and (limit is None or count < limit):
        try:
            record = decode_action_header(data, cursor)
        except DeltaDecodeError:
            return
        yield record
        cursor += record.header_size
        count += 1
