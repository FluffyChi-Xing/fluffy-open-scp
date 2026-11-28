"""Stage an environment-scoped repair for the zoning obstruction grid."""
import argparse
import hashlib
import json
from pathlib import Path

from capstone import CS_ARCH_X86, CS_MODE_32, Cs
from edgewater_native_sizing import conditional
from edgewater_stage_native import stage
from edgewater_surface_stage import describe_surface
from inspect_crash_dump import PEImage


SITE = 0x6EFEEA
STOCK = bytes.fromhex("8b0d24f0e400")


def describe_zoning(path):
    manifest = describe_surface(path)
    image = PEImage(path)
    assert image.read(SITE - image.image_base, len(STOCK)) == STOCK
    # EBX is the zoning game. Both builders and the later loops use these
    # locals. Read the already initialized EcoGame instance bounds.
    sizes = bytes.fromhex("8bb84c040000 897c2434 8b8850040000 894c2424")
    body = (
        bytes.fromhex("50 8b430c")
        + conditional(sizes, bytes.fromhex("81b8d4030000"))
        + bytes.fromhex("58")
        + STOCK
    )
    instructions = list(Cs(CS_ARCH_X86, CS_MODE_32).disasm(body, 0))
    assert sum(insn.size for insn in instructions) == len(body)
    jump, = (insn for insn in instructions if insn.mnemonic == "jne")
    restore, = (insn for insn in instructions if insn.mnemonic == "popfd")
    assert int(jump.op_str, 16) == restore.address
    manifest["stubs"].append({
        "name": "zoningBounds",
        "rva": hex(SITE - image.image_base),
        "resumeRva": hex(SITE - image.image_base + len(STOCK)),
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
    data = stage(args.source, describe_zoning)
    with args.output.open("xb") as output:
        output.write(data)
    with args.output.with_suffix(".json").open("x", encoding="utf-8") as output:
        json.dump(describe_zoning(args.source), output, indent=2)
        output.write("\n")
    print(hashlib.sha256(data).hexdigest())


if __name__ == "__main__":
    main()
