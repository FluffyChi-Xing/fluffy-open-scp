#!/usr/bin/env python3
"""Bounded, read-only tracing of retail GB::cDeltaStream calls."""
from __future__ import annotations

import json
import pathlib
import sys
import time

import frida


PID = int(sys.argv[1]) if len(sys.argv) > 1 else 46580
DURATION = int(sys.argv[2]) if len(sys.argv) > 2 else 120
OUT = pathlib.Path("tmp/dynamic/live/delta-stream")
OUT.mkdir(parents=True, exist_ok=True)
RUN_FILE = OUT / f"calls-{int(time.time())}.jsonl"

JS = r'''
const base = Process.findModuleByName('SimCity.exe').base;
// Runtime vtable is 0xd1aafc with the observed 0x400000 module base.
const vft = base.add(0x91aafc);
const slots = [0, 1, 2, 6, 7, 21, 22, 23];
const seen = new Set();
let count = 0;
function u32(p) { try { return p.readU32(); } catch (_) { return null; } }
function ptrAt(p, n) { try { return p.add(n).readPointer().toString(); } catch (_) { return null; } }
function emit(slot, self, phase, returnAddress) {
  // The first slots are shared AddRef/Release implementations. Only retain
  // calls made by an actual retail cDeltaStream object.
  if (ptrAt(self, 0) !== '0xd1aafc') return;
  if (count++ >= 20000) return;
  send({kind:'call', seq:count, slot, self:self.toString(), phase,
    returnAddress,
    vft:ptrAt(self,0), f0:ptrAt(self,4), f1:u32(self.add(8)), f2:u32(self.add(12))});
}
for (const slot of slots) {
  let fn; try { fn = vft.add(slot * 4).readPointer(); } catch (_) { continue; }
  if (fn.isNull() || fn.compare(base) < 0 || fn.compare(base.add(Process.findModuleByName('SimCity.exe').size)) >= 0) continue;
  Interceptor.attach(fn, { onEnter(args) {
    const self = this.context.ecx;
    emit(slot, self, u32(self.add(0x10)), this.returnAddress.toString());
  }});
}
send({kind:'ready', vft:vft.toString(), slots});
'''


def on_message(message, data):
    if message.get("type") != "send":
        print(message.get("description", message))
        return
    payload = message["payload"]
    if payload.get("kind") == "call":
        with RUN_FILE.open("a", encoding="utf-8") as fh:
            fh.write(json.dumps(payload) + "\n")
        if payload["seq"] % 100 == 0:
            print(json.dumps(payload))
    else:
        print(json.dumps(payload))


device = frida.get_local_device()
session = device.attach(PID)
script = session.create_script(JS)
script.on("message", on_message)
script.load()
print(f"capturing cDeltaStream PID {PID} for {DURATION}s; Ctrl+C to stop")
try:
    time.sleep(DURATION)
except KeyboardInterrupt:
    pass
finally:
    session.detach()
