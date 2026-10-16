"""Independently verify complete record/create/return chains in one capture.

No joins across capture directories or guesses from filenames. Missing/capped
events remain unverified even if the agent reported created-from-record.
"""
import argparse
import hashlib
import json
from pathlib import Path


def same_origin(a, b):
    return (isinstance(a.get('database'), dict)
            and a['database'].get('pointer') is not None
            and a['database'] == b.get('database')
            and isinstance(a.get('recordKey'), dict)
            and a['recordKey'] == b.get('recordKey'))


def verify(rows):
    groups, results = {}, []
    previous_sequence = 0
    for row in rows:
        if row.get('channel') != 'resource.provenance' or row.get('schema') != 1:
            raise ValueError('unsupported event')
        sequence = row.get('sequence')
        if not isinstance(sequence, int) or sequence <= previous_sequence:
            raise ValueError('event sequence must increase')
        previous_sequence = sequence
        request_id = row.get('requestId')
        if not isinstance(request_id, int) or request_id <= 0:
            raise ValueError('invalid requestId')
        group = groups.setdefault(request_id, [])
        if any(r.get('phase') == 'request' for r in group):
            raise ValueError('events after request completion')
        group.append(row)
    for request_id, group in groups.items():
        finals = [r for r in group if r.get('phase') == 'request']
        if not finals:
            results.append({'requestId': request_id, 'verified': False, 'reason': 'incomplete-request'})
            continue
        final = finals[0]
        result = {'requestId': request_id, 'verified': False, 'reason': final.get('provenance'),
                  'key': final.get('key'), 'resource': final.get('resource')}
        if final.get('success') is True and final.get('provenance') == 'created-from-record':
            creates = [r for r in group if r.get('phase') == 'create' and r.get('success') is True
                       and r.get('resource') == final.get('resource') and same_origin(r, final)]
            chains = []
            for create in creates:
                records = [r for r in group if r.get('phase') == 'record' and r.get('success') is True
                           and r.get('materialize') is True and r.get('record') == create.get('record')
                           and r['sequence'] < create['sequence'] < final['sequence']
                           and same_origin(r, create)]
                chains.extend((r, create) for r in records)
            if len(chains) == 1:
                record, create = chains[0]
                abstract, concrete = final.get('key') or {}, record['recordKey']
                if (abstract.get('type') == 0x08068ae9 and concrete.get('type') in [0x08068aea, 0x08068aeb]
                        and all(abstract.get(k) == concrete.get(k) for k in ['instance', 'group'])
                        and record.get('record') not in [None, '0x0']
                        and final.get('resource') not in [None, '0x0']):
                    result.update(verified=True, reason='complete-chain', database=record['database'],
                                  recordKey=concrete, sequences=[record['sequence'], create['sequence'], final['sequence']])
            if not result['verified']:
                result['reason'] = 'missing-or-ambiguous-chain'
        results.append(result)
    return results


def summarize(directory):
    raw = (directory / 'trace.jsonl').read_bytes()
    rows = [json.loads(line) for line in raw.splitlines() if line.strip()]
    status = json.loads((directory / 'status.json').read_text(encoding='utf-8'))
    return {'schema': 1, 'traceSha256': hashlib.sha256(raw).hexdigest(), 'status': status,
            'recordCountMatches': status.get('records') == len(rows), 'requests': verify(rows)}


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('directory', type=Path)
    args = ap.parse_args()
    print(json.dumps(summarize(args.directory), ensure_ascii=False, indent=2))
