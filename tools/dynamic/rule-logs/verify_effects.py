"""Independently check captured resource snapshots; never infer unobserved writes."""
import argparse
import hashlib
import json
from pathlib import Path


def i32(value):
    return ((value + 2**31) % 2**32) - 2**31


def integer(value):
    return type(value) is int and -(2**31) <= value < 2**31


def verify(rows):
    results, sequence = [], 0
    for row in rows:
        if row.get('channel') != 'simulation.effect' or row.get('schema') != 1:
            raise ValueError('unsupported effect event')
        if type(row.get('sequence')) is not int or row['sequence'] <= sequence:
            raise ValueError('sequence must increase')
        sequence = row['sequence']
        operations = row.get('operations')
        if not isinstance(operations, list) or len(operations) > 16 or not integer(row.get('applyCount')):
            raise ValueError('invalid operation list or apply count')
        for ordinal, op in enumerate(operations):
            result = dict(sequence=sequence, ordinal=ordinal, resourceId=op.get('resourceId'),
                          verified=False, reason=op.get('status'))
            if op.get('status') == 'observed' and not row.get('readErrors'):
                before, after = op.get('before', {}), op.get('after', {})
                valid = (op.get('ordinal') == ordinal
                         and all(integer(op.get(k)) for k in ['input', 'output'])
                         and op['output'] >= 0
                         and all(integer(s.get(k)) for s in [before, after] for k in ['amount', 'capacity', 'resourceIndex', 'binIndex'])
                         and before.get('pointer') not in [None, '0x0']
                         and all(before.get(k) == after.get(k) for k in ['pointer', 'resourceIndex', 'binIndex']))
                if valid:
                    consumed = op['input'] & (i32(-op['input']) >> 31)
                    expected = i32(row['applyCount'] * i32(op['output'] - consumed))
                    delta = after['amount'] - before['amount']
                    matches = after['amount'] == i32(before['amount'] + expected)
                    consistent = (op.get('delta') == delta and op.get('expectedDeltaInt32') == expected
                                  and op.get('matchesInt32') is matches)
                    result.update(verified=consistent, reason='consistent-snapshots' if consistent else 'derived-field-mismatch',
                                  delta=delta, expectedDeltaInt32=expected, matchesInt32=matches,
                                  nativeSuccess=row.get('nativeSuccess'))
                else:
                    result['reason'] = 'invalid-snapshots'
            results.append(result)
    return results


def summarize(directory):
    raw = (directory / 'trace.jsonl').read_bytes()
    rows = [json.loads(line) for line in raw.splitlines() if line.strip()]
    status = json.loads((directory / 'status.json').read_text(encoding='utf-8'))
    return dict(schema=1, traceSha256=hashlib.sha256(raw).hexdigest(), status=status,
                recordCountMatches=status.get('records') == len(rows), operations=verify(rows))


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('directory', type=Path)
    print(json.dumps(summarize(ap.parse_args().directory), ensure_ascii=False, indent=2))
