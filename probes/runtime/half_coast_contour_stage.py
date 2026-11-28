"""Stage Half Coast's environment-gated data-view width marker."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

from capstone import CS_ARCH_X86, CS_MODE_32, Cs
from edgewater_stage_native import stage
from half_coast_stage import ENVIRONMENT, describe_half_coast
from inspect_crash_dump import PEImage


SITE = 0xBF0A86
STOCK = bytes.fromhex("f30f115060")


def describe_contour(source):
    manifest = describe_half_coast(source)
    image = PEImage(source)
    # The retail producer stores real dimensions followed by reciprocals.
    assert image.read(SITE - image.image_base, 20) == bytes.fromhex(
        "f30f115060 f30f115864 f30f114868 f30f11406c"
    )
    # Preserve the real size in magnitude. Only for Half Coast set its sign.
    body = bytearray(STOCK + bytes.fromhex("9c52 8b154404fe00 85d2"))
    exits = []
    for prefix, opcode in (
        (b"", 0x74),
        (bytes.fromhex("8b5218 85d2"), 0x74),
        (bytes.fromhex("81ba207a0200") + struct.pack("<I", ENVIRONMENT), 0x75),
    ):
        body.extend(prefix)
        body.extend((opcode, 0))
        exits.append(len(body) - 1)
    body.extend(bytes.fromhex("81486000000080"))
    restore = len(body)
    body.extend(bytes.fromhex("5a9d"))
    for position in exits:
        body[position] = restore - (position + 1)
    decoded = list(Cs(CS_ARCH_X86, CS_MODE_32).disasm(body, 0))
    assert sum(i.size for i in decoded) == len(body)
    assert all(int(i.op_str, 16) == restore for i in decoded if i.mnemonic in ("je", "jne"))
    manifest["stubs"].append({
        "name": "halfCoastContourSize",
        "rva": hex(SITE - image.image_base),
        "resumeRva": hex(SITE - image.image_base + len(STOCK)),
        "stock": STOCK.hex(),
        "body": body.hex(),
        "callRva": None,
        "instructions": [f"{i.mnemonic} {i.op_str}".strip() for i in decoded],
    })
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to(Path(__file__).resolve().parents[2] / "tmp"):
        parser.error("stage the prototype under the workspace tmp directory")
    payload = stage(args.source, describe_contour)
    with args.output.open("xb") as out:
        out.write(payload)
    with args.output.with_suffix(".json").open("x", encoding="utf-8") as out:
        json.dump(describe_contour(args.source), out, indent=2)
        out.write("\n")
    print(hashlib.sha256(payload).hexdigest())


if __name__ == "__main__":
    main()
