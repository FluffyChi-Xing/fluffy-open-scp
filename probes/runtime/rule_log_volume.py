"""Measure native EcoGame rule-log entry volume without retaining event rows.

Attach to a running retail SimCity process. This is a volume baseline probe:
it counts calls to LogGlobalRule and LogUnitRule, plus unique game/slot/state
tuples observed during the window. It does not call game functions and does
not write a delta stream.

Usage:
  python probes/runtime/rule_log_volume.py --pid 36688 --seconds 15
"""
from __future__ import annotations

import argparse
from contextlib import ExitStack
import hashlib
import json
import time
from pathlib import Path

import frida
from capture_lock import acquire_capture_lock, release_capture_lock

RETAIL_SHA = "2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab"
UNIT_LOG_RVA = 0x1D47F0
GLOBAL_LOG_RVA = 0x1CFE20


def main() -> None:
    acquire_capture_lock()
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--pid", type=int, required=True)
    ap.add_argument("--seconds", type=int, choices=range(10, 16), default=15)
    ap.add_argument("--out", type=Path)
    args = ap.parse_args()

    agent = r"""
'use strict';
const module = Process.getModuleByName('SimCity.exe');
const base = module.base;
let unitHits = 0, globalHits = 0, errors = 0;
const units = new Set(), globals = new Set(), rules = new Set();
function reset() { unitHits = 0; globalHits = 0; errors = 0; units.clear(); globals.clear(); rules.clear(); }
function i32(p) { return p.readS32(); }
function hook(name, rva) {
  Interceptor.attach(base.add(rva), {
    onEnter(args) {
      try {
        if (name === 'unit') {
          const unitSlot = i32(this.context.esp.add(4));
          const stateIndex = i32(this.context.esp.add(8));
          unitHits++;
          units.add(unitSlot + ':' + stateIndex);
        } else {
          const stateIndex = i32(this.context.esp.add(4));
          globalHits++;
          globals.add(String(stateIndex));
        }
      } catch (_) { errors++; }
    }
  });
}
rpc.exports = { reset,
  arm() { hook('unit', 0x1D47F0); hook('global', 0x1CFE20); Interceptor.flush(); },
  snapshot() { return {unitHits, globalHits, errors, uniqueUnitStates: units.size, uniqueGlobalStates: globals.size, modulePath: module.path, arch: Process.arch}; } };
"""

    with ExitStack() as cleanup:
        cleanup.callback(release_capture_lock)
        device = frida.get_local_device()
        session = device.attach(args.pid)
        cleanup.callback(session.detach)
        script = session.create_script(agent, runtime="qjs")
        script.load()
        cleanup.callback(script.unload)
        identity = script.exports_sync.snapshot()
        module_path = Path(identity["modulePath"])
        digest = hashlib.sha256(module_path.read_bytes()).hexdigest()
        if digest != RETAIL_SHA or identity['arch'] != 'ia32':
            raise RuntimeError(f"unsupported_retail_sha256:{digest}")
        script.exports_sync.arm()
        script.exports_sync.reset()
        time.sleep(args.seconds)
        result = script.exports_sync.snapshot()
    result.update({"pid": args.pid, "seconds": args.seconds,
                   "module": str(module_path) if module_path else None,
                   "sha256": RETAIL_SHA, "observedUtc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())})
    text = json.dumps(result, ensure_ascii=False, indent=2)
    if args.out:
        args.out.write_text(text + "\n", encoding="utf-8")
    print(text)


if __name__ == "__main__":
    main()
