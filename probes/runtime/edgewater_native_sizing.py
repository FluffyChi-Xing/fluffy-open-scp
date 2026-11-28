"""Generate and verify retail x86 sizing stubs without patching an executable."""
import argparse
import hashlib
import json
import struct
from pathlib import Path

from capstone import CS_ARCH_X86, CS_MODE_32, Cs
from inspect_crash_dump import PEImage


EXE_SHA = "2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab"
ENVIRONMENT = 0x81CED36F
GRID_SITE = 0x5E8617
BOUNDS_SITE = 0x5E8784
GRID_STOCK = bytes.fromhex("f30f7e00 660fd6864c040000")
BOUNDS_STOCK = bytes.fromhex("d907 d99e34040000 83c40c")
FLAT_SCALE_SITE = 0xBE85CF
FLAT_SCALE_CALL = 0xBEC0F0
PHYSICS_SITE = 0xBF1169
PHYSICS_CALL = 0xBE6040


def immediate(value):
    return struct.pack("<I", value)


def conditional(body, compare=b"\x81\xbe" + struct.pack("<I", 0x3D4)):
    # Preserve flags around the environment gate.
    assert len(body) < 128
    return b"\x9c" + compare + immediate(ENVIRONMENT) + bytes((0x75, len(body))) + body + b"\x9d"


def stubs():
    grid = bytearray()
    for offset in (0x44C, 0x450):
        grid.extend(b"\xc7\x86" + immediate(offset) + immediate(256))
    bounds = bytearray()
    for offset, value in ((0, -2048.0), (4, -2048.0), (12, 2048.0), (16, 2048.0)):
        # mov dword ptr [edi + offset], float-bits
        bounds.extend(bytes((0xC7, 0x47, offset)) + struct.pack("<f", value))
    scale_stock = bytes.fromhex("d91c24 e8193b0000")
    scale = scale_stock[:3] + conditional(
        b"\xc7\x44\x24\x04" + struct.pack("<f", 2048.0),
        b"\x81\xbe" + immediate(0x27A20),
    )
    physics_stock = bytes.fromhex("6a00 6800010000 e8cb4effff")
    # edi is cTerrainGround. Its owner at +0x38 is cTerrainGfx.
    # Preserve eax while selecting the existing allocation call's face-size argument.
    physics = (
        physics_stock[:7]
        + bytes.fromhex("50 8b4738")
        + conditional(
            b"\xc7\x44\x24\x08" + immediate(512),
            b"\x81\xb8" + immediate(0x27A20),
        )
        + b"\x58"
    )
    return (
        ("grid", GRID_SITE, GRID_STOCK, GRID_STOCK + conditional(grid), None),
        ("bounds", BOUNDS_SITE, BOUNDS_STOCK, conditional(bounds) + BOUNDS_STOCK, None),
        ("terrainScale", FLAT_SCALE_SITE, scale_stock, scale, FLAT_SCALE_CALL),
        ("physicsFace", PHYSICS_SITE, physics_stock, physics, PHYSICS_CALL),
    )


def describe(path):
    image = PEImage(path)
    if hashlib.sha256(image.data).hexdigest() != EXE_SHA or image.image_base != 0x400000:
        raise ValueError("unsupported retail image")
    decoder = Cs(CS_ARCH_X86, CS_MODE_32)
    rows = []
    for name, site, original, body, target in stubs():
        assert image.read(site - image.image_base, len(original)) == original
        if target is not None:
            assert original[-5] == 0xE8
            assert site + len(original) + struct.unpack("<i", original[-4:])[0] == target
        insns = list(decoder.disasm(body, 0))
        assert sum(i.size for i in insns) == len(body)
        jump, = (i for i in insns if i.mnemonic == "jne")
        restore, = (i for i in insns if i.mnemonic in ("popfd", "popf"))
        assert int(jump.op_str, 16) == restore.address
        rows.append({
            "name": name,
            "rva": hex(site - image.image_base),
            "resumeRva": hex(site - image.image_base + len(original)),
            "stock": original.hex(),
            "body": body.hex(),
            "callRva": None if target is None else hex(target - image.image_base),
            "instructions": [f"{i.mnemonic} {i.op_str}".strip() for i in insns],
        })
    return {"exeSha256": EXE_SHA, "environment": hex(ENVIRONMENT), "stubs": rows}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("retail_exe", type=Path)
    parser.add_argument("output_json", type=Path)
    args = parser.parse_args()
    with args.output_json.open("x", encoding="utf-8") as output:
        json.dump(describe(args.retail_exe), output, indent=2)
        output.write("\n")
