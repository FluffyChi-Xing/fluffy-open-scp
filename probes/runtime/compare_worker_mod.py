"""Offline comparison of extracted community worker UI and saved school bins.

Only parses source text; never evaluates the mod or accesses the game process.
Projected values use one unit's bins, not the mod's connectedUnits aggregation.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re

from analyze_school_timed_watch import clock
from school_rule_schedule import properties


def fingerprint(path):
    return dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest())


def extract_model(source):
    block = source.split('simcity.RolloverItems.kWorkers = {', 1)[1].split(
        'simcity.RolloverItems.kShoppersAndTourists', 1)[0]
    queries = dict(re.findall(
        r'(\w+):\s*\[simcity.kDataConnectedUnits,\s*unitRef,\s*simcity.kDataBinAmount,\s*simcity.(kResourceID\w+)\]', block))
    expressions = {}
    for tier in range(1, 4):
        for field in (f'workerW{tier}Total', f'jobsW{tier}'):
            expr = re.search(r'var\s+' + field + r'\s*=\s*([^;]+);', block).group(1)
            terms = [x.strip() for x in expr.split('+')]
            if not all(re.fullmatch(r'results\.\w+', x) and x[8:] in queries for x in terms):
                raise ValueError('unsupported_mod_formula')
            expressions[field] = [queries[x[8:]] for x in terms]
    return dict(queries=queries, expressions=expressions,
                includesPaused=any('Paused' in v for v in queries.values()),
                includesMinimumWorkers='MinimumWorkersForProduction' in block)


def compare(directory, ui, captures):
    mod_js = directory / '8792243B-58FDB9EE.js'
    source = mod_js.read_text(encoding='utf8')
    names = {k: int(v) for k, v in re.findall(r'simcity\.(k\w+)=(\d+)', ui.read_text(encoding='utf8'))}
    # These three names are mod-authored labels, not native constant evidence.
    mod_names = {k: int(v, 16) for k, v in re.findall(r'simcity\.(kResourceID\w+)\s*=\s*(0x[0-9a-fA-F]+)', source)}
    names.update(mod_names)
    model = extract_model(source)
    baseline = directory / 'SimCity_Game.package.school.txt'
    modified = directory / 'mod-school-prop.txt'
    a, b = properties(baseline.read_text(encoding='utf8')), properties(modified.read_text(encoding='utf8'))
    differences = [dict(propertyId=f'0x{k:08x}', baseline=a.get(k), mod=b.get(k))
                   for k in sorted(a.keys() | b.keys()) if a.get(k) != b.get(k)]
    inventory = re.findall(r'^--- \[\d+\] type=0x(\w+) group=0x(\w+) instance=0x(\w+)',
                           (directory / 'package-dump.txt').read_text(encoding='utf-8-sig'), re.M)
    sources = [ui, mod_js, baseline, modified, directory/'package-dump.txt',
               directory/'ExtendedWorkerDataInRollover.package']
    windows = []
    for capture in captures:
        rows = [json.loads(x) for x in (capture/'snapshots.jsonl').read_text().splitlines()]
        mapping = {i: rid for rid, i in json.loads((capture/'resource-map.json').read_text())}
        snapshots, previous = [], None
        for number, row in enumerate(rows):
            amounts = {mapping[b['resourceIndex']]: b['amount'] for b in row['bins']}
            projected = {key: sum(amounts[names[s]] for s in symbols)
                         for key, symbols in model['expressions'].items()}
            extra = {symbol: amounts.get(names[symbol]) for symbol in (
                'kResourceIDTotalWorkers', 'kResourceIDWorkersNeeded', 'kResourceIDActiveBuilding',
                'kResourceIDExtraWorkerW1Paused')}
            state = dict(projected=projected, resources=extra)
            if state != previous:
                snapshots.append(dict(sample=number, tick=row['tickBefore'], clock=clock(row), **state))
                previous = state
        windows.append(dict(path=str(capture), samples=len(rows), firstClock=clock(rows[0]),
                            lastClock=clock(rows[-1]), projectedChanges=snapshots))
        sources.extend(capture/n for n in ('snapshots.jsonl', 'resource-map.json', 'status.json'))
    return dict(schema=1, evidenceKind='offline-community-worker-mod-comparison',
                sources=[fingerprint(p) for p in sources],
                inventory=dict(entries=len(inventory), types=dict(Counter(r[0] for r in inventory))),
                model=model, modAuthoredSymbols=mod_names, schoolGraphicsDifferences=differences,
                windows=windows, limitations=[
                    'Mod query uses connectedUnits; projections use only school unitSlot 802',
                    'Mod omitted ExtraWorkerPaused; displayed amount is not necessarily TotalWorkers',
                    'No live HUD or mod activation verified; archive existence is not proof of loading',
                    'Graphics property diff compares one TGI to SimCity_Game.package, not every override',
                    'Mod-authored symbols are supporting evidence, not engine definitions'])


if __name__ == '__main__':
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--directory', type=Path, default=Path('tmp/community-worker-mod-20261023'))
    ap.add_argument('--ui', type=Path, default=Path('tmp/installed-school-game-ui.js'))
    ap.add_argument('--capture', action='append', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    result = compare(args.directory, args.ui, args.capture)
    args.out.write_text(json.dumps(result, indent=2)+'\n', encoding='utf8')
    print(json.dumps(dict(inventory=result['inventory'], schoolGraphicsDifferences=result['schoolGraphicsDifferences'],
                         windows=[dict(path=w['path'], samples=w['samples'], changes=len(w['projectedChanges'])) for w in result['windows']]), indent=2))
