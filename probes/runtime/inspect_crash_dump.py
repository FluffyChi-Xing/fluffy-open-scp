"""Offline x86 MINIDUMP inspection. Never opens or attaches to a live process.

Raw stack words are candidates, not an unwound call stack. Missing memory in
a minidump does not prove that the address was unmapped in the live process.
"""
import argparse
import hashlib
import json
import struct
from pathlib import Path


class PEImage:
    """Minimal offline PE32 code/import lookup; no loading of the DLL."""
    def __init__(self, path):
        self.path = path
        self.data = path.read_bytes()
        p = struct.unpack_from('<I', self.data, 60)[0]
        if self.data[:2] != b'MZ' or self.data[p:p + 4] != b'PE\0\0':
            raise ValueError('expected_pe')
        n = struct.unpack_from('<H', self.data, p + 6)[0]
        opt = struct.unpack_from('<H', self.data, p + 20)[0]
        if struct.unpack_from('<H', self.data, p + 24)[0] != 0x10b:
            raise ValueError('expected_pe32')
        self.timestamp = struct.unpack_from('<I', self.data, p + 8)[0]
        self.image_size = struct.unpack_from('<I', self.data, p + 24 + 56)[0]
        self.image_base = struct.unpack_from('<I', self.data, p + 24 + 28)[0]
        self.sections = [struct.unpack_from('<8sIIII', self.data, p + 24 + opt + i * 40) for i in range(n)]
        self.imports_rva = struct.unpack_from('<I', self.data, p + 24 + 104)[0]

    def read(self, rva, size):
        for _, _, base, count, offset in self.sections:
            if base <= rva and rva + size <= base + count:
                return self.data[offset + rva - base:offset + rva - base + size]
        raise ValueError('pe_rva_not_backed')

    def unpack(self, fmt, rva):
        return struct.unpack('<' + fmt, self.read(rva, struct.calcsize('<' + fmt)))

    def string(self, rva):
        result = bytearray()
        for i in range(4096):
            b = self.read(rva + i, 1)
            if b == b'\0':
                return result.decode('ascii')
            result.extend(b)
        raise ValueError('unterminated_pe_string')

    def imports(self):
        result = {}
        for i in range(4096):
            original, _, _, name, first = self.unpack('5I', self.imports_rva + 20 * i)
            if not name:
                return result
            dll = self.string(name)
            for j in range(65536):
                entry = self.unpack('I', (original or first) + 4 * j)[0]
                if not entry:
                    break
                symbol = f'ordinal:{entry & 65535}' if entry & 0x80000000 else self.string(entry + 2)
                result[first + 4 * j] = dll + '!' + symbol
        raise ValueError('import_table_limit')


class Dump:
    def __init__(self, path):
        self.data = path.read_bytes()
        if self.data[:4] != b'MDMP':
            raise ValueError('expected_minidump')
        self.streams = {}
        for i in range(self.unpack('I', 8)[0]):
            kind, size, offset = self.unpack('III', self.unpack('I', 12)[0] + 12 * i)
            self.streams[kind] = (size, offset)
        if self.unpack('H', self.streams[7][1])[0] != 0:
            raise ValueError('only_x86_supported')
        self.ranges = []
        if 5 in self.streams:
            r = self.streams[5][1]
            for i in range(self.unpack('I', r)[0]):
                base, size, offset = self.unpack('QII', r + 4 + i * 16)
                self.ranges.append((base & 0xffffffff, size, offset))
        if 9 in self.streams:
            r = self.streams[9][1]
            count, offset = self.unpack('QQ', r)
            for i in range(count):
                base, size = self.unpack('QQ', r + 16 + i * 16)
                self.ranges.append((base & 0xffffffff, size, offset))
                offset += size
        self.modules = []
        r = self.streams[4][1]
        for i in range(self.unpack('I', r)[0]):
            base, size, checksum, timestamp, name = self.unpack('QIIII', r + 4 + i * 108)
            self.modules.append(dict(base=base & 0xffffffff, size=size, checksum=checksum,
                                     timestamp=timestamp, path=self.string(name)))

    def unpack(self, fmt, offset):
        return struct.unpack_from('<' + fmt, self.data, offset)

    def string(self, offset):
        size = self.unpack('I', offset)[0]
        return self.data[offset + 4:offset + 4 + size].decode('utf-16-le', errors='replace')

    def read(self, address, size):
        for base, count, offset in self.ranges:
            if base <= address and address + size <= base + count:
                return self.data[offset + address - base:offset + address - base + size]
        return None

    def owner(self, address):
        for m in self.modules:
            if m['base'] <= address < m['base'] + m['size']:
                return {'module': m['path'], 'rva': hex(address - m['base'])}
        return None

    def code(self, address, size=32):
        raw = self.read(address, size)
        result = {'address': hex(address), 'owner': self.owner(address), 'bytes': raw.hex() if raw else None}
        if raw:
            try:
                from capstone import Cs, CS_ARCH_X86, CS_MODE_32
                result['instructions'] = [f'{i.address:08x}: {i.mnemonic} {i.op_str}' for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(raw, address)]
            except ImportError:
                pass
        return result


def inspect(path):
    d = Dump(path)
    r = d.streams[6][1]
    code, flags, chain, address, count, _ = d.unpack('IIQQII', r + 8)
    _, context = d.unpack('II', r + 160)
    regs = {k: d.unpack('I', context + offset)[0] for k, offset in {
        'edi': 156, 'esi': 160, 'ebx': 164, 'edx': 168, 'ecx': 172,
        'eax': 176, 'ebp': 180, 'eip': 184, 'esp': 196}.items()}
    stack = []
    for i in range(96):
        at = regs['esp'] + i * 4
        raw = d.read(at, 4)
        if raw is None:
            break
        value = struct.unpack('<I', raw)[0]
        stack.append({'address': hex(at), 'value': hex(value), 'owner': d.owner(value)})
    hooks = []
    game = next(m for m in d.modules if m['path'].lower().endswith('\\simcity.exe'))
    for name, rva in {'globalRun': 0x218890, 'unitRun': 0x21db00, 'applyOps': 0x206580, 'binFind': 0x18ec70}.items():
        at = game['base'] + rva
        entry = {'name': name, **d.code(at)}
        raw = d.read(at, 5)
        if raw and raw[0] == 0xe9:
            target = (at + 5 + struct.unpack('<i', raw[1:])[0]) & 0xffffffff
            entry['jumpTarget'] = d.code(target, 96)
        hooks.append(entry)
    unloaded = []
    if 14 in d.streams:
        r = d.streams[14][1]
        header, stride, count_unloaded = d.unpack('III', r)
        for i in range(count_unloaded):
            base, size, _, _, name = d.unpack('QIIII', r + header + stride * i)
            unloaded.append({'base': hex(base & 0xffffffff), 'size': hex(size), 'path': d.string(name)})
    # Retain earlier exception/context candidates from the faulting thread's
    # stack. Do not label arbitrary stack words as an unwound call chain.
    saved = []
    tr = d.streams[3][1]
    for i in range(d.unpack('I', tr)[0]):
        at = tr + 4 + i * 48
        if d.unpack('I', at)[0] != d.unpack('I', d.streams[6][1])[0]:
            continue
        base, size, offset = d.unpack('QII', at + 24)
        raw = d.data[offset:offset + size]
        for j in range(0, len(raw) - 80, 4):
            ec, ef, chain, ip, n = struct.unpack_from('<5I', raw, j)
            if ec != 0xc0000005 or ef >= 4 or n != 2:
                continue
            row = {'recordAddress': hex(base + j), 'exceptionAddress': hex(ip),
                   'parameters': [hex(v) for v in struct.unpack_from('<2I', raw, j + 20)]}
            for k in range(0, len(raw) - 204, 4):
                cf = struct.unpack_from('<I', raw, k)[0]
                ci = struct.unpack_from('<I', raw, k + 184)[0]
                if cf & 0xffffff00 != 0x10000 or ci != ip:
                    continue
                sp = struct.unpack_from('<I', raw, k + 196)[0]
                row['contextAddress'] = hex(base + k)
                row['esp'] = hex(sp)
                row['stackWordsNotUnwound'] = []
                for step in range(12):
                    word = d.read(sp + step * 4, 4)
                    if not word:
                        break
                    value = struct.unpack('<I', word)[0]
                    row['stackWordsNotUnwound'].append({'address': hex(sp + step * 4),
                        'value': hex(value), 'owner': d.owner(value)})
                break
            saved.append(row)
    return {'path': str(path), 'sha256': hashlib.sha256(d.data).hexdigest(),
            'dumpTimestampUnix': d.unpack('I', 20)[0], 'threadId': d.unpack('I', d.streams[6][1])[0],
            'exception': {'code': hex(code), 'flags': flags, 'address': hex(address),
                          'parameters': [hex(x) for x in d.unpack('15Q', d.streams[6][1] + 40)[:count]]},
            'registers': {k: hex(v) for k, v in regs.items()}, 'faultCode': d.code(regs['eip']),
            'hooks': hooks, 'rawStackWordsNotUnwound': stack, 'modules': d.modules, 'unloadedModules': unloaded,
            'savedExceptionCandidates': saved,
            'limits': ['Missing bytes mean not retained, not necessarily unmapped.',
                       'Raw stack words are not a verified call stack.', 'No live process access.']}


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('dump', type=Path)
    ap.add_argument('--out', type=Path, required=True)
    ap.add_argument('--kernelbase', type=Path, help='Optional matching x86 KernelBase.dll for offline callsite decoding')
    ap.add_argument('--game', type=Path, help='Optional matching SimCity.exe for offline crash-handler decoding')
    args = ap.parse_args()
    report = inspect(args.dump)
    report['offlineImages'] = []
    for path, sites in [(args.kernelbase, [(0x147b30, 32), (0x14fe6a, 28)]),
                        (args.game, [(0x51b4f0, 50), (0x51c4e8, 22)])]:
        if path is None:
            continue
        pe = PEImage(path)
        mod = next(m for m in report['modules'] if m['path'].replace('\\', '/').split('/')[-1].lower() == path.name.lower())
        if pe.timestamp != mod['timestamp'] or pe.image_size != mod['size']:
            raise ValueError('offline_image_identity_mismatch')
        from capstone import Cs, CS_ARCH_X86, CS_MODE_32
        imports = pe.imports()
        rows = []
        for rva, size in sites:
            raw = pe.read(rva, size)
            instructions = []
            for ins in Cs(CS_ARCH_X86, CS_MODE_32).disasm(raw, mod['base'] + rva):
                row = {'address': hex(ins.address), 'text': ins.mnemonic + ' ' + ins.op_str}
                if ins.bytes[:2] == b'\xff\x15':
                    iat_rva = struct.unpack('<I', ins.bytes[2:6])[0] - pe.image_base
                    row['import'] = imports.get(iat_rva)
                instructions.append(row)
            rows.append({'rva': hex(rva), 'bytes': raw.hex(), 'instructions': instructions})
        item = {'path': str(path), 'sha256': hashlib.sha256(pe.data).hexdigest(),
                'timestampAndImageSizeMatchDump': True, 'source': 'disk bytes, not runtime code', 'sites': rows}
        if path == args.game:
            item['referencedStrings'] = {hex(rva): pe.string(rva) for rva in [0x92cbd8, 0x92cbc4]}
        report['offlineImages'].append(item)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({k: report[k] for k in ('threadId', 'exception', 'faultCode', 'hooks')}, indent=2))
