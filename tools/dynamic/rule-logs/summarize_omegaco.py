import json, collections, pathlib

root = pathlib.Path('tmp/omegaco-resource-watch-20261023')
rows = [json.loads(x) for x in (root / 'snapshots.jsonl').read_text(encoding='utf-8').splitlines()]
pairs = json.loads((root / 'resource-map.json').read_text())
idx_to_id = {int(i): f'0x{int(h):08x}' for h, i in pairs}
slots = sorted({u['unitSlot'] for r in rows for u in r['units']})
tracked = {
    'raw_ore': '0x96cf3b07', 'crude_oil': '0xbf75dc04', 'processor': '0xbee1c73e',
    'power': '0x8e8b4e10', 'water': '0x9e3c3dfa', 'sewage': '0xce461a05',
    'garbage': '0x7cb23a4e', 'total_workers': '0x1fd156b6',
    'worker_w1': '0x6ccc3a0f', 'workers_needed': '0x5378d6f9',
    'jobs_w1': '0xd3a424b9', 'jobs_w2': '0xd3a424ba', 'jobs_w3': '0xd3a424bb',
}
stats = {str(s): collections.defaultdict(lambda: {'values': []}) for s in slots}
for r in rows:
    for u in r['units']:
        for b in u['bins']:
            rid = idx_to_id.get(b['resourceIndex'])
            if rid:
                stats[str(u['unitSlot'])][rid]['values'].append(b['amount'])
summary = {}
for s, resources in stats.items():
    out = {}
    for rid, d in resources.items():
        vals = d['values']
        if len(set(vals)) <= 1:
            continue
        inc = dec = pos = neg = 0
        for a, b in zip(vals, vals[1:]):
            if b > a:
                inc += 1; pos += b - a
            elif b < a:
                dec += 1; neg += b - a
        out[rid] = {'first': vals[0], 'last': vals[-1], 'min': min(vals), 'max': max(vals),
                    'net': vals[-1] - vals[0], 'increaseEvents': inc, 'decreaseEvents': dec,
                    'sumPositiveDelta': pos, 'sumNegativeDelta': neg}
    summary[s] = out
effects = collections.defaultdict(lambda: {'observations': 0, 'sumDelta': 0, 'sumInput': 0, 'sumOutput': 0, 'slots': set(), 'rules': set()})
for line in pathlib.Path('tmp/omegaco-effects-20261023/trace.jsonl').read_text(encoding='utf-8').splitlines():
    rec = json.loads(line)
    for op in rec.get('operations', []):
        d = effects[op.get('resourceId')]
        d['observations'] += 1; d['sumDelta'] += op.get('delta', 0) or 0
        d['sumInput'] += op.get('input', 0) or 0; d['sumOutput'] += op.get('output', 0) or 0
        d['slots'].add(rec.get('unitSlot')); d['rules'].add(rec.get('ruleId'))
for d in effects.values():
    d['slots'] = sorted(d['slots']); d['rules'] = sorted(d['rules'])
top = sorted(effects.items(), key=lambda kv: kv[1]['observations'], reverse=True)[:25]
result = {
    'capture': json.loads((root / 'status.json').read_text()),
    'resourceMap': {'tracked': tracked, 'indices': {name: next((i for h, i in pairs if int(h) == int(rid, 16)), None) for name, rid in tracked.items()}},
    'slots': slots, 'snapshotSummary': summary, 'effectSummaryTop25': dict(top),
    'limitations': ['snapshot reads are non-atomic', 'snapshot net deltas do not equal total throughput',
                    'effect trace capped at 2000 records with 2272 dropped', 'Omega product hash not statically closed in this capture'],
}
pathlib.Path('tmp/omegaco-analysis-20261023.json').write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
