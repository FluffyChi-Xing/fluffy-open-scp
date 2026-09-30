#!/usr/bin/env python3
"""hook_inner_device.py — 定位内层系统 d3d9 设备（静态 rdata vftable）并钩 shader 槽。

背景（2026-09-30 交通图实锤）：外层包装设备只有 Present/CreateTexture 流量，
城市渲染的 SetVertexShader/SetPixelShader 走内部路径——内层系统设备的 vftable
是静态 rdata 表 0x6c2b1490（§65 signature：17/78/79/89 等值≈帧率）。

流程：全堆扫 vftable==0x6c2b1490 的对象 → 密度校验 → 常驻计数器（槽 3..129）
→ 巡视器发现 blob/obj 流量即武装 hookCreate/hookBind（内容校验落盘）。
"""
import frida, pathlib, sys, time

OUT = pathlib.Path(r"D:\rust\packages\fluffy-open-scp\tmp\dynamic\shaders")
OUT.mkdir(parents=True, exist_ok=True)

JS = r"""

var devices = {};
var armed = {};
var sizeP = null;

function fnAt(vt, slot) { return vt.add(slot * 4).readPointer(); }
function inExec(p) {
  var m = Process.findModuleByName('d3d9.dll');
  if (m === null) return false;
  var lo = parseInt(m.base.toString(16), 16);
  var hi = lo + m.size;
  var a = parseInt(p.toString(16), 16);
  return a >= lo && a < hi;
}
function validateReport(vt) {
  var exec = 0, n = 0;
  try {
    for (var i = 0; i < 130; i++) {
      var fn = vt.add(i * 4).readPointer();
      n++;
      if (!fn.isNull() && inExec(fn)) exec++;
    }
  } catch (e) {}
  return { exec: exec, n: n, ok: n >= 110 && exec >= 100 };
}
function quickComObj(p) {
  if (p.isNull()) return false;
  try {
    var vt = p.readPointer();
    var exec = 0;
    for (var i = 0; i < 24; i++) {
      var fn = vt.add(i * 4).readPointer();
      if (!fn.isNull() && inExec(fn)) exec++;
    }
    return exec >= 19;
  } catch (e) { return false; }
}

// 第一步：堆扫内层设备
function findInnerDevices() {
  var ranges = Process.enumerateRanges('rw-');
  let pending = ranges.slice(), found = 0;
  function step() {
    if (!pending.length) {
      send({ kind: 'log', text: '内层设备扫描完成：' + found + ' 台' });
      return;
    }
    const r = pending.shift();
    let off = 0;
    const CHUNK = 8 * 1048576;
    function sub() {
      if (off >= r.size) { setTimeout(step, 0); return; }
      const len = Math.min(CHUNK, r.size - off);
      let buf = null;
      try { buf = r.base.add(off).readByteArray(len); } catch (e) { off = r.size; setTimeout(sub, 0); return; }
      if (buf) {
        const u8 = new Uint8Array(buf);
        for (let i = 0; i + 4 <= u8.length; i += 4) {
          // vftable 可能堆构（包装层）——不用模块范围预检；廉价预检：vtable
          // 可读且前 12 槽 ≥10 槽指向 exec（COM 级密度），再 validateReport 确认
          const v = u8[i] | (u8[i+1] << 8) | (u8[i+2] << 16) | (u8[i+3] << 24);
          if (v < 0x10000) continue;
          const obj = r.base.add(off + i);
          let vt = 0;
          try { vt = obj.readPointer(); } catch (e) { continue; }
          if (!vt) continue;
          let e12 = 0, ok12 = true;
          try {
            for (let k = 0; k < 12; k++) {
              const f = vt.add(k * 4).readPointer();
              if (!f.isNull() && inExec(f)) e12++;
            }
          } catch (e) { continue; }
          if (!ok12 || e12 < 10) continue;
          const rep = validateReport(vt);
          if (rep.ok) {
            found++;
            send({ kind: 'log', text: '密度设备 @ ' + obj + ' vftable ' + vt + '（exec ' + rep.exec + '/' + rep.n + '）' });
            watchDevice(obj, vt);
          }
        }
      }
      off += len;
      setTimeout(sub, 0);
    }
    sub();
  }
  step();
}

// 第二步：常驻计数器（全部槽位）
function watchDevice(obj, vt) {
  const key = vt.toString();
  if (devices[key]) return;
  const w = { counters: {}, armed: false, armedBind: [] };
  devices[key] = w;
  for (let s = 3; s <= 129; s++) {
    (function (slot) {
      const r = { blob: 0, obj: 0, n: 0 };
      w.counters[slot] = r;
      try {
        const l = Interceptor.attach(fnAt(vt, slot), {
          onEnter: function (args) {
            try {
              r.n++;
              if (isShaderBlob(args[1])) { r.blob++; return; }
              if (quickComObj(args[1])) r.obj++;
            } catch (e) {}
          }
        });
      } catch (e) { /* thunk 槽位跳过 */ }
    })(s);
  }
  send({ kind: 'log', text: '设备 ' + key + ' 计数器已挂（槽 3-129）' });
}

function isShaderBlob(p) {
  try {
    const t = p.readU32();
    const v = p.add(4).readU32();
    const hi = v >>> 16;
    return (t === 0xFFFE || t === 0xFFFF) && (hi === 0xFFFE || hi === 0xFFFF);
  } catch (e) { return false; }
}

// 第三步：巡视武装——blob→create hook；obj→bind hook（内容校验落盘）
setInterval(function () {
  for (const key in devices) {
    const w = devices[key];
    const vt = ptr(key);
    const createSlots = [], bindSlots = [], detail = [];
    for (const s in w.counters) {
      const r = w.counters[s];
      if (r.blob > 0) createSlots.push(+s);
      if (r.obj > 0) { bindSlots.push(+s); detail.push(s + ':' + r.obj); }
    }
    const hot = [];
    for (const s2 in w.counters) if (w.counters[s2].n > 0) hot.push(s2 + ':' + w.counters[s2].n);
    hot.sort(function (a, b) { return b.split(':')[1] - a.split(':')[1]; });
    send({ kind: 'log', text: '内层交通图[' + key + '] ' + hot.slice(0, 8).join(' ') });
    if (!w.armed && createSlots.length > 0) {
      w.armed = true;
      createSlots.forEach(function (s) { hookCreate(vt, s); });
      send({ kind: 'log', text: '武装 create@' + JSON.stringify(createSlots) + '（' + key + '）' });
    }
    const newBind = bindSlots.filter(function (s) { return w.armedBind.indexOf(+s) < 0; });
    if (newBind.length > 0) {
      newBind.forEach(function (s) { w.armedBind.push(+s); hookBind(vt, +s); });
      send({ kind: 'log', text: '武装 bind@' + JSON.stringify(newBind) + '（' + key + '，obj ' + detail.join(',') + '）' });
    }
  }
}, 15000);

function hookCreate(vt, slot) {
  const fn = fnAt(vt, slot);
  const kf = fn.toString();
  Interceptor.attach(fn, {
    onEnter: function (args) {
      this.hit = isShaderBlob(args[1]);
      if (this.hit) { this.blob = args[1]; this.pp = args[2]; }
    },
    onLeave: function () {
      if (!this.hit) return;
      try {
        const len = this.blob.readU32();
        const ver = this.blob.add(4).readU32();
        send({ kind: 'shader', type: (ver >>> 16) === 0xFFFF ? 'ps' : 'vs', source: 'create', slot: slot },
          this.blob.readByteArray(len));
      } catch (e) {}
    }
  });
}

function hookBind(vt, slot) {
  const fn = fnAt(vt, slot);
  const kf = fn.toString();
  const seen = {};
  Interceptor.attach(fn, {
    onEnter: function (args) {
      const obj = args[1];
      if (obj.isNull()) return;
      const k = obj.toString();
      if (seen[k]) return;
      if (!quickComObj(obj)) return;
      seen[k] = true;
      try {
        const getFn = new NativeFunction(obj.readPointer().add(16).readPointer(),
          'int32', ['pointer', 'pointer', 'pointer']);
        if (getFn(obj, ptr(0), sizeP) !== 0) return;
        const size = sizeP.readU32();
        if (size < 16 || size > (4 << 20) || (size & 3) !== 0) return;
        const bufp = Memory.alloc(size);
        if (getFn(obj, bufp, sizeP) !== 0) return;
        const ver = bufp.add(4).readU32();
        const hi = ver >>> 16;
        if (hi !== 0xFFFF && hi !== 0xFFFE) return;
        send({ kind: 'shader', type: hi === 0xFFFF ? 'ps' : 'vs', source: 'bind', slot: slot, obj: k },
          bufp.readByteArray(size));
      } catch (e) {}
    }
  });
}

setTimeout(function () {
  sizeP = Memory.alloc(4);
  findInnerDevices();
}, 300);
"""

count = {"files": 0, "by_type": {}}


def on_message(message, data):
    if message.get("type") == "send":
        p = message["payload"]
        kind = p.get("kind")
        if kind == "log":
            print("[agent]", p.get("text", ""))
        elif kind == "shader":
            t = p.get("type", "x")
            slot = p.get("slot")
            count["files"] += 1
            count["by_type"][t] = count["by_type"].get(t, 0) + 1
            fn = OUT / f"inner_{t}_slot{slot}_{count['files']:04d}.bin"
            fn.write_bytes(data or b"")
            if count["files"] % 10 == 0:
                print(f"[落盘] {count['files']} 份 {count['by_type']}")
    elif message.get("type") == "error":
        print("[err]", str(message.get("description", ""))[:160])


def main():
    dev = frida.get_local_device()
    target = None
    for proc in dev.enumerate_processes():
        if proc.name.lower() == "simcity.exe":
            target = proc.pid
            break
    if target is None:
        print("SimCity.exe 未找到")
        sys.exit(1)
    print(f"attach PID {target}（内层设备猎手）")
    session = dev.attach(target)
    script = session.create_script(JS)
    script.on("message", on_message)
    script.load()
    while True:
        time.sleep(2)


if __name__ == "__main__":
    main()
