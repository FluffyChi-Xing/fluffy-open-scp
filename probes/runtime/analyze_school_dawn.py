"""Offline dawn snapshot and installed ERZ resource-writer correlation.

Does not open the game. Candidate rules are not proof of runtime execution.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct

from analyze_school_timed_watch import clock
from school_rule_schedule import sim_info, unit_prop_sets
from unit_rule_resource_bridge import decode
from resolve_ids import Reader, resolve


def analyze(capture, binary, ui, schedule):
    rows = [json.loads(line) for line in (capture / 'snapshots.jsonl').read_text().splitlines()]
    mapping = {idx: rid for rid, idx in json.loads((capture / 'resource-map.json').read_text())}
    names = {int(v): k for k, v in re.findall(r'simcity\.(kResourceID\w+)=(\d+)', ui.read_text(encoding='utf8'))}
    events = []
    for index, (before, after) in enumerate(zip(rows, rows[1:]), 1):
        previous = {b['resourceIndex']: b for b in before['bins']}
        for b in after['bins']:
            old = previous.get(b['resourceIndex'])
            if old is None or (b['amount'], b['capacity']) == (old['amount'], old['capacity']):
                continue
            rid = mapping[b['resourceIndex']]
            events.append(dict(sample=index, resourceId=f'0x{rid:08x}', symbol=names.get(rid),
                               previousTick=before['tickBefore'], observedTick=after['tickAfter'],
                               previousClock=clock(before), observedClock=clock(after),
                               before=old['amount'], after=b['amount'],
                               capacityBefore=old['capacity'], capacityAfter=b['capacity']))
    data = binary.read_bytes()
    layout = resolve(data, set())
    pool = data[layout['poolOffset']:layout['poolOffset'] + layout['poolSize']]
    # Inspect all local operation slots, including those beyond the prefix
    # decoder's 16-operation preview limit. Retain that limit on decoded rows.
    wanted = {0x7CB23A4E, 0x1FD156B6, 0x6CCC3A0F, 0x6CCC3A0C, 0x6CCC3A0D,
              0x06EDFBED, 0x6106B1CB, 0xC906E245}
    selected = set()
    r = Reader(data)
    r.take(13)
    for _ in range(r.u32()):
        offset = r.offset
        rid = r.u32()
        r.take(128)
        r.take(r.u32() * 12)
        r.take(8)
        start, count = struct.unpack_from('>2i', data, offset + 12)
        if count < 0 or (count and (start < 0 or start + count * 12 > len(pool))):
            raise ValueError('local_resource_bounds')
        for n in range(count):
            resource, inp, out = struct.unpack_from('<Iii', pool, start + n * 12)
            if resource in wanted and inp != out:
                selected.add(rid)
    matched = decode(data, selected)
    props = unit_prop_sets(data)
    known = json.loads(schedule.read_text())
    school_refs = {v['ruleId']: v['propertyReferences'] for v in known['rules']}
    for rule in matched['matches']:
        if rule['scope'] != 'unit':
            continue
        rule['simInfo'] = sim_info(pool, struct.unpack_from('>i', data, rule['offset'] + 8)[0])
        rule['propertySpecializations'] = props.get(rule['ruleId'], [])
        rule['schoolReferenceCandidates'] = school_refs.get(rule['ruleId'], [])
        for group in rule['resourcePrefix'].values():
            for op in group['values']:
                op['symbol'] = names.get(int(op['resourceId'], 16))
        for op in rule['fromBinOps']['values']:
            op['sourceSymbol'] = names.get(int(op['sourceResourceId'], 16))
    worker_checks = []
    required = {0x6CCC3A0F, 0x6CCC3A0C, 0x6CCC3A0D, 0x6106B1CB, 0xC906E245, 0x1FD156B6}
    for row in rows:
        amounts = {mapping[b['resourceIndex']]: b['amount'] for b in row['bins']}
        if not required <= amounts.keys():
            raise ValueError('missing_worker_bins')
        worker_checks.append(amounts[0x1FD156B6] == sum(amounts[rid] for rid in required - {0x1FD156B6}))
    sources = [Path(__file__), binary, ui, schedule, capture/'snapshots.jsonl', capture/'resource-map.json', capture/'status.json']
    return dict(evidenceKind='offline-dawn-correlation',
                sources=[dict(path=str(p), sha256=hashlib.sha256(p.read_bytes()).hexdigest()) for p in sources],
                sampleCount=len(rows), firstClock=clock(rows[0]), lastClock=clock(rows[-1]),
                coverage='05:20 onward; 05:00-05:20 absent; observations after 07:00 outside request',
                binChanges=events, candidates=matched,
                workerSnapshotRelation=dict(expression='TotalWorkers = WorkerW1 + WorkerW2 + WorkerW3 + ExtraWorkerW1 + ExtraWorkerW1Paused',
                                            matches=sum(worker_checks), samples=len(rows),
                                            scope='Observed sample relation only; not a universal worker formula'),
                limitations=['Non-atomic snapshots; events fall between previous and observed sample',
                             'Candidate inventory does not prove school binding or rule execution',
                             'Resource operation previews may be truncated; inspect truncated fields',
                             'No HUD calibration; no original ER2 source recovered',
                             'status.userClockAnchor is a stale hardcoded 09:00 label; not current evidence'])


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('capture', type=Path)
    ap.add_argument('--binary', type=Path, default=Path('tmp/live-school-installed-main.bin'))
    ap.add_argument('--ui', type=Path, default=Path('tmp/installed-school-game-ui.js'))
    ap.add_argument('--schedule', type=Path, default=Path('tmp/school-rule-schedule-20261023/schedule.json'))
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    result = analyze(args.capture, args.binary, args.ui, args.schedule)
    args.out.write_text(json.dumps(result, indent=2) + '\n', encoding='utf8')
    print(json.dumps(dict(samples=result['sampleCount'], changes=len(result['binChanges']),
                          candidates=len(result['candidates']['matches']))))
