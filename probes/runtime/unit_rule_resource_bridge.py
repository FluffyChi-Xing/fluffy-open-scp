"""Read-only ERZ 5/2/14 resource-prefix decoder; not a complete UnitRule decoder.

Usage: python probes/runtime/unit_rule_resource_bridge.py binary rule-id ...
Pool layouts follow cEcoUnitRule::Read and ApplyAmountsFromBins. Unknown tail
fields remain unnamed because development and retail record lengths differ.
"""
import argparse
import json
from pathlib import Path
import struct
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'tools/dynamic/rule-logs'))
from resolve_ids import resolve, resource_ops


def decode(data, ids):
    result = resolve(data, ids)
    pool = data[result['poolOffset']:result['poolOffset'] + result['poolSize']]
    for row in result['matches']:
        if row['scope'] != 'unit':
            continue
        words = struct.unpack_from('>13i', data, row['offset'])
        row['resourcePrefix'] = {
            scope: resource_ops(pool, words[i], words[i + 1])
            for scope, i in [('local', 3), ('nearby', 5), ('map', 7), ('global', 9)]
        }
        offset, count = words[11:13]
        if count < 0 or count > 1000000 or (count and (offset < 0 or offset + count * 8 > len(pool))):
            raise ValueError('from_bin_bounds')
        ops = []
        for i in range(min(count, 64)):
            source, target, slot, resource = struct.unpack_from('<BBHI', pool, offset + i * 8)
            ops.append({'sourceScopeIndex': source & 15,
                        'readRemainingCapacity': bool(source & 16),
                        'targetScopeIndex': target & 15,
                        'writeMode': target >> 4, 'targetOperationIndex': slot,
                        'sourceResourceId': f'0x{resource:08x}',
                        'rawSourceFlags': source, 'rawTargetFlags': target})
        row['fromBinOps'] = {'offset': offset, 'count': count, 'values': ops, 'truncated': count > 64}
    result['evidenceKind'] = 'unit-resource-prefix-static-candidates'
    result['limitations'] = ['Not original ER2 source', 'Tail/control/transport fields not decoded',
                              'Static operations precede runtime specialization and fromBin substitution']
    return result


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('binary', type=Path)
    ap.add_argument('ids', nargs='+', type=lambda x: int(x, 0))
    ap.add_argument('--out', type=Path)
    args = ap.parse_args()
    report = json.dumps(decode(args.binary.read_bytes(), set(args.ids)), indent=2) + '\n'
    if args.out:
        args.out.write_text(report, encoding='utf8')
    else:
        print(report)
