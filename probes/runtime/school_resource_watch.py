"""Bounded external ReadProcessMemory snapshots; no injection or remote calls.

The seed trace must belong to the selected live PID. Every sample revalidates
the game/unit vector and identity. Snapshots are not atomic or rule events.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import struct
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'tools/dynamic/ui-observer'))
from eawebkit_runtime import Reader, EXE_HASH


def vector(reader, at, stride, limit):
    raw = reader.read(at, 8)
    begin, end = struct.unpack('<II', raw)
    if begin < 0x10000 or end < begin or (end - begin) % stride or (end - begin) // stride > limit:
        raise ValueError('vector_bounds')
    return begin, end, raw


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--pid', required=True, type=int)
    ap.add_argument('--seed', required=True, type=Path)
    ap.add_argument('--out', required=True, type=Path)
    ap.add_argument('--seconds', type=int, default=60, choices=range(1, 121))
    ap.add_argument('--unit-entry', type=lambda v: int(v, 0), default=0xc264a268)
    args = ap.parse_args()
    prior = json.loads((args.seed / 'status.json').read_text(encoding='utf-8'))
    if prior['pid'] != args.pid:
        raise ValueError('seed_pid_mismatch')
    games = {int(json.loads(line)['game'], 16) for line in (args.seed / 'trace.jsonl').read_text().splitlines() if line}
    if len(games) != 1:
        raise ValueError('ambiguous_seed_game')
    game = games.pop()
    args.out.mkdir(parents=True, exist_ok=False)
    status = {'pid': args.pid, 'game': hex(game), 'mode': 'external-read-only', 'intervalSeconds': 0.5,
              'requestedSeconds': args.seconds, 'startedUtc': datetime.now(timezone.utc).isoformat(),
              'samples': 0, 'errors': [], 'limitations': ['non-atomic snapshots', 'net changes, not rule attribution', 'no HUD clock calibration']}
    reader = Reader(args.pid)
    try:
        module = next(m for m in reader.modules if m['name'].lower() == 'simcity.exe')
        status['exe'] = module
        status['sha256'] = hashlib.sha256(Path(module['path']).read_bytes()).hexdigest()
        if status['sha256'] != EXE_HASH:
            raise ValueError('unsupported_exe')
        scripts = reader.u32(game + 0x10)
        vtable = reader.u32(scripts)
        if reader.u32(vtable + 0xd0) != module['base'] + 0x1f76b0:
            raise ValueError('scripts_identity_mismatch')
        start, end, _ = vector(reader, game + 0x554, 0x380, 100000)
        matches = [i for i in range((end - start) // 0x380) if reader.u32(start + i * 0x380 + 12) == args.unit_entry]
        if len(matches) != 1:
            raise ValueError('ambiguous_or_missing_school:' + str(matches))
        slot = matches[0]
        status.update(unitSlot=slot, unitEntryId=hex(args.unit_entry), scripts=hex(scripts), scriptsVtable=hex(vtable))
        status['scriptsHeaderHex'] = reader.read(scripts, 0x120).hex()
        deadline = time.monotonic() + args.seconds
        with (args.out / 'snapshots.jsonl').open('w', encoding='utf-8') as trace:
            while time.monotonic() < deadline:
                before_tick = reader.u32(game + 0x17a0)
                start, end, unit_vector = vector(reader, game + 0x554, 0x380, 100000)
                unit = start + slot * 0x380
                if unit >= end or reader.u32(unit + 12) != args.unit_entry or reader.u32(game + 0x10) != scripts:
                    raise ValueError('target_identity_changed')
                begin, finish, bin_vector = vector(reader, unit + 0x210, 12, 65535)
                raw = reader.read(begin, finish - begin)
                bins = [{'resourceIndex': index, 'rawLink': link, 'amount': amount, 'capacity': capacity}
                        for index, link, amount, capacity in struct.iter_unpack('<hHii', raw)]
                if any(x['resourceIndex'] < 0 for x in bins) or len({x['resourceIndex'] for x in bins}) != len(bins):
                    raise ValueError('invalid_bin_indices')
                after_tick = reader.u32(game + 0x17a0)
                stable = (reader.read(game + 0x554, 8) == unit_vector and
                          reader.read(unit + 0x210, 8) == bin_vector and reader.u32(unit + 12) == args.unit_entry)
                row = {'utc': datetime.now(timezone.utc).isoformat(), 'tickBefore': before_tick, 'tickAfter': after_tick,
                       'sameTick': before_tick == after_tick, 'stableIdentity': stable,
                       'unitSlot': slot, 'unitPointer': hex(unit), 'bins': bins}
                trace.write(json.dumps(row) + '\n'); trace.flush(); status['samples'] += 1
                if not stable:
                    raise ValueError('target_moved_during_read')
                if status['samples'] == 1:
                    print(json.dumps({'ready': True, 'slot': slot, 'tick': before_tick, 'bins': len(bins), 'mode': status['mode']}), flush=True)
                time.sleep(min(0.5, max(0, deadline - time.monotonic())))
    except Exception as exc:
        status['errors'].append(str(exc))
    finally:
        reader.close()
        status['finishedUtc'] = datetime.now(timezone.utc).isoformat()
        (args.out / 'status.json').write_text(json.dumps(status, indent=2), encoding='utf-8')
        print(json.dumps({'samples': status['samples'], 'errors': status['errors']}), flush=True)
    return int(bool(status['errors']))


if __name__ == '__main__':
    raise SystemExit(main())
