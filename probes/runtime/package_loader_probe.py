"""Reproduce the bounded SimCity package-loader static evidence.

Reads supplied PE files only; never starts or modifies the game. Capstone is
needed only for --disassemble. Addresses are specific to the two known hashes.
Output belongs in an ignored local evidence directory, not the public repo.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
from pathlib import Path


KNOWN_SHA256 = {
    "06d681444011f1297a6cf53bc233ee5f138d41786b451e6f18dc02bf273ff569",
    "2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab",
}
# Bounded evidence spans, not a claim that each span is an entire function.
SPANS = {
    "AddPackageIfValid": (0x5EC40, 0x5C8),
    "RegisterPackages": (0x5F420, 0x1EC),
    "DirectoryIterator.Read": (0x515610, 0x2A0),
    "SetupResources": (0x5F6E0, 784),
    "RegisterMemoryPackage": (0x5DEB0, 384),
    "Manager.RegisterDatabase": (0x5145A0, 219),
    "InternalDatabaseList.Add": (0x511080, 139),
    "Manager.FindDatabase": (0x510B40, 75),
    "AppSystem.PreInit": (0x1A480, 0x83E),
    "AppSystem.Init": (0x16F80, 0x8F1),
    "Locale.RegisterDatabases": (0x61E90, 0x664),
    "DatabasePackedFile.Attach": (0x4E8830, 0xA6),
    "DatabasePackedFile.VerifyHeader": (0x4E9080, 0x87),
    "Manager.GetResourceInternal": (0x511760, 0x37B),
    "Manager.CheckCacheInternal": (0x511110, 0xFD),
    "EcoGame.InitVersions": (0x1C8C20, 0x64B),
    "EcoGame.AddPackage": (0x1C86B0, 0x223),
    "EcoGame.SwitchProduction": (0x1C7740, 0x77),
    "EcoGame.SwitchScripts": (0x1C6D70, 0x2D3),
    "package_info_group": (0x9E1C68, 4),
}


class PE:
    def __init__(self, path: Path):
        self.path = path.resolve()
        self.data = self.path.read_bytes()
        self.sha256 = hashlib.sha256(self.data).hexdigest()
        if self.sha256 not in KNOWN_SHA256:
            raise ValueError(f"Unrecognized build: {self.path}: {self.sha256}")
        pe = struct.unpack_from("<I", self.data, 0x3C)[0]
        if self.data[:2] != b"MZ" or self.data[pe:pe + 4] != b"PE\0\0":
            raise ValueError("Not a PE file")
        if struct.unpack_from("<H", self.data, pe + 24)[0] != 0x10B:
            raise ValueError("Expected PE32")
        self.base = struct.unpack_from("<I", self.data, pe + 52)[0]
        count = struct.unpack_from("<H", self.data, pe + 6)[0]
        opt_size = struct.unpack_from("<H", self.data, pe + 20)[0]
        self.sections = []
        for i in range(count):
            pos = pe + 24 + opt_size + i * 40
            _, rva, size, raw = struct.unpack_from("<4I", self.data, pos + 8)
            self.sections.append((rva, size, raw))

    def read(self, rva: int, size: int) -> tuple[int, bytes]:
        for start, length, raw in self.sections:
            if start <= rva and rva + size <= start + length:
                offset = raw + rva - start
                data = self.data[offset:offset + size]
                if len(data) != size:
                    raise ValueError("Truncated section")
                return offset, data
        raise ValueError(f"RVA {rva:#x} + {size:#x} is not file-backed")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path, nargs="+")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--disassemble", action="store_true")
    args = parser.parse_args()
    sources = [PE(path) for path in args.binary]
    if args.disassemble:
        import capstone
        decoder = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
    args.output.mkdir(parents=True, exist_ok=True)
    manifest = {"schema": 1, "evidence_kind": "offline-static", "sources": [], "spans": []}
    for source in sources:
        manifest["sources"].append({
            "path": str(source.path), "sha256": source.sha256,
            "image_base": hex(source.base), "size": len(source.data),
        })
    for name, (rva, size) in SPANS.items():
        items, contents = [], []
        for index, source in enumerate(sources):
            offset, data = source.read(rva, size)
            contents.append(data)
            item = {"source": index, "file_offset": hex(offset),
                    "sha256": hashlib.sha256(data).hexdigest(), "entry_bytes": data[:16].hex()}
            if name == "package_info_group":
                item["u32"] = hex(struct.unpack("<I", data)[0])
            elif args.disassemble:
                lines = [f"{ins.address:08X} {ins.bytes.hex():24} {ins.mnemonic:8} {ins.op_str}"
                         for ins in decoder.disasm(data, source.base + rva)]
                filename = f"{index}-{name}.asm.txt"
                (args.output / filename).write_text("\n".join(lines) + "\n", encoding="utf-8")
                item["disassembly"] = filename
            items.append(item)
        manifest["spans"].append({"name": name, "rva": hex(rva), "length": size,
                                  "all_equal": all(x == contents[0] for x in contents),
                                  "sources": items})
    path = args.output / "manifest.json"
    path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(path.resolve())


if __name__ == "__main__":
    main()
