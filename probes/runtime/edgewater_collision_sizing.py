"""Stage the 1031 prototype with a matching collision heightfield footprint."""
import argparse
import hashlib
import json
from pathlib import Path
import struct

from capstone import CS_ARCH_X86, CS_MODE_32, Cs
from edgewater_native_sizing import conditional, describe
from edgewater_stage_native import stage
from inspect_crash_dump import PEImage


COLLISION_SITE = 0x688C11
COLLISION_STOCK = bytes.fromhex("83c418 8d442428")


def describe_collision(path):
    manifest = describe(path)
    image = PEImage(path)
    assert image.read(COLLISION_SITE - image.image_base, len(COLLISION_STOCK)) == COLLISION_STOCK

    # cGraphicsGame::StartGame has finished reading the original scale and offset.
    # ESI holds cGraphicsGame, whose +0x0c points to the EcoGame.
    # After dropping the property-call arguments, scale starts at ESP+0x40.
    # PUSH EAX and PUSHFD temporarily add eight bytes to those offsets.
    scale = b"".join(b"\xc7\x44\x24" + bytes((offset,)) + struct.pack("<f", 2048.0)
                     for offset in (0x48, 0x4C))
    body = (
        COLLISION_STOCK[:3]
        + bytes.fromhex("50 8b460c")
        + conditional(scale, bytes.fromhex("81b8 d4030000"))
        + bytes.fromhex("58")
        + COLLISION_STOCK[3:]
    )
    decoded = list(Cs(CS_ARCH_X86, CS_MODE_32).disasm(body, 0))
    assert sum(insn.size for insn in decoded) == len(body)
    jump, = (insn for insn in decoded if insn.mnemonic == "jne")
    restore, = (insn for insn in decoded if insn.mnemonic == "popfd")
    assert int(jump.op_str, 16) == restore.address
    manifest["stubs"].append({
        "name": "collisionScale",
        "rva": hex(COLLISION_SITE - image.image_base),
        "resumeRva": hex(COLLISION_SITE - image.image_base + len(COLLISION_STOCK)),
        "stock": COLLISION_STOCK.hex(),
        "body": body.hex(),
        "callRva": None,
        "instructions": [f"{insn.mnemonic} {insn.op_str}".strip() for insn in decoded],
    })
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("retail_exe", type=Path)
    parser.add_argument("output_exe", type=Path)
    args = parser.parse_args()
    workspace_tmp = Path(__file__).resolve().parents[2] / "tmp"
    if not args.output_exe.resolve().is_relative_to(workspace_tmp):
        parser.error("stage this prototype under the workspace tmp directory")
    data = stage(args.retail_exe, describe_collision)
    with args.output_exe.open("xb") as output:
        output.write(data)
    with args.output_exe.with_suffix(".json").open("x", encoding="utf-8") as output:
        json.dump(describe_collision(args.retail_exe), output, indent=2)
        output.write("\n")
    print(hashlib.sha256(data).hexdigest())


if __name__ == "__main__":
    main()
