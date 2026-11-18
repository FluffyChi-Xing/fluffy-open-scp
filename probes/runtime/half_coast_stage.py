"""Stage a Half Coast-specific retail executable using the verified sizing hooks."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

from capstone import CS_ARCH_X86, CS_MODE_32, Cs
from edgewater_control_stage import describe_control
from edgewater_stage_native import stage


ENVIRONMENT = 0xE11F2425
STOCK_PROTOTYPE_ENVIRONMENT = 0x81CED36F


def replace_once(code, old, new):
    if code.count(old) != 1:
        raise ValueError(f"Expected one sizing operand {old.hex()}")
    return code.replace(old, new, 1)


def describe_half_coast(source):
    manifest = describe_control(source)
    decoder = Cs(CS_ARCH_X86, CS_MODE_32)
    for row in manifest["stubs"]:
        name = row["name"]
        body = bytes.fromhex(row["body"])
        body = replace_once(body, struct.pack("<I", STOCK_PROTOTYPE_ENVIRONMENT),
                            struct.pack("<I", ENVIRONMENT))
        if name == "grid":
            for field in (0x44C, 0x450):
                body = replace_once(body, struct.pack("<II", field, 256), struct.pack("<II", field, 512))
        if name == "bounds":
            for offset, sign in ((0, -1), (4, -1), (12, 1), (16, 1)):
                body = replace_once(body, bytes((0x47, offset)) + struct.pack("<f", sign * 2048.0),
                                    bytes((0x47, offset)) + struct.pack("<f", sign * 4096.0))
        if name == "terrainScale":
            body = replace_once(body, struct.pack("<f", 2048.0), struct.pack("<f", 4096.0))
        if name == "physicsFace":
            body = replace_once(body, bytes.fromhex("c7442408") + struct.pack("<I", 512),
                                bytes.fromhex("c7442408") + struct.pack("<I", 1024))
        if name == "collisionScale":
            for offset in (0x48, 0x4C):
                body = replace_once(body, bytes((0x24, offset)) + struct.pack("<f", 2048.0),
                                    bytes((0x24, offset)) + struct.pack("<f", 4096.0))
        if name.startswith(("stamp", "controlExtent")):
            # Stock width / half-width ×4. Repeat the gated doubling instruction.
            insn = {"stampHalf": "f30f58ff", "stampWidth": "f30f58c0"}.get(name, "d8c0")
            gate = bytes((0x75, len(bytes.fromhex(insn)))) + bytes.fromhex(insn) + b"\x9d"
            body = replace_once(body, gate, bytes((0x75, 2 * len(bytes.fromhex(insn))))
                                + bytes.fromhex(insn) * 2 + b"\x9d")
            if name.startswith("controlExtent"):
                body = replace_once(body, bytes.fromhex("7417"), bytes.fromhex("7419"))
                body = replace_once(body, bytes.fromhex("7410"), bytes.fromhex("7412"))
        decoded = list(decoder.disasm(body, 0))
        assert sum(insn.size for insn in decoded) == len(body)
        restores = [insn.address for insn in decoded if insn.mnemonic == "popfd"]
        gate, = (insn for insn in decoded if insn.mnemonic == "jne")
        assert int(gate.op_str, 16) == restores[0]
        for null_branch in [insn for insn in decoded if insn.mnemonic == "je"]:
            assert int(null_branch.op_str, 16) == restores[-1]
        row["body"] = body.hex()
        row["instructions"] = [f"{insn.mnemonic} {insn.op_str}".strip() for insn in decoded]
    manifest["environment"] = hex(ENVIRONMENT)
    manifest["ecoSide"] = 512
    manifest["terrainSide"] = 1024
    manifest["cityWidth"] = 8192
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to(Path(__file__).resolve().parents[2] / "tmp"):
        parser.error("stage the prototype under the workspace tmp directory")
    data = stage(args.source, describe_half_coast)
    with args.output.open("xb") as out:
        out.write(data)
    with args.output.with_suffix(".json").open("x", encoding="utf-8") as out:
        json.dump(describe_half_coast(args.source), out, indent=2)
        out.write("\n")
    print(f"SHA-256 {hashlib.sha256(data).hexdigest()}")


if __name__ == "__main__":
    main()
