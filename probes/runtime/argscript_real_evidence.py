"""Read-only static instruction evidence for a pinned development unwrapped PE.

Does not load/execute target code, attach to a process, or modify game files.
Other/original SimCity.exe files are not interchangeable with this artifact.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct


EXPECTED_SHA256 = '03045b870a3b3c405af72e0cd21c07aed1dfa9f08c0b8dd12aece4bcbf1f056e'
REGIONS = (
    ('StrtodEnglish', 0x89efc0, 0x162),
    ('EvalRealExpression', 0x8ca990, 0x6c),
    ('EvalRealTerm', 0x8ca680, 0x13b),
    ('EvalRealFactor', 0x8ca4e0, 0x58),
    ('Compare', 0x8c8f60, 0x28),
    ('EvalRealNumber-prefix', 0x8c9640, 0x17c),
)
CONSTANTS = (
    (0xc34118, '<d', 10.0), (0xc33cd0, '<d', 1.0),
    (0xc41368, '<d', 0.1), (0xc33cc4, '<f', 8388608.0),
    (0xc33cc8, '<I', 0x80000000), (0xcb8c94, '<f', 1.0),
)
SETTER_REGIONS = (('SetFCommand', 0x900e40, 0x57),)


def extract(binary, output, regions=REGIONS):
    data = binary.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != EXPECTED_SHA256:
        raise ValueError('Development executable identity mismatch; no address reuse permitted')
    import capstone
    import pefile
    pe = pefile.PE(data=data, fast_load=True)
    if pe.OPTIONAL_HEADER.ImageBase != 0x400000 or pe.FILE_HEADER.Machine != 0x14c:
        raise ValueError('Unexpected PE architecture or image base')
    disassembler = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
    rows = []
    for name, address, size in regions:
        blob = pe.get_data(address - 0x400000, size)
        instructions = [dict(address=f'{i.address:08X}', bytes=i.bytes.hex(),
                             mnemonic=i.mnemonic, operands=i.op_str)
                        for i in disassembler.disasm(blob, address)]
        if sum(len(bytes.fromhex(i['bytes'])) for i in instructions) != size:
            raise ValueError(f'Incomplete disassembly: {name}')
        rows.append(dict(name=name, address=f'{address:08X}', size=size,
                         sha256=hashlib.sha256(blob).hexdigest(), bytes=blob.hex(),
                         instructions=instructions))
    constants = []
    for address, format, expected in CONSTANTS:
        blob = pe.get_data(address - 0x400000, struct.calcsize(format))
        value = struct.unpack(format, blob)[0]
        if value != expected:
            raise ValueError(f'Constant mismatch at {address:08X}')
        constants.append(dict(address=f'{address:08X}', format=format,
                              bytes=blob.hex(), value=value))
    report = dict(schema='openscp.argscript.dev-instructions/1',
                  binary=str(binary.resolve()), sha256=digest,
                  imageBase='00400000', capstoneVersion=capstone.__version__,
                  evidenceKind='static-development-PE-instructions', regions=rows,
                  constants=constants,
                  limitations=['No target code execution or native result oracle',
                               'Instruction evidence does not prove host libm bit equivalence',
                               'Development addresses do not apply to retail'])
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--out', required=True, type=Path)
    parser.add_argument('--setters', action='store_true', help='Also retain SetFCommand double-store evidence')
    args = parser.parse_args()
    report = extract(args.binary, args.out, REGIONS + SETTER_REGIONS if args.setters else REGIONS)
    print(json.dumps(dict(sha256=report['sha256'], regions=len(report['regions']),
                         constants=len(report['constants']), output=str(args.out))))
