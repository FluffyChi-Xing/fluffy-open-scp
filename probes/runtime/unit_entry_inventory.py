"""Passively map runtime UnitEntry indices to loaded UnitEntry IDs.

This observes cEcoGameScripts::UnitEntryFromIndex while the game is running.
It does not call the function; on return it reads the returned entry's mID
(entry + 0x08) and aggregates index -> IDs for the observation window.
"""
from __future__ import annotations
import argparse, hashlib, json, time
from contextlib import ExitStack
from pathlib import Path
import frida
from capture_lock import acquire_capture_lock, release_capture_lock

RETAIL_SHA = "2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab"
RVA = 0x21CA70

def main():
    acquire_capture_lock()
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--pid', type=int, required=True)
    ap.add_argument('--seconds', type=int, choices=range(10, 31), default=15)
    ap.add_argument('--out', type=Path)
    args = ap.parse_args()
    agent = r'''
'use strict';
const mod = Process.getModuleByName('SimCity.exe');
const rows = new Map(); let calls = 0, errors = 0;
function arm() { Interceptor.attach(mod.base.add(0x21CA70), {
  onEnter(args) { this.index = this.context.esp.add(4).readS32(); calls++; },
  onLeave(retval) {
    try {
      if (this.index < 0 || retval.isNull()) return;
      const id = retval.add(8).readU32();
      const key = String(this.index), old = rows.get(key);
      if (!old) rows.set(key, {index: this.index, ids: new Set([id])}); else old.ids.add(id);
    } catch (_) { errors++; }
  }
}); Interceptor.flush(); }
rpc.exports = { arm, identity() { return {path: mod.path, arch: Process.arch}; }, snapshot() { return {calls, errors, entries: Array.from(rows.values()).map(x => ({index:x.index, ids:Array.from(x.ids).map(v => '0x'+v.toString(16).padStart(8,'0'))}))}; } };
'''
    with ExitStack() as cleanup:
        cleanup.callback(release_capture_lock)
        device = frida.get_local_device(); session = device.attach(args.pid)
        cleanup.callback(session.detach)
        script = session.create_script(agent, runtime='qjs'); script.load()
        cleanup.callback(script.unload)
        identity = script.exports_sync.identity()
        digest = hashlib.sha256(Path(identity['path']).read_bytes()).hexdigest()
        if digest != RETAIL_SHA or identity['arch'] != 'ia32':
            raise RuntimeError('unsupported_live_build')
        script.exports_sync.arm()
        time.sleep(args.seconds)
        result = script.exports_sync.snapshot()
    result.update({'pid': args.pid, 'seconds': args.seconds, 'rva': hex(RVA), 'sha256': RETAIL_SHA,
                   'observedUtc': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())})
    if args.out: args.out.write_text(json.dumps(result, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
    print(json.dumps(result, ensure_ascii=False, indent=2))
if __name__ == '__main__': main()
