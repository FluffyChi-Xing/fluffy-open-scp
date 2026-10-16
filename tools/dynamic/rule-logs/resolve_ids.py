"""Find rule IDs in an explicitly exported ERZ 5/2/14 resource.

This reports offline candidates, never the winning live package or a rule name.
Layout follows crates/dbpf/src/erz.rs and retail serialization at 0x5FC7A0.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct


class Reader:
    def __init__(self, data):
        self.data, self.offset = data, 0

    def take(self, size):
        if size < 0 or size > len(self.data) - self.offset:
            raise ValueError(f'truncated ERZ at {self.offset}, need {size}')
        start = self.offset
        self.offset += size
        return self.data[start:self.offset]

    def u32(self):
        return struct.unpack('>I', self.take(4))[0]

    def tag(self, expected):
        value = self.u32()
        if value != expected:
            raise ValueError(f'unsupported tag {value}, expected {expected}')


def resolve(data, wanted):
    r = Reader(data)
    for version in [5, 2, 14]:
        r.tag(version)
    if r.take(1) != b'\0':
        raise ValueError('namesIncluded layout is not supported')
    matches = []
    unit_count = r.u32()
    for index in range(unit_count):
        offset, rule_id = r.offset, r.u32()
        r.take(128)
        r.take(r.u32() * 12)
        r.take(8)
        if rule_id in wanted:
            matches.append(dict(scope='unit', index=index, ruleId=rule_id,
                                ruleIdHex=f'0x{rule_id:08x}', offset=offset))
    r.tag(5)
    r.take(r.u32() * 72)
    r.tag(8)
    global_count = r.u32()
    for index in range(global_count):
        offset, rule_id = r.offset, r.u32()
        r.take(100)
        if rule_id in wanted:
            matches.append(dict(scope='global', index=index, ruleId=rule_id,
                                ruleIdHex=f'0x{rule_id:08x}', offset=offset,
                                definition=global_definition(data[offset:offset + 104])))
    r.tag(2)
    for _ in range(r.u32()):
        r.take(21)
        for size in [12, 12, 8]:
            r.take(r.u32() * size)
    r.take(r.u32() * 8)
    for _ in range(2):
        for _ in range(r.u32()):
            r.take(r.u32() * 20)
    pool_size = r.u32()
    pool_offset = r.offset
    pool = r.take(pool_size)
    if r.offset != len(data):
        raise ValueError('trailing ERZ bytes')
    for match in matches:
        if match['scope'] == 'global':
            definition = match['definition']
            definition['resources'] = resource_ops(pool, **definition['resources'])

    return dict(schema=1, evidenceKind='offline-rule-id-candidates', namesIncluded=False,
                unitCount=unit_count, globalCount=global_count, exact=True, matches=matches,
                poolOffset=pool_offset, poolSize=pool_size,
                sha256=hashlib.sha256(data).hexdigest())


def global_definition(record):
    # Retail Read 0x5FDD30: first ten words are serialized in memory order.
    w = struct.unpack('>10i', record[:40])
    return dict(flags=w[1] & 0xffffffff, simInfoOffset=w[2],
                resources=dict(offset=w[3], count=w[4]),
                fromBinOps=dict(offset=w[5], count=w[6]),
                minApply=w[7], maxApply=w[8], applyResourceId=f'0x{w[9] & 0xffffffff:08x}')


def resource_ops(pool, offset, count):
    if count < 0 or count > 1000000 or (count and (offset < 0 or offset > len(pool) or count * 12 > len(pool) - offset)):
        raise ValueError('resource_ops_bounds')
    values = []
    for i in range(min(count, 16)):
        # DataStore pool is copied verbatim on x86, unlike big-endian headers.
        resource_id, input_value, output = struct.unpack_from('<Iii', pool, offset + i * 12)
        values.append(dict(resourceId=f'0x{resource_id:08x}', input=input_value, output=output))
    return dict(offset=offset, count=count, values=values, truncated=count > 16)


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('binary', type=Path)
    ap.add_argument('ids', nargs='+', type=lambda s: int(s, 0))
    args = ap.parse_args()
    print(json.dumps(resolve(args.binary.read_bytes(), set(args.ids)), indent=2))
