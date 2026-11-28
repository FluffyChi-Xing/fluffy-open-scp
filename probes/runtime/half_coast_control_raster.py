"""Read the live Half Coast control render target without attaching or injecting."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

from school_resource_watch import Reader


# The two fingerprinted EXEs use the same texture-manager storage and layout.
EXE_HASHES = {
    "a94399a363a4e243eaa7622257595c91cf0d11d840b1fde1722eba222e05b963",
    "3ecf0ffd11ebd4e132d287bb0b330727bf2d9d1fb81af0667b3eb28b5dfc8670",
}
ENVIRONMENT = 0xE11F2425
CONTROL = (0x1E53E618, 0, 0x40212A00)


def locate_raster(reader, base):
    # 0x405D30 exports the texture manager. GetTexture (0x4FBB60) passes
    # manager+0x68 to the retail hash lookup (0x6FEBF0).
    manager = reader.u32(base + 0xA4EFC4)
    buckets, count = struct.unpack("<II", reader.read(manager + 0x6C, 8))
    if buckets < 0x10000 or not 1 <= count <= 1_000_000:
        raise ValueError("texture manager is unavailable")
    node = reader.u32(buckets + 4 * ((CONTROL[0] ^ CONTROL[2]) % count))
    seen = set()
    while node:
        if node in seen or len(seen) >= 1024:
            raise ValueError("invalid or moving texture hash chain")
        seen.add(node)
        instance, group, kind, value, next_node = struct.unpack("<5I", reader.read(node, 20))
        if (instance, group, kind) == CONTROL:
            raster = reader.u32(value)
            width, height = struct.unpack("<HH", reader.read(raster + 0xC, 4))
            if not 1 <= width <= 16384 or not 1 <= height <= 16384:
                raise ValueError("unexpected raster size")
            return node, raster, width, height
        node = next_node
    raise ValueError("controlMapRT is not loaded")


def snapshot(reader, base):
    def terrain_identity():
        wrapper = reader.u32(base + 0xBE0444)
        terrain = reader.u32(wrapper + 0x18)
        return terrain, reader.u32(terrain + 0x27A20)

    identity = terrain_identity()
    if identity[1] != ENVIRONMENT:
        raise ValueError("active terrain is not Half Coast")
    sample = locate_raster(reader, base)
    half_width, = struct.unpack("<f", reader.read(base + 0xC3D43C, 4))
    if sample != locate_raster(reader, base) or identity != terrain_identity():
        raise ValueError("the terrain or raster changed during sampling")
    return {
        "environment": f"{identity[1]:08X}",
        "controlWidth": sample[2],
        "controlHeight": sample[3],
        "flatScale": half_width,
        "integerStrideForConfigured1024Face": sample[2] // 1024,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pid", required=True, type=int)
    args = parser.parse_args()
    reader = Reader(args.pid)
    try:
        module = next(item for item in reader.modules if item["name"].lower().startswith("simcity-halfcoast-8192"))
        fingerprint = hashlib.sha256(Path(module["path"]).read_bytes()).hexdigest()
        if fingerprint not in EXE_HASHES:
            raise ValueError("unsupported executable")
        print(json.dumps({"pid": args.pid, "sha256": fingerprint, **snapshot(reader, module["base"])}, indent=2))
    finally:
        reader.close()


if __name__ == "__main__":
    main()
