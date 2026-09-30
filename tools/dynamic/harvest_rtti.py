#!/usr/bin/env python3
"""harvest_rtti.py — 一次性被动收割（无钩子，只读，随游戏存活）。

1. RTTI 收割：SimCity.exe 模块内扫 .?AV 类型描述符 → SC:: 类名 → COL → vftable VA
2. 材质哈希上下文扫描：73684EFC(sign)/E5390A98(graf)/4491DE3A(hole) 字节模式 →
   命中处 ±0x800B 落盘（shader-def 上下文 + 变体对象线索，R2）
3. 已知 vftable 对象：cTessendorfWater 0xd9f7bc → 全实例 dump（水面研究）

产物：tmp/dynamic/live/rtti/、variant_objs/、water/
"""
import frida, json, pathlib, sys, time

LIVE = pathlib.Path(r"D:\rust\packages\fluffy-open-scp\tmp\dynamic\live")
OUT = LIVE / "rtti"
OUT.mkdir(parents=True, exist_ok=True)
(LIVE / "variant_objs").mkdir(exist_ok=True)
(LIVE / "water").mkdir(exist_ok=True)

JS = r"""
const MODULE = 'SimCity.exe';
const HASHES = [
  { name: 'sign_73684EFC', bytes: 'FC 4E 68 73' },
  { name: 'graf_E5390A98', bytes: '98 0A 39 E5' },
  { name: 'hole_4491DE3A', bytes: '3A DE 91 44' },
];
const KNOWN_VFT = { '0xd9f7bc': 'cTessendorfWater', '0xd9f7c8': 'cTessendorfWaterB' };

function sendChunk(kind, payload, data) { send({ kind: kind, payload: payload }, data); }

// ---------- 1) RTTI：模块内 .?AV 类型描述符 ----------
function harvestRTTI() {
  const mod = Process.findModuleByName(MODULE);
  const base = mod.base, size = mod.size;
  // 类型描述符名字段模式 ".?AV"（SC 命名空间优先，但先全收）
  const pattern = '2E 3F 41 56'; // ".?AV"
  const tds = [];
  Memory.scanSync(base, size, pattern).forEach(function (hit) {
    const td = hit.address.sub(8); // name 在 TD+8
    // 读类名（可打印，至多 128B）
    let name = '';
    try { name = hit.address.readUtf8String(128); } catch (e) { return; }
    if (!name || name.length < 4) return;
    tds.push({ td: td.toString(16), name: name });
  });
  sendChunk('rtti_names', tds);

  // TD → COL → vftable（限模块内，避免全堆扫）
  // COL+12 == TD 地址；vftable[-4] == COL
  const results = [];
  const tdByAddr = {};
  tds.forEach(function (t) { tdByAddr[t.td] = t.name; });
  const tdAddrs = Object.keys(tdByAddr);
  const modStart = parseInt(base.toString(16), 16);
  const modEnd = modStart + size;
  tdAddrs.forEach(function (tdHex) {
    const tdAddr = ptr('0x' + tdHex);
    // 在模块内找 u32 == tdAddr（即 COL+12 的位置）
    const pat = tdHex.replace(/(..)/g, '$1 ').trim().split(' ').reverse().join(' ');
    // little-endian 字节序
    const b = tdAddr.toString(16).padStart(8, '0').match(/../g).reverse().join(' ');
    Memory.scanSync(base, size, b).forEach(function (hit) {
      const col = hit.address.sub(12);
      try {
        const sig = col.readU32();
        if (sig !== 0) return; // x86 COL signature=0
        const offset = col.add(4).readU32();
        if (offset > 0x1000) return;
      } catch (e) { return; }
      // 找 vftable：模块内 u32 == col
      const colHex = col.toString(16).padStart(8, '0').match(/../g).reverse().join(' ');
      try {
        Memory.scanSync(base, size, colHex).forEach(function (h2) {
          const vft = h2.address.add(4);
          const vftInt = parseInt(vft.toString(16), 16);
          if (vftInt <= modStart || vftInt >= modEnd) return;
          results.push({ name: tdByAddr[tdHex], vft: '0x' + vft.toString(16) });
        });
      } catch (e) {}
    });
  });
  sendChunk('rtti_vfts', results);
}

// ---------- 2) 材质哈希上下文（rw- 堆，分块只读） ----------
function scanHashes() {
  const ranges = Process.enumerateRanges('rw-').concat(Process.enumerateRanges('r--'));
  let pending = ranges.slice(), totalMB = 0, failMB = 0;
  ranges.forEach(function (r) { totalMB += r.size / 1048576; });
  sendChunk('log', { text: '堆扫描 ' + ranges.length + ' 段 / ' + Math.round(totalMB) + 'MB' });
  const CHUNK = 16 * 1048576;
  function step() {
    if (!pending.length) { sendChunk('log', { text: '堆扫描完成: 实读 ' + Math.round(scannedMB) + 'MB / 不可读 ' + Math.round(failMB) + 'MB' }); scanVftObjects(); return; }
    const r = pending.shift();
    let off = 0;
    function sub() {
      if (off >= r.size) { setTimeout(step, 0); return; }
      const len = Math.min(CHUNK, r.size - off);
      let buf = null;
      try { buf = Memory.readByteArray(r.base.add(off), len); } catch (e) { failMB += len / 1048576; off = r.size; setTimeout(sub, 0); return; }
      if (buf) {
        scannedMB += len / 1048576;
        const u8 = new Uint8Array(buf);
        HASHES.forEach(function (h) {
          const needle = h.bytes.split(' ').map(function (x) { return parseInt(x, 16); });
          for (let i = 0; i + 4 <= u8.length; i++) {
            if (u8[i] === needle[0] && u8[i+1] === needle[1] && u8[i+2] === needle[2] && u8[i+3] === needle[3]) {
              const addr = r.base.add(off + i);
              const from = addr.sub(0x800);
              try {
                sendChunk('hashdump', { name: h.name, addr: addr.toString(16) },
                  from.readByteArray(0x1000));
              } catch (e) {}
            }
          }
        });
      }
      off += len;
      setTimeout(sub, 0);
    }
    sub();
  }
  step();
}

// ---------- 3) 已知 vftable 对象实例 ----------
function scanVftObjects() {
  const ranges = Process.enumerateRanges('rw-');
  sendChunk('log', { text: 'vft 对象扫描 ' + ranges.length + ' 段' });
  let pending = ranges.slice();
  const CHUNK = 16 * 1048576;
  function step() {
    if (!pending.length) { send({ kind: 'alldone' }); return; }
    const r = pending.shift();
    let off = 0;
    function sub() {
      if (off >= r.size) { setTimeout(step, 0); return; }
      const len = Math.min(CHUNK, r.size - off);
      let buf = null;
      try { buf = Memory.readByteArray(r.base.add(off), len); } catch (e) { off = r.size; setTimeout(sub, 0); return; }
      if (buf) {
        const u8 = new Uint8Array(buf);
        Object.keys(KNOWN_VFT).forEach(function (vftHex) {
          const vftInt = parseInt(vftHex, 16);
          const nb = [vftInt & 0xff, (vftInt >> 8) & 0xff, (vftInt >> 16) & 0xff, (vftInt >>> 24) & 0xff];
          for (let i = 0; i + 4 <= u8.length; i += 4) {
            if (u8[i] === nb[0] && u8[i+1] === nb[1] && u8[i+2] === nb[2] && u8[i+3] === nb[3]) {
              const addr = r.base.add(off + i);
              try {
                sendChunk('vftdump', { name: KNOWN_VFT[vftHex], addr: addr.toString(16) },
                  addr.readByteArray(0x800));
              } catch (e) {}
            }
          }
        });
      }
      off += len;
      setTimeout(sub, 0);
    }
    sub();
  }
  step();
}

setTimeout(function () {
  try { harvestRTTI(); } catch (e) { sendChunk('log', { text: 'RTTI err: ' + e }); }
  scanHashes();
}, 200);
"""

dump_counts = {}
stop = False


def on_message(message, data):
    global stop
    if message.get("type") == "send":
        payload = message["payload"]
        kind = payload.get("kind")
        if kind == "rtti_names":
            names = payload["payload"] if isinstance(payload.get("payload"), list) else payload.get("payload", [])
            lines = [f"{t['td']}  {t['name']}" for t in names]
            (OUT / "class_names.txt").write_text("\n".join(lines), encoding="utf-8")
            print(f"[rtti] 类型描述符 {len(names)} 个 → class_names.txt")
        elif kind == "rtti_vfts":
            vfts = payload["payload"] if isinstance(payload.get("payload"), list) else []
            lines = [f"{v['vft']}  {v['name']}" for v in vfts]
            (OUT / "vftables.txt").write_text("\n".join(lines), encoding="utf-8")
            print(f"[rtti] vftable 关联 {len(vfts)} 条 → vftables.txt")
        elif kind == "hashdump":
            p = payload["payload"]
            fn = LIVE / "variant_objs" / f"{p['name']}_{p['addr']}.bin"
            fn.write_bytes(data or b"")
            dump_counts["hash"] = dump_counts.get("hash", 0) + 1
            if dump_counts["hash"] % 10 == 0:
                print(f"[hash] 已落盘 {dump_counts['hash']} 份")
        elif kind == "vftdump":
            p = payload["payload"]
            fn = LIVE / "water" / f"{p['name']}_{p['addr']}.bin"
            fn.write_bytes(data or b"")
            dump_counts["vft"] = dump_counts.get("vft", 0) + 1
        elif kind == "log":
            print(f"[agent] {payload.get('text','')}")
        elif kind == "alldone":
            print(f"[完成] hash 上下文 {dump_counts.get('hash',0)} 份 / vft 对象 {dump_counts.get('vft',0)} 份")
            stop = True
    elif message.get("type") == "error":
        print(f"[err] {message.get('description','')[:200]}")


def main():
    dev = frida.get_local_device()
    target = None
    for p in dev.enumerate_processes():
        if p.name.lower() == "simcity.exe":
            target = p.pid
            break
    if target is None:
        print("SimCity.exe 未找到")
        sys.exit(1)
    print(f"attach PID {target}（只读扫描，无钩子）")
    session = dev.attach(target)
    script = session.create_script(JS)
    script.on("message", on_message)
    script.load()
    deadline = time.time() + 1800
    while not stop and time.time() < deadline:
        time.sleep(1)
    print("扫描结束（会话保持挂载，直接退出即可）")


if __name__ == "__main__":
    main()
