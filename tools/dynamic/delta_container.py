"""Parse the observed retail ``SLDelta*.mfs`` record envelope.

This parser deliberately treats the record payload as opaque. The envelope
was observed from the retail save stream and is independent of the nested
action schema, which still needs version-specific validation.
"""

from dataclasses import dataclass
import struct
from pathlib import Path


MAGIC = b"\xff\x02\x03\x00"
HEADER_SIZE = 24


class DeltaContainerError(ValueError):
    pass


@dataclass(frozen=True)
class DeltaRecord:
    offset: int
    size: int
    delta_id: int
    sim_ticks: int
    kind: int
    payload: bytes


def parse_records(data: bytes) -> tuple[int, list[DeltaRecord]]:
    """Return the file version and all complete retail delta records."""
    if len(data) < 4:
        raise DeltaContainerError("truncated file header")
    # The four-byte file preamble is little endian; record fields below are
    # big endian, matching the observed retail envelope.
    version = struct.unpack_from("<I", data, 0)[0]
    records: list[DeltaRecord] = []
    cursor = 4
    while cursor < len(data):
        if cursor + HEADER_SIZE > len(data):
            raise DeltaContainerError(f"truncated record header at {cursor}")
        if data[cursor:cursor + 4] != MAGIC:
            raise DeltaContainerError(f"invalid record marker at {cursor}")
        size, delta_id, sim_ticks, kind = struct.unpack_from(">IQII", data, cursor + 4)
        if size < HEADER_SIZE:
            raise DeltaContainerError(f"invalid record size {size} at {cursor}")
        end = cursor + size
        if end > len(data):
            raise DeltaContainerError(f"truncated record at {cursor}: need {size} bytes")
        records.append(DeltaRecord(cursor, size, delta_id, sim_ticks, kind, data[cursor + HEADER_SIZE:end]))
        cursor = end
    return version, records


def parse_file(path: str | Path) -> tuple[int, list[DeltaRecord]]:
    return parse_records(Path(path).read_bytes())
