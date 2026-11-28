"""Prepare an offline 4096 m / 256-square EGB grid from the stock city bootstrap."""
import argparse
import gzip
import hashlib
import struct
from pathlib import Path


SOURCE_SHA = "0ac5c2608a14df296de3b790f06e187eac20c614a9c000d1fa63ae9ee45c7d3a"
OLD = 128
NEW = 256
INSET = (NEW - OLD) // 2


def expand(payload):
    def word(offset):
        return struct.unpack_from(">I", payload, offset)[0]

    assert word(0) == 17
    env = 16 + 8 * word(12)
    assert word(env) == 0x81CED36F
    header = env + 20 + 8 * word(env + 4)
    assert word(header - 4) == 0
    assert (word(header), word(header + 4)) == (34, 3)
    offset = header + 8
    result = bytearray(payload[:offset])
    recovered = bytearray(payload[:offset])
    seen = set()
    for _ in range(34):
        map_id, cells = struct.unpack_from(">II", payload, offset)
        assert map_id not in seen and cells == OLD * OLD
        seen.add(map_id)
        start = offset + 8
        end = start + cells * 4
        raw = payload[start:end]
        grid = bytearray(NEW * NEW * 4)
        for y in range(OLD):
            origin = y * OLD * 4
            target = ((y + INSET) * NEW + INSET) * 4
            grid[target:target + OLD * 4] = raw[origin:origin + OLD * 4]
        total = sum(x for (x,) in struct.iter_unpack(">i", grid))
        assert total == struct.unpack_from(">i", payload, end)[0]
        result.extend(struct.pack(">II", map_id, NEW * NEW))
        result.extend(grid)
        result.extend(struct.pack(">i", total))
        recovered.extend(struct.pack(">II", map_id, OLD * OLD))
        for y in range(INSET, INSET + OLD):
            start = (y * NEW + INSET) * 4
            recovered.extend(grid[start:start + OLD * 4])
        recovered.extend(struct.pack(">i", total))
        offset = end + 4
    result.extend(payload[offset:])
    recovered.extend(payload[offset:])
    assert recovered == payload
    return bytes(result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("stock_gzip", type=Path)
    parser.add_argument("output_bin", type=Path)
    args = parser.parse_args()
    packed = args.stock_gzip.read_bytes()
    if hashlib.sha256(packed).hexdigest() != SOURCE_SHA:
        parser.error("expected the original EdgeWaterBay 1031 bootstrap")
    result = expand(gzip.decompress(packed))
    with args.output_bin.open("xb") as stream:
        stream.write(result)


if __name__ == "__main__":
    main()
