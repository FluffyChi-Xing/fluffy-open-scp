"""Offline elementary-school ERZ schedule evidence; never opens a process.

Uses the existing prop_dump executable against an explicitly selected package.
Property ancestry is a reference graph, NOT a reproduction of bundle merging,
exclusions, specialization, or addon precedence. Raw property dumps are retained.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess

from unit_rule_resource_bridge import decode
from resolve_ids import Reader

DEFAULT_SIM = struct.pack('<3i3I6ffIiff', 0, 16, 0, 0, 0, 0,
                          0, 0, 0, 0, 0, 0, -1, 0, 0, 0, 1)


def sim_info(pool, offset):
    if offset == -1:
        raw = DEFAULT_SIM
    elif offset < 0 or offset + 68 > len(pool):
        raise ValueError('sim_info_bounds')
    else:
        raw = pool[offset:offset + 68]
    v = struct.unpack('<3i3I6ffIiff', raw)
    return dict(offset=offset, default=offset == -1, rawHex=raw.hex(),
                priority=v[0], rate=v[1], counterOffset=v[2],
                timeWindows=[dict(periodId=f'0x{v[3+i]:08x}',
                                  start=v[6+2*i], end=v[7+2*i]) for i in range(3)],
                triggerTime=v[12], triggerPeriodId=f'0x{v[13]:08x}',
                triggerCount=v[14], triggerVariance=v[15], chance=v[16])


def properties(text):
    result = {}
    for line in text.splitlines():
        m = re.match(r'0x([0-9A-F]{8})\s+(.+?)\s+= (.*)', line)
        if m:
            result.setdefault(int(m[1], 16), []).append(m[3])
    return result


def keys(values):
    for value in values:
        m = re.fullmatch(r'T ([0-9A-F]{8}) - G ([0-9A-F]{8}) - I ([0-9A-F]{8})', value)
        if m:
            yield tuple(int(x, 16) for x in m.groups())


def unit_prop_sets(data):
    """ERZ outer tables are big endian, including the 20-byte cPropOp."""
    r = Reader(data)
    for tag in (5, 2, 14):
        r.tag(tag)
    if r.take(1) != b'\0':
        raise ValueError('namesIncluded')
    for _ in range(r.u32()):
        r.take(132)
        r.take(r.u32() * 12)
        r.take(8)
    r.tag(5)
    r.take(r.u32() * 72)
    r.tag(8)
    r.take(r.u32() * 104)
    r.tag(2)
    for _ in range(r.u32()):
        r.take(21)
        for size in (12, 12, 8):
            r.take(r.u32() * size)
    mapping = dict(struct.unpack('>II', r.take(8)) for _ in range(r.u32()))
    sets = []
    for _ in range(r.u32()):
        ops = []
        for _ in range(r.u32()):
            offset = r.offset
            prop, mul, add, d32, d16, d8, op = struct.unpack('>IffIHBB', r.take(20))
            ops.append(dict(fileOffset=offset, propertyId=f'0x{prop:08x}', multiplier=mul,
                            addend=add, data32=d32, data16=d16, data8=d8, opId=op))
        sets.append(ops)
    if any(i >= len(sets) for i in mapping.values()):
        raise ValueError('prop_set_index')
    return {rid: sets[i] for rid, i in mapping.items()}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--package', type=Path, required=True)
    ap.add_argument('--binary', type=Path, required=True)
    ap.add_argument('--prop-dump', type=Path, default=Path('target/debug/examples/prop_dump.exe'))
    ap.add_argument('--ui', type=Path, default=Path('tmp/installed-school-game-ui.js'))
    ap.add_argument('--snapshots', type=Path, default=Path('tmp/school-morning-watch-20261023/snapshots.jsonl'))
    ap.add_argument('--trace', type=Path, action='append', default=[])
    ap.add_argument('--exe', type=Path, help='Optional known retail PE for bounded offline disassembly')
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    cache, missing, edges = {}, [], []

    def read_prop(group, instance):
        key = (group, instance)
        if key in cache:
            return cache[key]
        proc = subprocess.run([str(args.prop_dump.resolve()), str(args.package.resolve()),
                               '00b1b104', f'{group:08x}', f'{instance:08x}'],
                              capture_output=True, encoding='utf8', check=False)
        if proc.returncode:
            if 'not found' not in proc.stderr:
                raise RuntimeError(proc.stderr)
            missing.append([f'{group:08x}', f'{instance:08x}'])
            cache[key] = {}
        else:
            (args.out / f'property-{group:08x}-{instance:08x}.txt').write_text(proc.stdout, encoding='utf8')
            cache[key] = properties(proc.stdout)
        return cache[key]

    clock = read_prop(0x40800200, 0x622B9CD7)
    tick_rate = float(clock[0x07E0AC04][0])
    period_ids = list(keys(clock[0x083950BC]))
    lengths, phases = clock[0x083950BD], clock[0x083950BE]
    assert len(period_ids) == len(lengths) == len(phases)
    periods = [dict(id=f'0x{k[2]:08x}', seconds=float(length),
                    ticks=tick_rate * float(length), phase=float(phase))
               for k, length, phase in zip(period_ids, lengths, phases)]
    pending, visited = [(0x40E0C000, 0xC264A268)], set()
    while pending:
        key = pending.pop()
        if key in visited:
            continue
        if len(visited) >= 256:
            raise ValueError('property_graph_limit')
        visited.add(key)
        p = read_prop(*key)
        for prop in (0x00B2CCCB, 0x0A0209A3, 0x0A0209A4, 0x0A0209A5):
            for ty, group, instance in keys(p.get(prop, [])):
                dest = (group, instance) if prop == 0x00B2CCCB else (0x40E0C700, instance)
                edges.append(dict(source=[f'{x:08x}' for x in key], property=f'{prop:08x}',
                                  target=[f'{x:08x}' for x in dest]))
                pending.append(dest)

    data = args.binary.read_bytes()
    prop_sets = unit_prop_sets(data)
    reader = Reader(data)
    reader.take(13)
    ids = set()
    for _ in range(reader.u32()):
        ids.add(reader.u32())
        reader.take(128)
        reader.take(reader.u32() * 12)
        reader.take(8)
    decoded = decode(data, ids)
    by_id = {m['ruleId']: m for m in decoded['matches'] if m['scope'] == 'unit'}
    pool = data[decoded['poolOffset']:decoded['poolOffset'] + decoded['poolSize']]
    names = {int(v): k for k, v in re.findall(r'simcity\.(kResourceID\w+)=(\d+)', args.ui.read_text(encoding='utf8'))}
    refs = {}
    for (group, instance), p in cache.items():
        if (group, instance) not in visited:
            continue
        for prop, values in p.items():
            for _, _, rid in keys(values):
                if rid in by_id:
                    refs.setdefault(rid, []).append(dict(group=f'{group:08x}', instance=f'{instance:08x}',
                                                         property=f'{prop:08x}'))
    # Explicit comparison rules are not asserted to be bound to the school.
    comparison = {0x830BD003, 0xD51CB94C, 0x68615EA7, 0xD54F8B18, 0xB78DD3BE}
    traces = []
    for path in args.trace:
        events = [json.loads(line) for line in path.read_text(encoding='utf8').splitlines()]
        traces.append((path, events))
    observed_ids = {int(e.get('ruleId') or e.get('mapping', {}).get('ruleIdHex'), 16)
                    for _, events in traces for e in events
                    if e.get('ruleId') or e.get('mapping', {}).get('ruleIdHex')}
    selected = set(refs) | comparison | {0xB52C889D} | observed_ids
    # Only the primary Unit property ancestry is used to evaluate float inputs.
    # Bundle property merges and connected module aggregation remain unmodeled.
    effective_props = {}
    key, ancestry = (0x40E0C000, 0xC264A268), set()
    while key not in ancestry:
        ancestry.add(key)
        p = cache[key]
        for prop, values in p.items():
            effective_props.setdefault(prop, (key, values))
        parents = list(keys(p.get(0x00B2CCCB, [])))
        if not parents:
            break
        key = parents[0][1:]
    rows = []
    for rid in sorted(selected):
        row = by_id[rid]
        words = struct.unpack_from('>33i', data, row['offset'])
        row['propertyReferences'] = refs.get(rid, [])
        row['selection'] = 'property-reference-candidate' if rid in refs else 'comparison-or-prior-trace'
        row['simInfo'] = sim_info(pool, words[2])
        row['propertySpecializations'] = prop_sets.get(rid, [])
        for op in row['propertySpecializations']:
            prop = int(op['propertyId'], 16)
            if prop in effective_props:
                source, values = effective_props[prop]
                op['unitPropertySource'] = [f'{x:08x}' for x in source]
                op['unitPropertyValues'] = values
                if len(values) == 1:
                    try:
                        op['evaluatedFloat'] = float(values[0]) * op['multiplier'] + op['addend']
                    except ValueError:
                        pass
        row['rawHeaderWords'] = [f'{w & 0xffffffff:08x}' for w in words]
        for ops in row['resourcePrefix'].values():
            for op in ops['values']:
                op['symbol'] = names.get(int(op['resourceId'], 16))
        for op in row['fromBinOps']['values']:
            op['sourceSymbol'] = names.get(int(op['sourceResourceId'], 16))
        rows.append(row)
    snapshots = [json.loads(line) for line in args.snapshots.read_text().splitlines()]
    day = next(p for p in periods if p['id'] == '0x12952639')

    def predicted_clock(tick):
        # Configuration-derived only; not an observed HUD value. Avoid truncating
        # 07:59:00 to 07:58 due to stored single-precision phase approximation.
        seconds = ((tick / day['ticks'] + day['phase']) % 1) * 86400
        nearest = round(seconds) % 86400
        return f'{nearest // 3600:02d}:{nearest // 60 % 60:02d}:{nearest % 60:02d}'

    changes, previous = [], {}
    watched = {303, 729, 1757, 3547, 3573, 1117, 5526, 675, 5501, 1910, 2293, 4512}
    for s in snapshots:
        for b in s['bins']:
            i, amount = b['resourceIndex'], b['amount']
            if i in watched and i in previous and previous[i] != amount:
                changes.append(dict(utc=s['utc'], tickBefore=s['tickBefore'], tickAfter=s['tickAfter'],
                                    predictedClock=predicted_clock(s['tickBefore']), resourceIndex=i,
                                    before=previous[i], after=amount, sameTick=s['sameTick']))
            previous[i] = amount
    trace_timings = []
    for path, events in traces:
        grouped = {}
        for e in events:
            rid = e.get('ruleId') or e.get('mapping', {}).get('ruleIdHex')
            if not rid or 'simTicks' not in e:
                continue
            # Keep sessions, games, units and channels separate.
            key = (e.get('game'), e.get('unitSlot'), e.get('channel'), rid)
            group = grouped.setdefault(key, dict(game=key[0], unitSlot=key[1], channel=key[2],
                                                 ruleId=rid, count=0, ticks={}))
            group['count'] += 1
            tick = e['simTicks']
            group['ticks'][tick] = group['ticks'].get(tick, 0) + 1
        for g in grouped.values():
            g['ticks'] = [dict(tick=t, count=n, predictedClock=predicted_clock(t))
                          for t, n in sorted(g['ticks'].items())]
        trace_timings.append(dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
                                  groups=list(grouped.values())))
    report = dict(schema=1, evidenceKind='offline-school-schedule-candidates',
                  binarySha256=decoded['sha256'], package=str(args.package.resolve()),
                  packageSha256=hashlib.sha256(args.package.read_bytes()).hexdigest(),
                  unitCount=decoded['unitCount'], globalCount=decoded['globalCount'],
                  tickRate=tick_rate, periods=periods, propertyEdges=edges, missingProperties=missing,
                  propertyGraphCount=len(visited), rules=rows, changes=changes, traceTimings=trace_timings,
                  snapshotWindow=dict(firstTick=snapshots[0]['tickBefore'], lastTick=snapshots[-1]['tickAfter'],
                                      predictedFirst=predicted_clock(snapshots[0]['tickBefore']),
                                      predictedLast=predicted_clock(snapshots[-1]['tickAfter'])),
                  limitations=['Not original ER2 text; namesIncluded=false',
                               'Reference graph does not implement exclusions, merge semantics or addons',
                               'Base SimInfo may be specialized from property values',
                               'Predicted clocks are not a synchronized HUD measurement',
                               'Snapshots do not attribute changes to particular rules'])
    if args.exe:
        from package_loader_probe import PE
        from capstone import Cs, CS_ARCH_X86, CS_MODE_32
        pe = PE(args.exe)
        if pe.sha256 != '2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab':
            raise ValueError('schedule spans require the verified retail build')
        report['retailEvidence'] = dict(path=str(args.exe.resolve()), sha256=pe.sha256, spans=[])
        for name, va, length in [('SimInfo.constructor', 0x5FBC10, 0x90),
                                 ('Game.SetTimePeriodsFromSimTicks', 0x5CF4D0, 0xBD),
                                 ('UnitRule.Read', 0x5FF8D0, 0x2DB),
                                 ('RulePropSet.Read', 0x5FF840, 0x90)]:
            offset, raw = pe.read(va - pe.base, length)
            asm = '\n'.join(f'{i.address:08x}: {i.mnemonic} {i.op_str}'
                            for i in Cs(CS_ARCH_X86, CS_MODE_32).disasm(raw, va))
            (args.out / f'{name}.asm.txt').write_text(asm + '\n', encoding='utf8')
            report['retailEvidence']['spans'].append(dict(name=name, va=hex(va), length=length,
                                                         fileOffset=offset, sha256=hashlib.sha256(raw).hexdigest()))
    (args.out / 'schedule.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf8')
    print(json.dumps(dict(properties=len(visited), rules=len(rows), missing=len(missing),
                          snapshotWindow=report['snapshotWindow']), indent=2))


if __name__ == '__main__':
    main()
