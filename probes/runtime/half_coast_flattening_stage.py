"""Stage Half Coast's default road-flattening shader scale for the 8192 m city."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

from capstone import CS_ARCH_X86, CS_MODE_32, Cs

from edgewater_stage_native import stage
from half_coast_stage import ENVIRONMENT, describe_half_coast
from inspect_crash_dump import PEImage


SITE = 0x661251
STOCK = bytes.fromhex("b8 ccaadf00")
VECTOR = (1.0, 0.0, 1.0 / 8192, 0.5)
STOCK_VECTOR = (1.0, 0.0, 1.0 / 2048, 0.5)


def describe_flattening(source):
    manifest = describe_half_coast(source)
    image = PEImage(source)
    assert image.read(SITE - image.image_base, len(STOCK)) == STOCK
    assert image.unpack("4f", 0xDFAACC - image.image_base) == STOCK_VECTOR
    assert image.read(SITE - image.image_base - 2, 2) == b"\x75\x05"
    # The conditional branch immediately before SITE skips this fallback if
    # the renderer has supplied its own slot 730 vector.
    body = (
        STOCK + bytes.fromhex("9c 52 8b154404fe00 85d2 741c 8b5218 85d2 7415")
        + bytes.fromhex("81ba207a0200") + struct.pack("<I", ENVIRONMENT)
        + bytes.fromhex("7509 e800000000 58 8d4017 5a9d eb1f")
        + b"\x90" * 15 + struct.pack("<4f", *VECTOR)
    )
    vector_offset = len(body) - 16
    instructions = list(Cs(CS_ARCH_X86, CS_MODE_32).disasm(body[:vector_offset], 0))
    assert sum(i.size for i in instructions) == vector_offset
    restore, = (i for i in instructions if i.mnemonic == "pop" and i.op_str == "edx")
    for ins in instructions:
        if ins.mnemonic in ("je", "jne"):
            assert int(ins.op_str, 16) == restore.address
    call, = (i for i in instructions if i.mnemonic == "call")
    lea, = (i for i in instructions if i.mnemonic == "lea")
    assert call.address + call.size + int(lea.op_str.split("+")[1].strip(" ]"), 0) == vector_offset
    skip, = (i for i in instructions if i.mnemonic == "jmp")
    assert int(skip.op_str, 16) == len(body)
    assert vector_offset % 16 == 0
    manifest["stubs"].append({
        "name": "halfCoastDefaultFlattening",
        "rva": hex(SITE - image.image_base),
        "resumeRva": hex(SITE - image.image_base + len(STOCK)),
        "stock": STOCK.hex(),
        "body": body.hex(),
        "callRva": None,
        "instructions": [f"{i.mnemonic} {i.op_str}".strip() for i in instructions],
        "vectorOffset": vector_offset,
    })
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to(Path(__file__).resolve().parents[2] / "tmp"):
        parser.error("stage the prototype under the workspace tmp directory")
    payload = stage(args.source, describe_flattening)
    with args.output.open("xb") as output:
        output.write(payload)
    with args.output.with_suffix(".json").open("x", encoding="utf-8") as output:
        json.dump(describe_flattening(args.source), output, indent=2)
        output.write("\n")
    print(hashlib.sha256(payload).hexdigest())


if __name__ == "__main__":
    main()
