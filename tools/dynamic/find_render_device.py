#!/usr/bin/env python3
"""find_render_device.py — 三步定位渲染设备并自动武装（2026-09-30 深夜版）。

1. 外部 ReadProcessMemory 扫描：找全部「vftable 在 d3d9 模块 + exec 密度达标」
   且有堆实例的接口（零注入）。
2. frida 对每台候选挂 slot17(Present) 计数器（每台 1 钩，共 ≤15 钩）——
   30s 后按帧率 signature（Present ≈ 帧率）锁定渲染设备。
3. 渲染设备 78..105 全槽武装：blob→create hook、obj→bind hook（内容校验落盘，
   错槽零产出）。此后进城/飞清单即持续落盘。
"""
import ctypes, frida, sys, time, glob, pathlib
from ctypes import wintypes

PID = int(sys.argv[1]) if len(sys.argv) > 1 else None
MOD_LO, MOD_HI = 0x6C2B0000, 0x6C42A000
EXEC_LO, EXEC_HI = 0x6C2B1000, 0x6C40D000
OUT = pathlib.Path(r"D:\rust\packages\fluffy-open-scp\tmp\dynamic\shaders")
OUT.mkdir(parents=True, exist_ok=True)

k32 = ctypes.WinDLL("kernel32", use_last_error=True)
PROCESS_VM_READ, PROCESS_QUERY_INFORMATION = 0x0010, 0x0400
MEM_COMMIT = 0x1000


class MBI(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_void_p), ("AllocationBase", ctypes.c_void_p),
                ("AllocationProtect", wintypes.DWORD), ("RegionSize", ctypes.c_size_t),
                ("State", wintypes.DWORD), ("Protect", wintypes.DWORD), ("Type", wintypes.DWORD)]


def external_scan(h):
    """返回 {vftable_va_hex: [堆实例地址, ...]}（密度达标且有堆实例者）。"""
    regs = []
    addr, mbi = 0, MBI()
    while addr < 0x7FFF0000:
        if not k32.VirtualQueryEx(h, ctypes.c_void_p(addr), ctypes.byref(mbi), ctypes.sizeof(mbi)):
            addr += 0x1000
            continue
        if mbi.State == MEM_COMMIT and mbi.Protect in (0x02, 0x04, 0x06, 0x20, 0x40, 0x80):
            regs.append((mbi.BaseAddress, mbi.RegionSize))
        addr = (mbi.BaseAddress or addr) + mbi.RegionSize
    read = k32.ReadProcessMemory
    read.argtypes = [wintypes.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
    from collections import defaultdict
    hits = defaultdict(list)
    vtbuf = (ctypes.c_char * (130 * 4))()
    got = ctypes.c_size_t(0)
    CHUNK = 1 << 20
    for base, size in regs:
        off = 0
        while off < size:
            n = min(CHUNK, size - off)
            buf = (ctypes.c_char * n)()
            if not read(h, ctypes.c_void_p(base + off), buf, n, ctypes.byref(got)) or got.value == 0:
                break
            n = got.value
            b = buf
            for i in range(0, n - 4, 4):
                # 只关心堆位置（跳过模块映像内部伪指针）
                pos = base + off + i
                if MOD_LO <= pos < MOD_HI:
                    continue
                v = int.from_bytes(b[i:i + 4], "little")
                if not (MOD_LO <= v < MOD_HI):
                    continue
                if not read(h, ctypes.c_void_p(v), vtbuf, 130 * 4, ctypes.byref(got)) or got.value != 130 * 4:
                    continue
                execn = sum(1 for s2 in range(130)
                            if (lambda fp: fp and EXEC_LO <= fp < EXEC_HI)(int.from_bytes(bytes(vtbuf[s2 * 4:s2 * 4 + 4]), "little")))
                if execn >= 100:
                    hits[hex(v)].append(hex(pos))
            off += n
    return hits


JS_TEMPLATE = r"""
var VTS = __VTS__;   // [[vftableHex, label], ...]
var counters = {};   // vtHex -> {n:0}
var armedVt = null;
var sizeP = Memory.alloc(4);

function fnAt(vt, slot) { return vt.add(slot * 4).readPointer(); }
function inExec(p) {
  var lo = 0x6C2B1000, hi = 0x6C40D000;
  var a = parseInt(p.toString(16), 16);
  return a >= lo && a < hi;
}
function isShaderBlob(p) {
  try {
    var t = p.readU32(), v = p.add(4).readU32(), hi = v >>> 16;
    return t === 0xFFFE && (hi === 0xFFFE || hi === 0xFFFF);
  } catch (e) { return false; }
}
function structComObj(p) {
  if (p.isNull()) return false;
  try {
    var vt = p.readPointer();
    if (vt.isNull()) return false;
    var n = 0;
    for (var i = 0; i < 8; i++) if (!vt.add(i * 4).readPointer().isNull()) n++;
    return n >= 4;
  } catch (e) { return false; }
}

function hookBind(vt, slot) {
  var fn = fnAt(vt, slot), kf = fn.toString();
  Interceptor.attach(fn, {
    onEnter: function (args) {
      var obj = args[1];
      if (obj.isNull()) return;
      var k = obj.toString();
      if (hookBind.seen[k]) return;
      if (!structComObj(obj)) return;
      hookBind.seen[k] = true;
      try {
        var getFn = new NativeFunction(obj.readPointer().add(16).readPointer(),
          'int32', ['pointer', 'pointer', 'pointer']);
        if (getFn(obj, ptr(0), sizeP) !== 0) return;
        var size = sizeP.readU32();
        if (size < 16 || size > (4 << 20) || (size & 3) !== 0) return;
        var bufp = Memory.alloc(size);
        if (getFn(obj, bufp, sizeP) !== 0) return;
        var hi = bufp.add(4).readU32() >>> 16;
        if (hi !== 0xFFFF && hi !== 0xFFFE) return;
        send({ kind: 'shader', type: hi === 0xFFFF ? 'ps' : 'vs', slot: slot, obj: k },
          bufp.readByteArray(size));
      } catch (e) {}
    }
  });
}
hookBind.seen = {};

function hookCreate(vt, slot) {
  var fn = fnAt(vt, slot), kf = fn.toString();
  Interceptor.attach(fn, {
    onEnter: function (args) {
      this.hit = isShaderBlob(args[1]);
      if (this.hit) { this.blob = args[1]; }
    },
    onLeave: function () {
      if (!this.hit) return;
      try {
        var len = this.blob.readU32();
        var hi = this.blob.add(4).readU32() >>> 16;
        send({ kind: 'shader', type: hi === 0xFFFF ? 'ps' : 'vs', source: 'create', slot: slot },
          this.blob.readByteArray(len));
      } catch (e) {}
    }
  });
}

// 阶段 1：候选 vftable 各挂 slot17(Present) 计数器
var listeners = [];
VTS.slice(0, 6).forEach(function (entry) {
  var vt = ptr(entry[0]);
  try {
    var l = Interceptor.attach(fnAt(vt, 17), {
      onEnter: function () {
        var c = counters[entry[0]] || (counters[entry[0]] = { n: 0 });
        c.n++;
      }
    });
    listeners.push(l);
  } catch (e) { send({ kind: 'log', text: 'slot17 hook 失败 ' + entry[0] + ': ' + e }); }
});

// 阶段 2：30s 后按帧率 signature 锁定渲染设备并武装
setTimeout(function () {
  // 计数窗到期：6 个计数器全部 detach（钩子预算铁律）
  listeners.forEach(function (l) { l.detach(); });
  var best = null, bestN = 0;
  var report = [];
  for (var k in counters) {
    var n = counters[k].n;
    report.push(k.slice(2) + ':' + n);
    if (n > bestN) { bestN = n; best = k; }
  }
  send({ kind: 'log', text: 'slot17(Present) 30s 计数: ' + report.join(' ') });
  if (best === null || bestN < 100) {
    send({ kind: 'log', text: '无帧率级 Present（最高 ' + bestN + '）——渲染设备可能不在这批候选里' });
    return;
  }
  armedVt = ptr('0x' + best);
  send({ kind: 'log', text: '渲染设备锁定: 0x' + best + '（Present ' + bestN + '/30s）——武装 91/92/100/101 四槽（总活钩≤4）' });
  [91, 92, 100, 101].forEach(function (slot) {
    var fn = fnAt(armedVt, slot);
    Interceptor.attach(fn, {
      onEnter: function (args) {
        try {
          if (isShaderBlob(args[1])) { hookCreate(armedVt, slot); return; }
          if (structComObj(args[1])) hookBind(armedVt, slot);
        } catch (e) {}
      }
    });
  });
  send({ kind: 'log', text: '武装完成（91/92/100/101 四槽）——请按 A-G 清单飞行，绑定/创建即落盘' });
}, 30000);
"""


def on_message(m, data):
    if m.get("type") == "send":
        p = m["payload"]
        if p.get("kind") == "log":
            print("[agent]", p.get("text", ""))
        elif p.get("kind") == "shader":
            count["n"] += 1
            t = p.get("type", "x")
            src = p.get("source", "bind")
            fn = OUT / f"cap_{t}_{src}_{count['n']:04d}.bin"
            fn.write_bytes(data or b"")
            if count["n"] % 5 == 0:
                print(f"[落盘] {count['n']} 份（最新 {fn.name}）")
    elif m.get("type") == "error":
        print("[err]", str(m.get("description", ""))[:140])


count = {"n": 0}


def main():
    if PID is None:
        print("用法: find_render_device.py <pid>")
        sys.exit(1)
    # 1) 外部扫描
    h = k32.OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, False, PID)
    if not h:
        print(f"OpenProcess({PID}) 失败")
        sys.exit(1)
    hits = external_scan(h)
    k32.CloseHandle(h)
    heap_vts = sorted(hits.items(), key=lambda kv: -len(kv[1]))
    print(f"外部扫描：{len(heap_vts)} 个 vftable 有堆实例")
    for v, objs in heap_vts[:12]:
        print(f"  {v}: {len(objs)} 实例")
    if not heap_vts:
        print("无候选——退出")
        sys.exit(1)
    # 2) frida 挂 slot17 计数器（只取堆实例最多的前 12 台）
    vts = [[v, ""] for v, _ in heap_vts[:12]]
    dev = frida.get_local_device()
    session = dev.attach(PID)
    script = session.create_script(JS_TEMPLATE.replace("__VTS__", repr(vts)))
    script.on("message", on_message)
    script.load()
    print("[阶段1] slot17 计数器已挂（30s 后自动锁定渲染设备并武装）——请在城里正常移动")
    deadline = time.time() + 45 * 60
    while time.time() < deadline:
        time.sleep(2)


if __name__ == "__main__":
    main()
