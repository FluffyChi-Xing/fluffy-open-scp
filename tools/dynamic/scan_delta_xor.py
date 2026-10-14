#!/usr/bin/env python3
"""Read-only runtime scan for retail cStreamXOR objects and key material."""
import frida, json, pathlib, time, sys

PID = 46580
OUT = pathlib.Path("tmp/dynamic/live/delta-xor")
OUT.mkdir(parents=True, exist_ok=True)

JS = r'''
const mod = Process.findModuleByName('SimCity.exe');
const base = mod.base, size = mod.size;
function u32(addr) { try { return addr.readU32(); } catch (_) { return 0; } }
function bytesFor(v) { return [v & 255, (v >>> 8) & 255, (v >>> 16) & 255, (v >>> 24) & 255]; }
const tdNeedles = [
  '.?AVcStreamXOR@GB@@', '.?AVcDeltaStream@GB@@', '.?AVcMD5Stream@GB@@',
  '.?AVcMultiFileStreamWrapper@GB@@', '.?AVcMultiFileStream@GB@@',
  '.?AVcStreamChildWrapper@GB@@'
];
const names = [];
for (const n of tdNeedles) {
  const hits = Memory.scanSync(base, size, n.split('').map(c=>c.charCodeAt(0).toString(16).padStart(2,'0')).join(' '));
  names.push({needle:n, hits:hits.map(h=>h.address.toString())});
}
// Locate COLs whose type descriptor points at each name, then the associated vftable.
const vfts = [];
for (const item of names) for (const hitHex of item.hits) {
  const td = ptr(hitHex).sub(8);
  const tdBytes = td.toString().slice(2).padStart(8,'0').match(/../g).reverse().join(' ');
  for (const h of Memory.scanSync(base, size, tdBytes)) {
    const col = h.address.sub(12);
    if (u32(col) !== 0) continue;
    const colBytes = col.toString().slice(2).padStart(8,'0').match(/../g).reverse().join(' ');
    for (const h2 of Memory.scanSync(base, size, colBytes)) {
      const vf = h2.address.add(4);
      if (vf.compare(base) >= 0 && vf.compare(base.add(size)) < 0)
        vfts.push({name:item.needle, vft:vf.toString()});
    }
  }
}
send({kind:'meta', module:{base:base.toString(),size:size}, names:names, vfts:vfts});
for (const vf of vfts) {
  const entries=[]; const a=ptr(vf.vft);
  for (let i=0;i<24;i++) { try { const f=a.add(i*4).readPointer(); entries.push({slot:i,address:f.toString(),offset:f.sub(base).toString()}); } catch (_) { break; } }
  send({kind:'vtable', name:vf.name, vft:vf.vft, entries:entries});
}
// Scan writable ranges for vft pointers; report object bytes and likely key fields at +0xc/+0x10/+0x14.
const wanted = new Set(vfts.map(x=>x.vft));
let count=0;
for (const r of Process.enumerateRanges('rw-')) {
  let buf; try { buf = new Uint8Array(Memory.readByteArray(r.base, r.size)); } catch (_) { continue; }
  for (let i=0;i+4<=buf.length;i+=4) {
    const p = r.base.add(i); const x = p.readPointer().toString();
    if (!wanted.has(x)) continue;
    let dump; try { dump=p.readByteArray(0x80); } catch (_) { continue; }
    send({kind:'obj', addr:p.toString(), type:vfts.find(v=>v.vft===x)?.name || 'unknown', vft:x, keyPtr:u32(p.add(0xc)), keyLen:u32(p.add(0x10)), phase:u32(p.add(0x14))}, dump);
    if (++count >= 200) break;
  }
  if (count>=200) break;
}
send({kind:'done',count:count});
'''

def on_message(message, data):
    if message.get('type') != 'send':
        print(message.get('description', message)); return
    p = message['payload']; kind = p.get('kind')
    if kind == 'meta': (OUT/'meta.json').write_text(json.dumps(p, indent=2), encoding='utf-8'); print(json.dumps(p, indent=2))
    elif kind == 'obj':
        addr = p['addr'].replace('0x','')
        (OUT/f'obj_{addr}.bin').write_bytes(data or b'')
        print(json.dumps(p))
    else: print(json.dumps(p))

dev=frida.get_local_device(); session=dev.attach(PID); script=session.create_script(JS); script.on('message', on_message); script.load(); time.sleep(20)
