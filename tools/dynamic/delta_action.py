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


def decode_action_header(data: bytes, offset: int = 0, *, stream_version: int) -> ActionHeader:
    """Decode one ``EncodeActionHeader`` value from *data*.

    Both builds write the header big endian. data1 uses 0xfffe/0xffff.
    Retail DecodeActionHeader (VA 0x5cc1d0, known offline build) selects
    data2 markers 0x3e/0x3f for stream versions below 0x1b and 0xfe/0xff
    otherwise. The stream version is NOT the outer MFS envelope version 4.
    Require the caller to supply it rather than guessing from payload bytes.
    """
    if isinstance(stream_version, bool) or not isinstance(stream_version, int) or stream_version < 0:
        raise DeltaDecodeError("stream_version must be a non-negative integer")
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

    extension2 = 0x3E if stream_version < 0x1B else 0xFE
    if raw_data2 == extension2 + 1:
        data2 = -1
    elif raw_data2 >= extension2:
        data2 = _read_i32_be(data, cursor)
        cursor += 4
    else:
        data2 = raw_data2

    return ActionHeader(offset, action_id, data1, data2, header, cursor - offset)


def iter_action_headers(data: bytes, *, stream_version: int, offset: int = 0, limit: Optional[int] = None):
    """Yield headers until bytes end or *limit* records are reached.

    This is useful for probing a raw stream. It stops at the first byte that
    is not a valid action header because action-specific payloads must be
    skipped by a schema-aware decoder.
    """
    count = 0
    cursor = offset
    while cursor + 4 <= len(data) and (limit is None or count < limit):
        try:
            record = decode_action_header(data, cursor, stream_version=stream_version)
        except DeltaDecodeError:
            return
        yield record
        cursor += record.header_size
        count += 1
