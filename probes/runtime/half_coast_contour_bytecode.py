"""Build the Half Coast contour shader at the retail pixel-cache creation site."""
import argparse
import ctypes as ct
from difflib import unified_diff
import hashlib
from pathlib import Path
import re
import struct

from edgewater_stage_native import stage
from half_coast_contour_stage import describe_contour
from inspect_crash_dump import PEImage


SITE = 0x548DEB
CREATE = 0x422280
STOCK = bytes.fromhex("e89094edff")
VARIANTS = 4
SAMPLE = "mad_pp r0.xy, v0.zwzw, c10, r0.y"
TABLE_SHA256 = (
    "eaab68aac08119bb9855024a81970e8899b3f4dd812e6ab6bf602fdbace01657",
    "bb13caf3f9689f8aa233bab86fd5781b0bb9ca42ac3a727a1edbcc0862290f72",
    "035809139022ad4ce9b023e7179c1e6659ab12dabd6de217546b5b9442c636cb",
    "a2ae3db91b1a52320b76458eb975aeb46c430e5b7975711204d0fc1d451aab4c",
)


def read_blob(blob):
    vtable = ct.cast(blob, ct.POINTER(ct.POINTER(ct.c_void_p)))[0]
    start = ct.WINFUNCTYPE(ct.c_void_p, ct.c_void_p)(vtable[3])(blob)
    size = ct.WINFUNCTYPE(ct.c_size_t, ct.c_void_p)(vtable[4])(blob)
    payload = ct.string_at(start, size)
    ct.WINFUNCTYPE(ct.c_ulong, ct.c_void_p)(vtable[2])(blob)
    return payload


def disassemble(code):
    function = ct.WinDLL("d3dcompiler_47").D3DDisassemble
    function.argtypes = [ct.c_char_p, ct.c_size_t, ct.c_uint, ct.c_char_p, ct.POINTER(ct.c_void_p)]
    output = ct.c_void_p()
    if function(code, len(code), 0, None, ct.byref(output)):
        raise ValueError("Could not disassemble the retail contour shader")
    return read_blob(output).decode("ascii")


def assemble(source):
    function = ct.WinDLL("d3dx9_43").D3DXAssembleShader
    output, errors = ct.c_void_p(), ct.c_void_p()
    source = source.encode("ascii")
    if function(source, len(source), None, None, 0, ct.byref(output), ct.byref(errors)):
        raise ValueError(read_blob(errors).decode("ascii") if errors else "D3D assembly failed")
    return read_blob(output)


def shader_end(data, start):
    pos = start + 4
    while pos + 4 <= len(data):
        word, = struct.unpack_from("<I", data, pos)
        opcode = word & 0xFFFF
        if opcode == 0xFFFF:
            return pos + 4
        words = (word >> 16) & 0x7FFF if opcode == 0xFFFE else (word >> 24) & 15
        pos += 4 * (words + 1)
    raise ValueError("Contour shader has no END token")


def read_variants(data):
    variants = []
    for hit in re.finditer(rb"\x00\x03\xff\xff\xfe\xff..(?:DBUG|CTAB)", data, re.DOTALL):
        start = hit.start()
        pos = start + 4
        while data[pos:pos + 2] == b"\xfe\xff":
            words, = struct.unpack_from("<H", data, pos + 2)
            pos += 4 * (words + 1)
        if b"contourMap\0" not in data[start:pos] or b"contourInfo\0" not in data[start:pos]:
            continue
        size, = struct.unpack_from(">I", data, start - 4)
        end = shader_end(data, start)
        if start + size != end:
            raise ValueError("Pixel-cache record length does not match shader bytecode")
        variants.append(data[start:end])
    if len(variants) != VARIANTS:
        raise ValueError(f"Expected {VARIANTS} stock contour variants, found {len(variants)}")
    return variants


def instructions(source):
    return [line.split("//", 1)[0].strip() for line in source.splitlines()
            if line.strip() and not line.lstrip().startswith(("//", "#"))]


def patch_shader(code):
    lines = instructions(disassemble(code))
    if lines[0] != "ps_3_0" or lines.count(SAMPLE) != 1:
        raise ValueError("Unexpected stock shader sampling contract")
    if re.search(r"\b(?:c220|r31)\b", "\n".join(lines)):
        raise ValueError("Contour patch registers are already in use")
    limit = [line for line in lines if re.fullmatch(r"add_pp r0.xy, c\d+\.[xyzw], v1_abs", line)]
    if len(limit) != 1:
        raise ValueError("Could not uniquely locate contour clip")
    register, component = re.search(r"(c\d+)\.([xyzw])", limit[0]).groups()
    definition = next(line for line in lines if line.startswith(f"def {register}, "))
    if float(definition.split(", ")[1 + "xyzw".index(component)]) != -1024:
        raise ValueError("Unexpected stock contour clip distance")

    lines.insert(1, "def c220, 1, 0.25, 0.5, 0.0001220703125")
    sample = lines.index(SAMPLE)
    lines[sample:sample + 1] = [
        "mad r31.xy, v1, c220.w, c220.z",
        "cmp r31.xy, c10.x, v0.zwzw, r31",
        "mad_pp r0.xy, r31, c10_abs, r0.y",
    ]
    clip = lines.index(limit[0])
    lines[clip:clip + 1] = [
        "mov r31.w, c10.x",
        "cmp r31.z, r31.w, c220.x, c220.y",
        "mul r31.xy, v1_abs, r31.z",
        limit[0].replace("v1_abs", "r31"),
    ]
    output = assemble("\n".join(lines) + "\n")
    roundtrip = instructions(disassemble(output))
    if struct.pack("<4f", 1, 0.25, 0.5, 1 / 8192) not in output:
        raise ValueError("Unexpected packed contour constants")
    lines[1] = "def c220, 1, 0.25, 0.5, 0.000122070312"
    if roundtrip != lines:
        raise ValueError("D3D instruction roundtrip mismatch\n" +
                         "\n".join(unified_diff(lines, roundtrip)))
    return output


def select_shader(pairs):
    # Keep the original two cdecl arguments and three saved registers intact
    # until bytecode and its full size match a known stock cache record.
    body = bytearray.fromhex("565751 e800000000 5a")
    anchor = 8
    matches = []
    for original, patched in pairs:
        size = len(original)
        body.extend(bytes.fromhex("817c243c") + struct.pack("<I", size))
        body.append(0x75)
        wrong_size = len(body)
        body.extend(bytes.fromhex("00 8b74240c 8dba"))
        original_offset = len(body)
        body.extend(bytes(4))
        body.extend(bytes.fromhex("b9") + struct.pack("<I", size))
        body.extend(bytes.fromhex("f3a6 75"))
        wrong_code = len(body)
        body.extend(bytes.fromhex("00 8d82"))
        patched_offset = len(body)
        body.extend(bytes(4))
        body.extend(bytes.fromhex("8944240c e9"))
        exit_offset = len(body)
        body.extend(bytes(4))
        for fixup in (wrong_size, wrong_code):
            body[fixup] = len(body) - fixup - 1
        matches.append((original_offset, patched_offset, exit_offset, original, patched))
    body.append(0xE9)
    skip_data = len(body)
    body.extend(bytes(4))
    for original_offset, patched_offset, _, original, patched in matches:
        for field, code in ((original_offset, original), (patched_offset, patched)):
            struct.pack_into("<i", body, field, len(body) - anchor)
            body.extend(code)
    epilogue = len(body)
    for offset in [skip_data] + [match[2] for match in matches]:
        struct.pack_into("<i", body, offset, epilogue - offset - 4)
    body.extend(bytes.fromhex("595f5e"))
    return bytes(body)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("stock_exe", type=Path)
    parser.add_argument("shader_tables", type=Path, nargs=4)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if not args.output.resolve().is_relative_to(Path(__file__).resolve().parents[2] / "tmp"):
        parser.error("stage under workspace tmp")
    originals = {}
    for table, expected in zip(args.shader_tables, TABLE_SHA256, strict=True):
        data = table.read_bytes()
        if hashlib.sha256(data).hexdigest() != expected:
            raise ValueError(f"Unexpected stock shader table: {table}")
        originals.update(dict.fromkeys(read_variants(data)))
    pairs = [(original, patch_shader(original)) for original in originals]

    def describe(source):
        manifest = describe_contour(source)
        image = PEImage(source)
        assert image.read(SITE - image.image_base, len(STOCK)) == STOCK
        manifest["stubs"].append({
            "name": "halfCoastContourCache",
            "rva": hex(SITE - image.image_base),
            "resumeRva": hex(SITE + len(STOCK) - image.image_base),
            "stock": STOCK.hex(),
            "body": select_shader(pairs).hex(),
            "callRva": hex(CREATE - image.image_base),
        })
        return manifest

    result = stage(args.stock_exe, describe)
    with args.output.open("xb") as output:
        output.write(result)
    print(f"{len(pairs)} distinct contour programs, SHA-256 {hashlib.sha256(result).hexdigest()}")


if __name__ == "__main__":
    main()
