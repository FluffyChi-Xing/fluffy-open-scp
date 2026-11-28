"""Link the verified EdgeWaterBay sizing stubs into a workspace-only PE copy."""
import argparse
from pathlib import Path
import struct

import pefile

from edgewater_native_sizing import describe


def align(value, alignment):
    return (value + alignment - 1) // alignment * alignment


def branch(opcode, source, target):
    return struct.pack("<Bi", opcode, target - (source + 5))


def stage(source, describe_image=describe):
    manifest = describe_image(source)
    original = source.read_bytes()
    pe = pefile.PE(data=original)
    optional = pe.OPTIONAL_HEADER
    certificate = optional.DATA_DIRECTORY[4]
    last = pe.sections[-1]
    rva = align(last.VirtualAddress + last.Misc_VirtualSize, optional.SectionAlignment)
    assert rva == optional.SizeOfImage
    header = last.get_file_offset() + 40
    assert header + 40 <= optional.SizeOfHeaders and original[header:header + 40] == bytes(40)
    raw = align(len(original), optional.FileAlignment)
    payload = bytearray()
    redirects = []
    for item in manifest["stubs"]:
        payload.extend(b"\xcc" * (align(len(payload), 16) - len(payload)))
        entry = rva + len(payload)
        payload.extend(bytes.fromhex(item["body"]))
        if item["callRva"] is not None:
            payload.extend(branch(0xE8, rva + len(payload), int(item["callRva"], 16)))
        payload.extend(branch(0xE9, rva + len(payload), int(item["resumeRva"], 16)))
        site = int(item["rva"], 16)
        stock = bytes.fromhex(item["stock"])
        redirects.append((site, entry, stock))
    size = align(len(payload), optional.FileAlignment)
    result = bytearray(original)
    result.extend(bytes(raw - len(original)))
    result.extend(payload)
    result.extend(bytes(size - len(payload)))
    struct.pack_into("<8s6I2HI", result, header, b".ew4096", len(payload), rva, size, raw, 0, 0, 0, 0, 0x60000020)
    struct.pack_into("<H", result, pe.FILE_HEADER.get_field_absolute_offset("NumberOfSections"), len(pe.sections) + 1)
    struct.pack_into("<I", result, optional.get_field_absolute_offset("SizeOfImage"), align(rva + len(payload), optional.SectionAlignment))
    struct.pack_into("<I", result, optional.get_field_absolute_offset("SizeOfCode"), optional.SizeOfCode + size)
    # The original certificate remains in the copied bytes but no longer covers this image.
    struct.pack_into("<II", result, certificate.get_file_offset(), 0, 0)
    for site, entry, stock in redirects:
        offset = pe.get_offset_from_rva(site)
        assert result[offset:offset + len(stock)] == stock
        result[offset:offset + len(stock)] = branch(0xE9, site, entry) + b"\x90" * (len(stock) - 5)
    staged = pefile.PE(data=result)
    struct.pack_into("<I", result, optional.get_field_absolute_offset("CheckSum"), staged.generate_checksum())
    staged = pefile.PE(data=result)
    assert staged.verify_checksum()
    assert staged.sections[-1].VirtualAddress == rva
    assert staged.get_data(rva, len(payload)) == payload
    for site, entry, stock in redirects:
        opcode, displacement = struct.unpack("<Bi", staged.get_data(site, 5))
        assert opcode == 0xE9 and site + 5 + displacement == entry
        assert staged.get_data(site + 5, len(stock) - 5) == b"\x90" * (len(stock) - 5)
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("retail_exe", type=Path)
    parser.add_argument("output_exe", type=Path)
    args = parser.parse_args()
    workspace = Path(__file__).resolve().parents[2]
    if not args.output_exe.resolve().is_relative_to(workspace / "tmp"):
        parser.error("stage this prototype under the workspace tmp directory")
    data = stage(args.retail_exe)
    with args.output_exe.open("xb") as output:
        output.write(data)
