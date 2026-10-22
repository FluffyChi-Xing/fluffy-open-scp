"""External read-only school/clock watch. No injection, hooks or remote calls.

Rule state bytes and the separate scheduler SimStates are captured separately.
Stops on identity changes; bounded by wall time and simulated tick advance.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import struct
import time

from school_resource_watch import Reader, EXE_HASH, vector
from school_rule_schedule import sim_info

TARGETS = {0xD51CB94C, 0xD54F8B18, 0x23E737BD, 0xB52C889D}


def validate_core_binding(core_indices, current_indices, state_count):
    if len(current_indices) % 4 or not current_indices.startswith(core_indices):
        raise ValueError('core_rule_binding_changed')
    if state_count != len(current_indices)//4:
        raise ValueError('rule_vectors_in_transition')


def rule_pointer(reader, rule_set, index):
    a = struct.unpack('<4I', reader.read(rule_set + 0x20, 16))
    b = struct.unpack('<4I', reader.read(rule_set + 0x30, 16))
    for current, begin, end, node in (a, b):
        if begin < 0x10000 or node < 0x10000 or end - begin != 624 or not 0 <= current - begin < 624 or (current - begin) % 156:
            raise ValueError('deque_bounds')
    distance = b[3] - a[3]
    if distance < 0 or distance % 4 or distance > 1000000:
        raise ValueError('deque_map_bounds')
    first = (a[0] - a[1]) // 156
    count = distance + (b[0] - b[1]) // 156 - first
    if not 0 <= index < count:
        raise ValueError('rule_index_bounds')
    pos = first + index
    return reader.u32(a[3] + pos // 4 * 4) + pos % 4 * 156


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--pid', type=int, required=True)
    ap.add_argument('--seed', type=Path, required=True, help='Prior external watch status.json')
    ap.add_argument('--out', type=Path, required=True)
    ap.add_argument('--seconds', type=int, default=600, choices=range(1, 901))
    ap.add_argument('--ticks', type=int, default=180, choices=range(1, 361))
    args = ap.parse_args()
    prior = json.loads(args.seed.read_text())
    if prior['pid'] != args.pid:
        raise ValueError('seed_pid_mismatch')
    game = int(prior['game'], 16)
    args.out.mkdir(parents=True, exist_ok=False)
    status = dict(pid=args.pid, game=hex(game), mode='external-read-only', intervalSeconds=0.5,
                  samples=0, errors=[], requestedTicks=args.ticks, requestedSeconds=args.seconds,
                  startedUtc=datetime.now(timezone.utc).isoformat(), userClockAnchor=None,
                  clockSource='live period interpolation; no synchronized HUD anchor')
    reader = Reader(args.pid)
    try:
        module = next(m for m in reader.modules if m['name'].lower() == 'simcity.exe')
        status['exeSha256'] = hashlib.sha256(Path(module['path']).read_bytes()).hexdigest()
        if status['exeSha256'] != EXE_HASH:
            raise ValueError('unsupported_exe')
        scripts = reader.u32(game + 16)
        if reader.u32(reader.u32(scripts) + 0xd0) != module['base'] + 0x1f76b0:
            raise ValueError('scripts_identity')
        ub, ue, _ = vector(reader, game + 0x554, 0x380, 100000)
        matches = [i for i in range((ue - ub) // 0x380) if reader.u32(ub + i * 0x380 + 12) == 0xc264a268]
        if len(matches) != 1:
            raise ValueError('ambiguous_school')
        slot = matches[0]
        unit = ub + slot * 0x380
        status.update(unitSlot=slot, unitPointer=hex(unit), scripts=hex(scripts))
        # Keep the actual current resource index map, never reuse another session's indexes.
        rb, re, _ = vector(reader, scripts + 0x90, 8, 20000)
        resource_map = list(struct.iter_unpack('<Ii', reader.read(rb, re - rb)))
        if len({x[0] for x in resource_map}) != len(resource_map) or len({x[1] for x in resource_map}) != len(resource_map):
            raise ValueError('resource_map_duplicates')
        (args.out / 'resource-map.json').write_text(json.dumps(resource_map), encoding='utf8')
        ib, ie, indices_header = vector(reader, unit + 0x1cc, 4, 10000)
        indices_raw = reader.read(ib, ie - ib)
        setb, sete, _ = vector(reader, scripts + 0x5c, 4, 100)
        mappings = []
        for state_index, (index,) in enumerate(struct.iter_unpack('<I', indices_raw)):
            set_index, local = index >> 20, index & 0xfffff
            if set_index >= (sete - setb) // 4:
                raise ValueError('set_index')
            rule_set = reader.u32(setb + set_index * 4)
            rule = rule_pointer(reader, rule_set, local)
            rid = reader.u32(rule)
            if rid not in TARGETS:
                continue
            raw = reader.read(rule, 156)
            offset = struct.unpack_from('<i', raw, 8)[0]
            pb, pe, _ = vector(reader, rule_set + 4, 1, 67108864)
            if offset != -1 and not 0 <= offset <= pe - pb - 68:
                raise ValueError('siminfo_bounds')
            raw_sim = reader.read(pb + offset, 68) if offset >= 0 else b''
            info = sim_info(raw_sim, 0 if offset >= 0 else -1)
            info['originalPoolOffset'] = offset
            mappings.append(dict(stateIndex=state_index, ruleIndex=index, ruleId=f'0x{rid:08x}',
                                 rulePointer=hex(rule), ruleSet=hex(rule_set), simInfo=info, rawRuleHex=raw.hex()))
        status['ruleMappings'] = mappings
        stateb, statee, states_header = vector(reader, unit + 0x1dc, 12, 10000)
        if (statee - stateb) // 12 != len(indices_raw) // 4:
            raise ValueError('state_count_mismatch')
        simb, sime, sim_header = vector(reader, unit + 0x1bc, 28, 10000)
        sim_count = (sime-simb)//28
        if sim_count > len(indices_raw)//4:
            raise ValueError('sim_state_count')
        status['simStateCount'] = sim_count
        core_indices = indices_raw[:sim_count*4]
        # Event/sink rules append transient states during normal play. Track
        # core scheduled rules only; array growth is not an identity change.
        mappings = [m for m in mappings if m['stateIndex'] < sim_count]
        status['watchedCoreRules'] = [m['ruleId'] for m in mappings]
        status['bindingResizes'] = 0
        first_tick = reader.u32(game + 0x17a0)
        status['firstTick'] = first_tick
        deadline = time.monotonic() + args.seconds
        started = time.monotonic()
        with (args.out / 'snapshots.jsonl').open('w', encoding='utf8') as out:
            while time.monotonic() < deadline:
                if (args.out/'stop.request').exists():
                    status['stopReason']='stop_file_requested'
                    break
                tick = reader.u32(game + 0x17a0)
                ub, ue, unit_header = vector(reader, game + 0x554, 0x380, 100000)
                if ub + slot * 0x380 != unit or unit >= ue or reader.u32(unit + 12) != 0xc264a268 or reader.u32(game + 16) != scripts:
                    raise ValueError('identity_changed')
                bb, be, bin_header = vector(reader, unit + 0x210, 12, 65535)
                bins = [dict(resourceIndex=i, rawLink=l, amount=a, capacity=c)
                        for i, l, a, c in struct.iter_unpack('<hHii', reader.read(bb, be - bb))]
                periods = {}
                period_headers = {}
                for name, offset, fmt in [('ids',0x18a4,'I'), ('inverseLengths',0x18b4,'f'),
                                          ('phases',0x18c4,'f'), ('times',0x18d4,'f'), ('cycles',0x18e4,'f')]:
                    b,e,h = vector(reader,game+offset,4,16)
                    periods[name] = list(struct.unpack('<'+fmt*((e-b)//4),reader.read(b,e-b)))
                    period_headers[offset] = h
                if len({len(v) for v in periods.values()}) != 1:
                    raise ValueError('period_count_mismatch')
                new_ib,new_ie,new_ih = vector(reader,unit+0x1cc,4,10000)
                new_sb,new_se,new_sh = vector(reader,unit+0x1dc,12,10000)
                validate_core_binding(core_indices,reader.read(new_ib,new_ie-new_ib),(new_se-new_sb)//12)
                if new_ih != indices_header or new_sh != states_header:
                    status['bindingResizes'] += 1
                ib,ie,indices_header=new_ib,new_ie,new_ih
                stateb,statee,states_header=new_sb,new_se,new_sh
                states = [dict(stateIndex=m['stateIndex'],ruleId=m['ruleId'],
                               rawHex=reader.read(stateb+m['stateIndex']*12,12).hex()) for m in mappings]
                if reader.read(unit+0x1bc,8) != sim_header:
                    raise ValueError('sim_state_vector_changed')
                sim_states=[]
                for m in mappings:
                    if m['stateIndex']>=sim_count:
                        continue  # sink/event rule has no core scheduler state
                    raw=reader.read(simb+m['stateIndex']*28,28)
                    counter,t0,t1,t2,period,next_time,count=struct.unpack('<5ifi',raw)
                    sim_states.append(dict(stateIndex=m['stateIndex'],ruleId=m['ruleId'],rawHex=raw.hex(),
                                           simCounter=counter,timeIndices=[t0,t1,t2],triggerTimeIndex=period,
                                           nextTriggerTime=next_time,triggerCount=count))
                after = reader.u32(game+0x17a0)
                stable = (reader.read(game+0x554,8)==unit_header and reader.read(unit+0x210,8)==bin_header
                          and reader.u32(unit+12)==0xc264a268
                          and reader.read(unit+0x1cc,8)==indices_header and reader.read(unit+0x1dc,8)==states_header
                          and reader.read(unit+0x1bc,8)==sim_header
                          and all(reader.read(game+o,8)==h for o,h in period_headers.items()))
                row = dict(utc=datetime.now(timezone.utc).isoformat(), elapsedSeconds=time.monotonic()-started,
                           tickBefore=tick,tickAfter=after,sameTick=tick==after,stableIdentity=stable,
                           unitSlot=slot,periods=periods,bins=bins,ruleStates=states,simStates=sim_states)
                out.write(json.dumps(row)+'\n');out.flush()
                status['samples']+=1
                status['lastTick']=after
                if not stable or after < first_tick:
                    raise ValueError('unstable_or_rewound')
                if status['samples']==1 or status['samples']%40==0:
                    print(json.dumps(dict(samples=status['samples'],tick=after,advance=after-first_tick,
                                          dayFraction=periods['times'][periods['ids'].index(0x12952639)])),flush=True)
                    (args.out/'status.json').write_text(json.dumps(status,indent=2),encoding='utf8')
                if after-first_tick>=args.ticks:
                    status['stopReason']='tick_target_reached';break
                time.sleep(min(.5,max(0,deadline-time.monotonic())))
            else:
                status['stopReason']='wall_time_limit'
    except KeyboardInterrupt:
        status['stopReason']='keyboard_interrupt'
    except Exception as exc:
        status['errors'].append(str(exc));status['stopReason']='validation_error'
    finally:
        reader.close()
        status['finishedUtc']=datetime.now(timezone.utc).isoformat()
        (args.out/'status.json').write_text(json.dumps(status,indent=2),encoding='utf8')
        print(json.dumps(dict(samples=status['samples'],errors=status['errors'],stopReason=status.get('stopReason'))),flush=True)
    return int(bool(status['errors']))


if __name__=='__main__':
    raise SystemExit(main())
