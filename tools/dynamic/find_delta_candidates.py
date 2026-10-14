"""Find retail x86 code patterns related to the verified delta header encoder.

This is a read-only disk scanner. It does not load or attach to the game.
"""

import argparse
import struct
from pathlib import Path

from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from capstone.x86 import X86_OP_IMM


def executable_sections(blob: bytes):
    pe = struct.unpack_from("<I", blob, 0x3C)[0]
    if blob[pe:pe + 4] != b"PE\0\0":
        raise ValueError("not a PE image")
    coff = pe + 4
    sections_count = struct.unpack_from("<H", blob, coff + 2)[0]
    optional_size = struct.unpack_from("<H", blob, coff + 16)[0]
    optional = coff + 20
    magic = struct.unpack_from("<H", blob, optional)[0]
    if magic != 0x10B:
        raise ValueError("expected PE32")
    image_base = struct.unpack_from("<I", blob, optional + 28)[0]
    section = optional + optional_size
    for index in range(sections_count):
        pos = section + index * 40
        name = blob[pos:pos + 8].rstrip(b"\0").decode("ascii", "replace")
        virtual_size, virtual_address, raw_size, raw_offset = struct.unpack_from("<IIII", blob, pos + 8)
        characteristics = struct.unpack_from("<I", blob, pos + 36)[0]
        if characteristics & 0x20000000 and raw_size:
            yield name, image_base, virtual_address, blob[raw_offset:raw_offset + raw_size]


def scan(path: Path):
    blob = path.read_bytes()
    disassembler = Cs(CS_ARCH_X86, CS_MODE_32)
    disassembler.detail = True
    for name, image_base, rva, code in executable_sections(blob):
        for insn in disassembler.disasm(code, image_base + rva):
            for operand in insn.operands:
                if operand.type == X86_OP_IMM and operand.imm in (0xFFFE, 0xFFFF, 0x3E, 0x3F):
                    yield {
                        "section": name,
                        "va": f"0x{insn.address:08X}",
                        "rva": f"0x{insn.address - image_base:08X}",
                        "mnemonic": insn.mnemonic,
                        "opStr": insn.op_str,
                        "constant": f"0x{operand.imm & 0xFFFFFFFF:X}",
                    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("image", type=Path)
    args = parser.parse_args()
    hits = list(scan(args.image))
    print(f"image={args.image}")
    print(f"hits={len(hits)}")
    for hit in hits:
        print("{section} {rva} {mnemonic} {opStr} ({constant})".format(**hit))


if __name__ == "__main__":
    main()
