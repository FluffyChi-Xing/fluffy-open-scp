#!/usr/bin/env python3
"""Capture retail delta wrapper and XOR writes in one bounded run."""
from __future__ import annotations

import json
import pathlib
import sys
import time

import frida


PID = int(sys.argv[1]) if len(sys.argv) > 1 else 46580
DURATION = int(sys.argv[2]) if len(sys.argv) > 2 else 15
MAX_WRITES = int(sys.argv[3]) if len(sys.argv) > 3 else 2000
OUT = pathlib.Path("tmp/dynamic/live/delta-layers")
OUT.mkdir(parents=True, exist_ok=True)
RUN = int(time.time())

JS = r'''
const base = Process.findModuleByName('SimCity.exe').base;
const wrapperWrite = base.add(0x1c1f70);
const xorWrite = base.add(0x1c6060);
const wrapperVfts = new Set(['0xd1ab5c', '0xd1ab54']);
const xorVfts = new Set(['0xd1ae5c', '0xd1ae54']);
let seq = 0;
function vft(self) { try { return self.readPointer().toString(); } catch (_) { return null; } }
function emit(kind, self, buf, len, ret) {
  const vf = vft(self);
  const allowed = kind === 'wrapper' ? wrapperVfts.has(vf) : xorVfts.has(vf);
  if (!allowed || !buf || !len || seq >= __MAX_WRITES__) return;
  const n = Math.min(len >>> 0, 256);
  let bytes; try { bytes = buf.readByteArray(n); } catch (_) { return; }
  send({kind, seq:seq++, time:Date.now(), len:len >>> 0, self:self.toString(), vft:vf,
    returnAddress:ret.toString()}, bytes);
}
Interceptor.attach(wrapperWrite, { onEnter(args) {
  emit('wrapper', this.context.ecx, args[0], args[1].toUInt32(), this.returnAddress);
}});
Interceptor.attach(xorWrite, { onEnter(args) {
  emit('xor', this.context.ecx, args[0], args[1].toUInt32(), this.returnAddress);
}});
send({kind:'ready', wrapperWrite:wrapperWrite.toString(), xorWrite:xorWrite.toString()});
'''.replace('__MAX_WRITES__', str(MAX_WRITES))


def on_message(message, data):
    if message.get("type") != "send":
        print(message.get("description", message))
        return
    p = message["payload"]
    if p.get("kind") in ("wrapper", "xor"):
        name = f"{RUN:010d}_{p['seq']:06d}_{p['kind']}.bin"
        (OUT / name).write_bytes(data or b"")
        record = dict(p)
        record["data"] = name
        with (OUT / f"{RUN:010d}.jsonl").open("a", encoding="utf-8") as fh:
            fh.write(json.dumps(record) + "\n")
        if p["seq"] % 100 == 0:
            print(json.dumps(p))
    else:
        print(json.dumps(p))


device = frida.get_local_device()
session = device.attach(PID)
script = session.create_script(JS)
script.on("message", on_message)
script.load()
print(f"capturing delta layers PID {PID} for {DURATION}s; Ctrl+C to stop")
try:
    time.sleep(DURATION)
except KeyboardInterrupt:
    pass
finally:
    session.detach()
