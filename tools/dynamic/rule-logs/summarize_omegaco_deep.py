import json
import collections
from pathlib import Path

source = Path('tmp/omegaco-deep-20261023/consumption-60s/trace.jsonl')
by_resource = collections.defaultdict(lambda: {'records': 0, 'delta': 0, 'input': 0, 'output': 0, 'slots': set(), 'rules': set()})
by_rule = collections.defaultdict(lambda: {'operations': 0, 'delta': 0, 'input': 0, 'output': 0, 'resources': set(), 'slots': set()})
rows = [json.loads(line) for line in source.read_text(encoding='utf-8').splitlines()]
for row in rows:
    for op in row.get('operations', []):
        rid = op.get('resourceId')
        rule = row.get('ruleId')
        r = by_resource[rid]
        r['records'] += 1; r['delta'] += op.get('delta', 0) or 0
        r['input'] += op.get('input', 0) or 0; r['output'] += op.get('output', 0) or 0
        r['slots'].add(row.get('unitSlot')); r['rules'].add(rule)
        q = by_rule[rule]
        q['operations'] += 1; q['delta'] += op.get('delta', 0) or 0
        q['input'] += op.get('input', 0) or 0; q['output'] += op.get('output', 0) or 0
        q['resources'].add(rid); q['slots'].add(row.get('unitSlot'))
for value in list(by_resource.values()) + list(by_rule.values()):
    for key in ('slots', 'rules', 'resources'):
        if key in value:
            value[key] = sorted(value[key])
result = {
    'source': str(source),
    'records': len(rows),
    'resourceSummary': dict(sorted(by_resource.items(), key=lambda item: item[1]['records'], reverse=True)),
    'ruleSummary': dict(sorted(by_rule.items(), key=lambda item: item[1]['operations'], reverse=True)),
    'interpretation': {
        'confirmedUtilityConsumption': {
            'water': '0x9e3c3dfa, delta -1008 per observed event',
            'power': '0x8e8b4e10, delta -9495 per observed event',
        },
        'confirmedRefillOrTransfer': {
            'rawOre': '0x96cf3b07, positive writes to capacity 60000',
            'crudeOil': '0xbf75dc04, positive writes to capacity 60000',
            'processor': '0xbee1c73e, no non-zero delta in this unit-consumption window',
        },
        'unresolved': ['0x415586b3', '0xd3c17458', '0x11a2434e', '0x8e25d7fb', '0xf40144d9', '0xce415e6a'],
    },
}
Path('tmp/omegaco-deep-analysis-20261023.json').write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
