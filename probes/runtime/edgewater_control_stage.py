"""Stage a matched XY projection for the retail city control-map passes."""
import argparse
import hashlib
import json
from pathlib import Path

from capstone import CS_ARCH_X86, CS_MODE_32, Cs
from edgewater_native_sizing import conditional
from edgewater_stage_native import stage
from edgewater_zoning_stage import describe_zoning
from inspect_crash_dump import PEImage


SITES = (0x680C23, 0x680C41)
STOCK = bytes.fromhex("d90584e1d100")
# The wrapper is the same GameTerrain global used by UpdatePhysicsMap.
# Leave the original extent if either pointer is absent.
LOAD_TERRAIN = bytes.fromhex("a14404fe00 85c0 7417 8b4018 85c0 7410")
SCALE = conditional(bytes.fromhex("d8c0"), bytes.fromhex("81b8207a0200"))
assert len(SCALE) == 16


def projection_body():
    # Both null jumps land on the outer POPFD, after the environment gate.
    return STOCK + b"\x50\x9c" + LOAD_TERRAIN + SCALE + b"\x9d\x58"


def describe_control(path):
    manifest = describe_zoning(path)
    image = PEImage(path)
    decoder = Cs(CS_ARCH_X86, CS_MODE_32)
    body = projection_body()
    instructions = list(decoder.disasm(body, 0))
    assert sum(insn.size for insn in instructions) == len(body)
    null_jumps = [insn for insn in instructions if insn.mnemonic in ("je", "jz")]
    restore = [insn for insn in instructions if insn.mnemonic == "popfd"][-1]
    assert all(int(jump.op_str, 16) == restore.address for jump in null_jumps)
    for axis, site in zip(("X", "Y"), SITES):
        assert image.read(site - image.image_base, len(STOCK)) == STOCK
        manifest["stubs"].append({
            "name": f"controlExtent{axis}",
            "rva": hex(site - image.image_base),
            "resumeRva": hex(site - image.image_base + len(STOCK)),
            "stock": STOCK.hex(),
            "body": body.hex(),
            "callRva": None,
            "instructions": [f"{insn.mnemonic} {insn.op_str}".strip() for insn in instructions],
        })
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to(Path(__file__).resolve().parents[2] / "tmp"):
        parser.error("stage this prototype under the workspace tmp directory")
    data = stage(args.source, describe_control)
    with args.output.open("xb") as output:
        output.write(data)
    with args.output.with_suffix(".json").open("x", encoding="utf-8") as output:
        json.dump(describe_control(args.source), output, indent=2)
        output.write("\n")
    print(hashlib.sha256(data).hexdigest())


if __name__ == "__main__":
    main()
