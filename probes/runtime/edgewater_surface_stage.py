"""Stage the collision fix and 4096 m city-to-region terrain stamp geometry."""
import argparse
import hashlib
import json
from pathlib import Path

from capstone import CS_ARCH_X86, CS_MODE_32, Cs
from edgewater_collision_sizing import describe_collision
from edgewater_native_sizing import conditional
from edgewater_stage_native import stage
from inspect_crash_dump import PEImage


# ESI holds the terrain graphics object throughout these instructions.
# The destination page size stays 2048 << mip. Only the source footprint changes.
SITES = (
    ("stampHalf", 0xC03D3A, "f30f103dd4e2cf00", "f30f58ff"),
    ("stampWidth", 0xC03D89, "f30f5e442450", "f30f58c0"),
    ("stampAlignX", 0xC03E8F, "d90584e1d100", "d8c0"),
    ("stampAlignY", 0xC03EC4, "d90584e1d100", "d8c0"),
)


def describe_surface(path):
    manifest = describe_collision(path)
    image = PEImage(path)
    decoder = Cs(CS_ARCH_X86, CS_MODE_32)
    for name, site, original_hex, double_hex in SITES:
        original = bytes.fromhex(original_hex)
        assert image.read(site - image.image_base, len(original)) == original
        body = original + conditional(bytes.fromhex(double_hex), bytes.fromhex("81be207a0200"))
        instructions = list(decoder.disasm(body, 0))
        assert sum(insn.size for insn in instructions) == len(body)
        jump, = (insn for insn in instructions if insn.mnemonic == "jne")
        restore, = (insn for insn in instructions if insn.mnemonic == "popfd")
        assert int(jump.op_str, 16) == restore.address
        manifest["stubs"].append({
            "name": name,
            "rva": hex(site - image.image_base),
            "resumeRva": hex(site - image.image_base + len(original)),
            "stock": original.hex(),
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
    data = stage(args.source, describe_surface)
    with args.output.open("xb") as output:
        output.write(data)
    with args.output.with_suffix(".json").open("x", encoding="utf-8") as output:
        json.dump(describe_surface(args.source), output, indent=2)
        output.write("\n")
    print(hashlib.sha256(data).hexdigest())


if __name__ == "__main__":
    main()
