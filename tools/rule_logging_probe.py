"""Reproduce bounded retail rule-logging evidence from known SimCity PE files.

Static reads only: no process attachment, native calls or game-file writes.
Function names are semantic identifications checked against dev source and
retail instructions, not claims that the retail executable contains symbols.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from capstone import Cs, CS_ARCH_X86, CS_MODE_32
from package_loader_probe import PE

# Start/end VAs verified with IDA 9.2 and raw instructions, image base 0x400000.
SPANS = {
    "EncodeActionHeader": (0x5CC130, 0x5CC1D0),
    "DecodeActionHeader": (0x5CC1D0, 0x5CC28B),
    "EcoGame.LogAction": (0x5CFB00, 0x5CFB57),
    "EcoGame.LogGlobalRule": (0x5CFE20, 0x5CFE8F),
    "EcoGame.LogUnitRule": (0x5D47F0, 0x5D48DF),
    "EcoBinSet.ApplyRule": (0x618AC0, 0x618B55),
    "EcoUnit.ApplyRule": (0x61FD60, 0x61FE6D),
    "Scripts.UnitRuleFromIndex": (0x5F76B0, 0x5F76F8),
    "Scripts.GlobalRuleFromIndex": (0x5F6AE0, 0x5F6AFD),
    "UnitRuleDeque.Advance": (0x5F6CD0, 0x5F6D4C),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path, nargs="+")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    images = [PE(path) for path in args.binary]
    args.output.mkdir(parents=True, exist_ok=True)
    decoder = Cs(CS_ARCH_X86, CS_MODE_32)
    report = {"schema": 1, "evidenceKind": "offline-static", "runtimeVerified": False,
              "sources": [{"path": str(p.path), "sha256": p.sha256, "imageBase": hex(p.base)} for p in images], "functions": []}
    for name, (start, end) in SPANS.items():
        spans = []
        for index, pe in enumerate(images):
            offset, code = pe.read(start - 0x400000, end - start)
            instructions = list(decoder.disasm(code, pe.base + start - 0x400000))
            if sum(i.size for i in instructions) != len(code):
                raise ValueError(f"Incomplete disassembly: {name}")
            filename = f"{index}-{name}.asm.txt"
            (args.output / filename).write_text("\n".join(
                f"{i.address:08X} {i.bytes.hex():24} {i.mnemonic:8} {i.op_str}" for i in instructions) + "\n", encoding="utf-8")
            spans.append({"source": index, "fileOffset": hex(offset), "sha256": hashlib.sha256(code).hexdigest(),
                          "entryBytes": code[:16].hex(), "disassembly": filename})
        report["functions"].append({"name": name, "va": hex(start), "rva": hex(start - 0x400000), "size": end-start,
                                    "allEqual": len({x["sha256"] for x in spans}) == 1, "spans": spans})
    (args.output / "manifest.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps({"functions": len(SPANS), "sources": len(images), "allEqual": all(f["allEqual"] for f in report["functions"])}))


if __name__ == "__main__":
    main()
