#!/usr/bin/env python3
"""Capture bounded cStreamXOR buffers from the verified retail build."""
import frida, json, pathlib, sys, time

PID = int(sys.argv[1]) if len(sys.argv) > 1 else 46580
OUT = pathlib.Path("tmp/dynamic/live/delta-xor/capture")
OUT.mkdir(parents=True, exist_ok=True)

JS = r'''
const base = Process.findModuleByName('SimCity.exe').base;
const readFn = base.add(0x1c6010);
const writeFn = base.add(0x1c6060);
let seq = 0;
let captured = 0;
let readCaptured = 0;
let writeCaptured = 0;
let keySent = false;
function safeU32(p) { try { return p.readU32(); } catch (_) { return null; } }
function safeBytes(p, n) { try { return p.readByteArray(n); } catch (_) { return null; } }
function meta(self) {
  return { self:self.toString(), vft:safeBytes(self,4) ? self.readPointer().toString() : null,
    keyPtr:safeU32(self.add(0xc)), keyLen:safeU32(self.add(0x10)), phase:safeU32(self.add(0x14)) };
}
function emit(kind, ctx, buf, len) {
  if (kind === 'read' && readCaptured >= 500) return;
  if (kind === 'write' && writeCaptured >= 5000) return;
  if (!ctx || (ctx.vft !== '0xd1ae54' && ctx.vft !== '0xd1ae5c')) return;
  const n = Math.min(Math.max(len >>> 0, 0), 4096);
  if (!buf || n === 0) return;
  captured++;
  if (kind === 'read') readCaptured++; else writeCaptured++;
  if (!keySent && ctx && ctx.keyPtr && ctx.keyLen === 128) {
    try { send({kind:'key', address:ptr(ctx.keyPtr).toString(), length:ctx.keyLen}, ptr(ctx.keyPtr).readByteArray(ctx.keyLen)); keySent=true; } catch (_) {}
  }
  send({kind:kind, seq:seq++, len:len, meta:ctx}, safeBytes(buf,n));
}
Interceptor.attach(readFn, {
  onEnter(args) { this.self=this.context.ecx; this.buf=args[0]; this.len=args[1].toUInt32(); this.ctx=meta(this.self); },
  onLeave(retval) { const n=retval.toUInt32(); if (n) emit('read', this.ctx, this.buf, n); }
});
Interceptor.attach(writeFn, {
  onEnter(args) { const self=this.context.ecx, buf=args[0], len=args[1].toUInt32(); emit('write', meta(self), buf, len); }
});
send({kind:'ready', read:readFn.toString(), write:writeFn.toString()});
'''

def on_message(message, data):
    if message.get("type") != "send":
        print(message.get("description", message)); return
    p=message["payload"]; kind=p.get("kind")
    if kind in ("read","write"):
        name=f"{p['seq']:08d}_{kind}.bin"; (OUT/name).write_bytes(data or b"")
        info={k:v for k,v in p.items() if k != "kind"}; info["data"]=name
        (OUT/(name+".json")).write_text(json.dumps(info, indent=2), encoding="utf-8")
        if p["seq"] % 100 == 0:
            print(json.dumps({"kind":kind,"seq":p["seq"],"len":p["len"],"meta":p.get("meta",{})}))
    elif kind == "key":
        (OUT/"stream-key-128.bin").write_bytes(data or b'')
        print(json.dumps({"kind":"key","address":p.get("address"),"length":p.get("length")}))
    else: print(json.dumps(p))

dev=frida.get_local_device(); session=dev.attach(PID); script=session.create_script(JS); script.on("message", on_message); script.load()
print(f"capturing PID {PID}; Ctrl+C to stop")
try:
    while True: time.sleep(1)
except KeyboardInterrupt: pass
finally: session.detach()
