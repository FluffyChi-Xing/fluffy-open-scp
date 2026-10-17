"""Capture bounded rule entries, resource provenance, effects or eligibility.

No game functions are called. Each scope observes its documented native path,
not all attempts or successful persistence. Interceptor changes entry instructions.
Provenance supports attach or an explicit cold launch, leaving the game running.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import sys
import threading
import time

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from package_loader_probe import PE

HOOKS = {"global": 0x1CFE20, "unit": 0x1D47F0}
PROVENANCE_HOOKS = {'request': 0x511760, 'cache': 0x511110, 'record': 0x4EA530, 'create': 0x1C9980}
EFFECT_HOOKS = {'globalRun': 0x218890, 'applyOps': 0x206580, 'binFind': 0x18EC70}
UNIT_EFFECT_HOOKS = {'unitRun': 0x21DB00, 'applyOps': 0x206580, 'binFind': 0x18EC70}
CONDITION_HOOKS = {'globalCheck': 0x218660, 'resourceCount': 0x206370, 'binFind': 0x18EC70}
RETAIL_SHA = "2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab"


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    target = ap.add_mutually_exclusive_group(required=True)
    target.add_argument('--pid', type=int)
    target.add_argument('--launch-exe', type=Path, help='Start the specified build with hooks installed before resume')
    ap.add_argument('--out', type=Path, required=True)
    ap.add_argument('--seconds', type=int, default=20)
    ap.add_argument('--scope', choices=['global', 'unit', 'both', 'provenance', 'effects', 'unit-effects', 'unit-consumption', 'conditions'], default='global')
    ap.add_argument('--rule-id', type=lambda s: int(s, 0), help='Filter uint32 rule ID for rule/effects/conditions scopes')
    ap.add_argument('--resource-id', type=lambda s: int(s, 0), help='Effects scopes: filter operation resource ID')
    ap.add_argument('--state-index', type=int)
    ap.add_argument('--unit-slot', type=int)
    ap.add_argument('--max-records', type=int, default=250)
    ap.add_argument('--all-targets', action='store_true', help='Effects scopes: observe multiple units/rules until max-records')
    args = ap.parse_args()
    if not 1 <= args.seconds <= 120 or not 1 <= args.max_records <= 2000 or (args.pid is not None and args.pid <= 0):
        ap.error('pid must be positive; seconds 1..120; max-records 1..2000')
    if any(v is not None and v < 0 for v in [args.state_index, args.unit_slot]):
        ap.error('indices must be non-negative')
    if args.unit_slot is not None and args.scope == 'global':
        ap.error('unit-slot requires unit or both scope')
    if args.scope == 'provenance' and (args.unit_slot is not None or args.state_index is not None):
        ap.error('provenance observes rule resource loads, not rule state/slot indices')
    if args.scope in ['effects', 'conditions'] and (args.unit_slot is not None or args.state_index is not None):
        ap.error('effects/conditions use rule-id, not state-index or unit-slot')
    if args.scope in ['unit-effects', 'unit-consumption'] and args.state_index is not None:
        ap.error('unit-effects uses rule-id/unit-slot, not state-index')
    if args.rule_id is not None and (args.scope not in ['global', 'unit', 'both', 'effects', 'unit-effects', 'unit-consumption', 'conditions'] or not 0 <= args.rule_id <= 0xffffffff):
        ap.error('rule-id requires a rule, effects or conditions scope and a uint32 value')
    if args.resource_id is not None and (args.scope not in ['effects', 'unit-effects', 'unit-consumption'] or not 0 <= args.resource_id <= 0xffffffff):
        ap.error('resource-id requires effects/unit-effects/unit-consumption scope and a uint32 value')
    if args.all_targets and args.scope not in ['effects', 'unit-effects', 'unit-consumption']:
        ap.error('all-targets requires effects, unit-effects or unit-consumption scope')
    if args.launch_exe and args.scope != 'provenance':
        ap.error('launch-exe is only supported for provenance capture')
    import frida
    args.out.mkdir(parents=True, exist_ok=False)
    agent_name = {'provenance': 'provenance_agent.js', 'effects': 'effects_agent.js', 'unit-effects': 'effects_agent.js', 'unit-consumption': 'effects_agent.js', 'conditions': 'conditions_agent.js'}.get(args.scope, 'rule_agent.js')
    agent = Path(__file__).with_name(agent_name).read_text(encoding='utf-8')
    status = {'pid': args.pid, 'startedUtc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
              'seconds': args.seconds, 'scope': args.scope, 'records': 0, 'ready': False,
              'detached': False, 'errors': [], 'fridaVersion': frida.__version__,
              'agentSha256': hashlib.sha256(agent.encode()).hexdigest(),
              'sampling': 'rule resource requests, capped' if args.scope == 'provenance' else 'first matching game/slot/stateIndex per scope, then capped'}
    if args.scope == 'effects':
        status['sampling'] = 'first matching global game/rule, <=16 effective operations, capped; snapshots around native writes'
    if args.scope == 'unit-effects':
        status['sampling'] = 'first matching game/unit slot/pointer/rule; unit-local bins only; <=16 effective operations, capped; stamp unknown'
    if args.scope == 'unit-consumption':
        status['sampling'] = 'first unit-local ApplyBinRule with input > 0; unit-local bins only; <=16 effective operations, capped; stamp unknown'
        if args.all_targets:
            status['sampling'] = 'all unit-local ApplyBinRule targets with input > 0 until max-records; unit-local bins only; <=16 operations; stamp unknown'
    if args.scope == 'conditions':
        status['sampling'] = 'first matching game, optional rule ID; capped eligibility checks; <=16 effective operations per count check'
    gone = threading.Event()
    session = script = None
    device = None
    launched_pid = None
    lock = threading.Lock()
    trace = (args.out / 'trace.jsonl').open('w', encoding='utf-8')

    def message(m, _data):
        with lock:
            if m['type'] == 'send' and m['payload'].get('kind') == 'batch':
                for row in m['payload']['records']:
                    trace.write(json.dumps(row, ensure_ascii=False) + '\n')
                    status['records'] += 1
                trace.flush()
            elif m['type'] == 'error':
                status['errors'].append(m)

    try:
        device = frida.get_local_device()
        if args.launch_exe:
            launch = PE(args.launch_exe)
            if launch.sha256 != RETAIL_SHA:
                raise ValueError('unsupported_launch_build')
            if any(p.name.lower() == 'simcity.exe' for p in device.enumerate_processes()):
                raise ValueError('game_already_running: save and exit manually before launch capture')
            launched_pid = device.spawn([str(launch.path)], cwd=str(launch.path.parent))
            args.pid = launched_pid
            status.update({'pid': launched_pid, 'spawned': True, 'resumed': False})
        session = device.attach(args.pid)
        session.on('detached', lambda *_: gone.set())
        script = session.create_script(agent, runtime='qjs')
        script.on('message', message)
        script.load()
        identity = script.exports_sync.identity()
        pe = PE(Path(identity['path']))
        if pe.sha256 != RETAIL_SHA or identity['arch'] != 'ia32':
            raise ValueError('unsupported_live_build')
        status.update({'identity': identity, 'sha256': pe.sha256})
        selected_hooks = {'provenance': PROVENANCE_HOOKS, 'effects': EFFECT_HOOKS, 'unit-effects': UNIT_EFFECT_HOOKS, 'unit-consumption': UNIT_EFFECT_HOOKS, 'conditions': CONDITION_HOOKS}.get(args.scope, HOOKS)
        scope_hooks = {
            'effects': ['globalRun', 'applyOps', 'binFind'],
            'unit-effects': ['unitRun', 'applyOps', 'binFind'],
            'unit-consumption': ['unitRun', 'applyOps', 'binFind'],
            'conditions': ['globalCheck', 'resourceCount', 'binFind'],
        }
        kinds = list(selected_hooks) if args.scope in ['both', 'provenance'] else scope_hooks.get(args.scope, [args.scope])
        cfg = {'stateIndex': args.state_index, 'unitSlot': args.unit_slot, 'ruleId': args.rule_id, 'resourceId': args.resource_id, 'requireInputPositive': args.scope == 'unit-consumption', 'allTargets': args.all_targets, 'maxRecords': args.max_records,
               'hooks': {k: {'rva': selected_hooks[k], 'bytes': pe.read(selected_hooks[k], 16)[1].hex()} for k in kinds}}
        (args.out / 'config.json').write_text(json.dumps(cfg, indent=2), encoding='utf-8')
        status['ready'] = script.exports_sync.arm(cfg)
        if launched_pid:
            device.resume(launched_pid)
            status['resumed'] = True
        print(json.dumps({'ready': status['ready'], 'pid': args.pid}), flush=True)
        gone.wait(args.seconds)
    except KeyboardInterrupt:
        status['interrupted'] = True
    except Exception as e:
        status['errors'].append(str(e))
    finally:
        if script and not gone.is_set():
            try:
                summary = script.exports_sync.stop()
                status['summary'] = summary
                status['channels'] = summary['channels']
                status['dropped'] = summary['dropped']
            except Exception as e:
                status['errors'].append(str(e))
            try:
                script.unload()
            except Exception as e:
                status['errors'].append(str(e))
        if session and not gone.is_set():
            try:
                session.detach()
                status['detached'] = True
            except Exception as e:
                status['errors'].append(str(e))
        if launched_pid and not status.get('resumed') and not gone.is_set():
            try:
                device.resume(launched_pid)
                status['resumed'] = True
            except Exception as e:
                status['errors'].append(str(e))
        with lock:
            trace.close()
            (args.out / 'status.json').write_text(json.dumps(status, ensure_ascii=False, indent=2), encoding='utf-8')
        print(json.dumps({'out': str(args.out), 'records': status['records'], 'errors': status['errors']}), flush=True)
    read_errors = status.get('summary', {}).get('errors', 0)
    return 1 if status['errors'] or read_errors else 0


if __name__ == '__main__':
    raise SystemExit(main())
