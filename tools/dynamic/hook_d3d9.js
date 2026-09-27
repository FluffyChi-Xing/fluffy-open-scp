'use strict';
// =====================================================================
// hook_d3d9.js — SimCity (2013) D3D9 shader 字节码抓取探针（frida 17.x）
// 配套主机端：tmp/dynamic/dump_shaders.py
//
// 设备 vftable 定位（v7，两级策略，全部实测不猜槽位）：
//   A. 随行观察：挂 Direct3DCreate9/9Ex 导出，捕获游戏自己的 IDirect3D9*，
//      对其 vtable 槽 3..16 装被动观察：某槽 onLeave 后 args[6]/args[7]
//      出现"130 槽中 ≥100 个 exec 指针的对象"→ 该槽即 CreateDevice。
//      （须在游戏创建设备之前挂上：片头阶段 attach 最稳）
//   D. 自建哑设备 + 槽位探测：CreateWindowExW(STATIC) → Direct3DCreate9 →
//      对 IDirect3D9 候选槽 [16,14,15,13,12] 逐一用完整 CreateDevice 参数
//      试调（错误槽位会抛访问违例异常，被 Frida 捕获，游戏无伤），成功返回
//      设备对象即得 vftable，随后立即释放哑设备。与游戏设备同一份 vftable。
//
// 槽位校准：对设备 vftable 槽 83..105 装被动计数（arg1 是 shader blob →
// create 槽；arg1 形似 COM 对象 → bind 候选），20s 后按实测装真 hook。
// bind hook 落盘前再验版本 token，滤掉顶点声明等同形对象。
// =====================================================================

var SMALL_MAX = ptr('0x10000'); // 小整数不是指针，先挡掉避免异常开销

var d3d9 = null;
var execLo = null, execHi = null;
var modLoN = 0, modHiN = 0; // d3d9 模块整体范围（数值比较；vftable 地址在模块数据段）
var armed = false;
var deviceFound = false;
var seenBind = new Set();
var hookedFns = new Set();
var getFnFailLog = 0;
var sizeP = Memory.alloc(4);
var stats = { create: 0, bind: 0 };
var cfg = { spawn: false };

function log(text) { send({ kind: 'log', text: text }); }

function inMod(p) {
  return p.compare(d3d9.base) >= 0 && p.compare(d3d9.base.add(d3d9.size)) < 0;
}

function inExec(p) {
  return execLo !== null && p.compare(execLo) >= 0 && p.compare(execHi) < 0;
}

function computeExecRanges() {
  Process.enumerateRanges('r-x').forEach(function (r) {
    if (inMod(r.base) && inMod(r.base.add(r.size))) {
      if (execLo === null || r.base.compare(execLo) < 0) execLo = r.base;
      var e = r.base.add(r.size);
      if (execHi === null || e.compare(execHi) > 0) execHi = e;
    }
  });
  modLoN = parseInt(d3d9.base.toString(16), 16);
  modHiN = parseInt(d3d9.base.add(d3d9.size).toString(16), 16);
}

function getExport(mod, name) {
  if (typeof mod.getExportByName === 'function') return mod.getExportByName(name);
  return Module.getGlobalExportByName(name);
}

// D3D9 字节码 blob：DWORD0=总长（含自身），DWORD1=版本 token（高半 0xFFFF=ps/0xFFFE=vs）
// token 流按 DWORD 对齐，长度必为 4 的倍数且至少 16 字节
function isShaderBlob(p) {
  if (p.isNull() || p.compare(SMALL_MAX) < 0) return false;
  try {
    var len = p.readU32();
    if (len < 16 || len > (4 << 20) || (len & 3) !== 0) return false;
    var hi = p.add(4).readU32() >>> 16;
    return hi === 0xFFFF || hi === 0xFFFE;
  } catch (e) { return false; }
}

// shader/声明类 COM 对象的粗筛：vftable 前几槽全指向 d3d9 代码段
function looksLikeComObj(p) {
  if (p.isNull() || p.compare(SMALL_MAX) < 0) return false;
  try {
    var vt = p.readPointer();
    if (!inExec(vt)) return false;
    for (var i = 0; i <= 4; i++) {
      if (!inExec(vt.add(i * 4).readPointer())) return false;
    }
    return true;
  } catch (e) { return false; }
}

// 130 槽中指向 exec 的数量（容忍接口尾部稀疏/空槽）
function validateReport(vt) {
  var exec = 0, n = 0;
  try {
    for (var i = 0; i < 130; i++) {
      var fn = vt.add(i * 4).readPointer();
      n++;
      if (!fn.isNull() && inExec(fn)) exec++;
    }
  } catch (e) { /* 读过界即止 */ }
  return { exec: exec, n: n, ok: n >= 110 && exec >= 100 };
}

function fnAt(vt, slot) { return vt.add(slot * 4).readPointer(); }

function safeAttach(fn, cb, tag) {
  try {
    return Interceptor.attach(fn, cb);
  } catch (e) {
    log('无法 hook ' + tag + ' @ ' + fn + ': ' + e);
    return null;
  }
}

// ---------- A. 随行观察：从游戏自己的 IDirect3D9 上捕获设备创建 ----------
function lurkDirect3DCreate() {
  ['Direct3DCreate9', 'Direct3DCreate9Ex'].forEach(function (name) {
    var exp = null;
    try { exp = getExport(d3d9, name); } catch (e) { return; }
    safeAttach(exp, {
      onLeave: function (ret) {
        if (!ret.isNull() && !deviceFound) {
          log(name + ' -> IDirect3D9* = ' + ret + '，开始随行观察 CreateDevice…');
          lurkCreateDevice(ret);
        }
      }
    }, name);
  });
}

function lurkCreateDevice(pD3D) {
  var vt = pD3D.readPointer();
  for (var s = 3; s <= 16; s++) {
    (function (slot) {
      safeAttach(fnAt(vt, slot), {
        onEnter: function (args) {
          // CreateDevice 含 7 参（this 下标 0..6），CreateDeviceEx 含 8 参
          this.p6 = args[6];
          this.p7 = args[7];
        },
        onLeave: function () {
          if (deviceFound) return;
          var self = this;
          [self.p6, self.p7].forEach(function (pp) {
            if (deviceFound || pp === undefined || pp.isNull()) return;
            try {
              var obj = pp.readPointer();
              if (obj.isNull() || !looksLikeComObj(obj)) return;
              var ovt = obj.readPointer();
              var rep = validateReport(ovt);
              if (!rep.ok) return;
              deviceFound = true;
              log('实测 CreateDevice = IDirect3D9 槽 ' + slot + '，设备 vftable @ ' + ovt);
              installSlotCounters(ovt, 20);
            } catch (e) { /* 非 CreateDevice 槽位，忽略 */ }
          });
        }
      }, 'lurk@' + slot);
    })(s);
  }
}

// ---------- D. 自建哑设备 + 槽位探测 ----------
function selfDeviceVftable() {
  var hwnd = ptr(0), pD3D = ptr(0);
  var madeDevice = ptr(0);
  try {
    var user32 = Process.findModuleByName('user32.dll');
    var k32 = Process.findModuleByName('kernel32.dll');
    if (user32 === null || k32 === null) { log('D: user32/kernel32 未加载'); return null; }
    var CreateWindowExW = new NativeFunction(getExport(user32, 'CreateWindowExW'),
      'pointer', ['uint32', 'pointer', 'pointer', 'uint32',
        'int32', 'int32', 'int32', 'int32', 'pointer', 'pointer', 'pointer', 'pointer']);
    var GetModuleHandleW = new NativeFunction(getExport(k32, 'GetModuleHandleW'),
      'pointer', ['pointer']);
    var DestroyWindow = new NativeFunction(getExport(user32, 'DestroyWindow'),
      'int32', ['pointer']);

    hwnd = CreateWindowExW(0, Memory.allocUtf16String('STATIC'),
      Memory.allocUtf16String('openscp_probe'), 0, 0, 0, 8, 8,
      ptr(0), ptr(0), GetModuleHandleW(ptr(0)), ptr(0));
    log('D: hwnd=' + hwnd);
    if (hwnd.isNull()) return null;

    var Direct3DCreate9 = getExport(d3d9, 'Direct3DCreate9');
    // 主线程渲染持锁时，Direct3DCreate9 可能死锁——放到独立原生线程跑，
    // 线程起点=Direct3DCreate9、参数=32，返回值（IDirect3D9*）即线程退出码
    var CreateThread = new NativeFunction(getExport(k32, 'CreateThread'),
      'pointer', ['pointer', 'uint', 'pointer', 'pointer', 'uint32', 'pointer']);
    var WaitForSingleObject = new NativeFunction(getExport(k32, 'WaitForSingleObject'),
      'uint32', ['pointer', 'uint32']);
    var GetExitCodeThread = new NativeFunction(getExport(k32, 'GetExitCodeThread'),
      'uint32', ['pointer', 'pointer']);
    var CloseHandle = new NativeFunction(getExport(k32, 'CloseHandle'),
      'int32', ['pointer']);
    var tid = Memory.alloc(4);
    var hThread = CreateThread(ptr(0), 0, Direct3DCreate9, ptr(32), 0, tid);
    if (hThread.isNull()) { log('D: CreateThread 失败'); return null; }
    var waitR = WaitForSingleObject(hThread, 15000);
    var exitCode = Memory.alloc(4);
    GetExitCodeThread(hThread, exitCode);
    CloseHandle(hThread);
    if (waitR !== 0) { log('D: Direct3DCreate9 独立线程 15s 超时'); return null; }
    pD3D = ptr(exitCode.readU32());
    log('D: IDirect3D9=' + pD3D);
    if (pD3D.isNull()) return null;

    // D3DPRESENT_PARAMETERS (x86, 48B): 8x8, UNKNOWN 格式, windowed, DISCARD
    var pp = Memory.alloc(48);
    for (var i = 0; i < 12; i++) pp.add(i * 4).writeU32(0);
    pp.add(0).writeU32(8);
    pp.add(4).writeU32(8);
    pp.add(12).writeU32(1);
    pp.add(24).writeU32(1);
    pp.add(28).writePointer(hwnd);
    pp.add(32).writeU32(1);
    var ppDev = Memory.alloc(4);

    // 槽位探测：错误槽位抛访问违例异常被捕获，游戏无伤
    var candidates = [16, 14, 15, 13, 12];
    for (var c = 0; c < candidates.length; c++) {
      if (madeDevice.isNull()) {
        (function (slot) {
          try {
            var createDevice = new NativeFunction(fnAt(pD3D.readPointer(), slot),
              'int32', ['pointer', 'uint32', 'uint32', 'pointer', 'uint32',
                'pointer', 'pointer']);
            ppDev.writePointer(ptr(0));
            var hr = createDevice(pD3D, 0, 1, hwnd, 0x20 /*SW VP*/, pp, ppDev);
            log('D: 探测槽 ' + slot + ' → hr=0x' + (hr >>> 0).toString(16));
            if (hr !== 0) return;
            var pDev = ppDev.readPointer();
            if (pDev.isNull()) return;
            var vt = pDev.readPointer();
            var rep = validateReport(vt);
            log('D: 槽 ' + slot + ' 设备=' + pDev + ' vftable=' + vt +
                '（exec ' + rep.exec + '/' + rep.n + '）');
            if (rep.ok) {
              madeDevice = pDev;
              log('D: 自建设备成功，vftable 定谳 @ ' + vt);
            } else {
              // 不合规对象也释放掉
              try { new NativeFunction(fnAt(vt, 2), 'uint32', ['pointer'])(pDev); } catch (e) {}
            }
          } catch (e) {
            log('D: 探测槽 ' + slot + ' 异常（已捕获，继续）: ' +
                (e + '').slice(0, 80));
          }
        })(candidates[c]);
      }
    }
    if (madeDevice.isNull()) {
      log('D: 所有候选槽位均未产出设备');
      return null;
    }
    var result = madeDevice.readPointer();
    // 立即释放哑设备（vftable 是类静态的，释放对象不影响 hook）
    try { new NativeFunction(fnAt(result, 2), 'uint32', ['pointer'])(madeDevice); } catch (e) {}
    try { new NativeFunction(fnAt(pD3D.readPointer(), 2), 'uint32', ['pointer'])(pD3D); } catch (e) {}
    try { DestroyWindow(hwnd); } catch (e) {}
    return result;
  } catch (e) {
    log('D: 异常 ' + e);
    return null;
  }
}

// ---------- C. 全堆扫描：堆里找设备对象本体（首字段即 vftable） ----------
// 抽查预过滤：10 个代表性槽位先过，全过才做完整 validateReport
function spotCheck(vt) {
  var probe = [0, 1, 2, 17, 18, 88, 89, 97, 98, 109];
  try {
    for (var i = 0; i < probe.length; i++) {
      var fn = vt.add(probe[i] * 4).readPointer();
      if (fn.isNull() || !inExec(fn)) return false;
    }
    return true;
  } catch (e) { return false; }
}

function scanHeapV2() {
  var candidates = [];  // 唯一 vftable 候选（不再 3 个就停，扫全堆）
  var seen = {};
  var ranges = Process.enumerateRanges('rw-'); // 不设大小上限，大段分块读
  var totalMB = 0;
  ranges.forEach(function (r) { totalMB += r.size / (1024 * 1024); });
  log('C: 全堆分块扫描 ' + ranges.length + ' 段 / ' + Math.round(totalMB) + 'MB…');
  var doneMB = 0;
  var CHUNK = 32 << 20;
  for (var ri = 0; ri < ranges.length && candidates.length < 40; ri++) {
    var r = ranges[ri];
    for (var off = 0; off < r.size && candidates.length < 40; off += CHUNK) {
      var len = Math.min(CHUNK, r.size - off);
      var base = r.base.add(off);
      var buf;
      try { buf = base.readByteArray(len); } catch (e) { break; }
      var u32 = new Uint32Array(buf);
    for (var i = 0; i < u32.length; i++) {
      var v = u32[i];
      if (v < 0x10000) continue;
      // vftable 地址必落在 d3d9 模块内（含头部 rdata 混合区——用流量模式区分真假）
      if (v < modLoN || v >= modHiN) continue;
      var vt = ptr(v);
        var key = vt.toString();
        if (seen[key]) continue;
        if (!spotCheck(vt)) continue;
        seen[key] = true;
        candidates.push(vt);
        log('候选 ' + candidates.length + ': vftable ' + vt + ' @ ' + base.add(i * 4));
      }
      doneMB += len / (1024 * 1024);
      log('C: 已扫 ' + Math.round(doneMB) + '/' + Math.round(totalMB) +
          'MB，候选 ' + candidates.length);
    }
  }
  return candidates;
}

// 多候选同时校准：78..105 槽全部挂被动计数，流量最大的候选即真设备
function calibrateAll(candidates) {
  var reports = {};
  var listeners = [];
  candidates.forEach(function (vt) {
    var key = vt.toString();
    reports[key] = { vt: vt, total: 0 };
    for (var s = 78; s <= 105; s++) {
      (function (slot) {
        var rep = reports[key];
        var l = safeAttach(fnAt(vt, slot), {
          onEnter: function (args) {
            rep.total++;
            try {
              if (isShaderBlob(args[1])) {
                rep['b' + slot] = (rep['b' + slot] || 0) + 1;
                return;
              }
              if (looksLikeComObj(args[1])) rep['o' + slot] = (rep['o' + slot] || 0) + 1;
            } catch (e) { }
          }
        }, 'cal@' + key + '@' + slot);
        if (l) listeners.push(l);
      })(s);
    }
  });
  log('多候选校准中（20s，' + candidates.length + ' 个候选 × 28 槽）——请在城里走动……');
  setTimeout(function () {
    listeners.forEach(function (l) { l.detach(); });
    var best = null, bestN = 0;
    for (var k in reports) {
      if (reports[k].total > bestN) { bestN = reports[k].total; best = k; }
    }
    if (best === null || bestN < 20) {
      log('校准：无活跃候选（最高流量 ' + bestN + '）——回报此日志');
      return;
    }
    var rep = reports[best], vt = rep.vt;
    var createSlots = [], bindSlots = [], hotSlots = [];
    for (var s2 = 78; s2 <= 105; s2++) {
      if (rep['b' + s2]) createSlots.push(s2);
      if (rep['o' + s2]) bindSlots.push(s2);
      if (rep.total > 0) { /* 汇总热槽 */ }
    }
    for (var s3 = 78; s3 <= 105; s3++) {
      var n = (rep['b' + s3] || 0) + (rep['o' + s3] || 0);
      if (n > 0) hotSlots.push(s3 + ':' + n);
    }
    log('胜出 vftable ' + best + '（总流量 ' + bestN + '）热槽=' + hotSlots.join(','));
    log('create=' + JSON.stringify(createSlots) + ' bind=' + JSON.stringify(bindSlots));
    if (createSlots.length === 0 && bindSlots.length === 0) {
      log('胜者无 shader 槽位命中——回报此日志');
      return;
    }
    armed = true;
    createSlots.forEach(function (s4) { hookCreate(vt, s4); });
    bindSlots.forEach(function (s4) { hookBind(vt, s4); });
    log('已装载 hooks（实测槽位）：create@' + JSON.stringify(createSlots) +
        ' bind@' + JSON.stringify(bindSlots));
  }, 20000);
}

// ---------- 侦察：多候选 × 代表性槽位的流量分布，人工定槽 ----------
var RECON_SLOTS = [17, 18, 39, 40, 78, 79, 80, 81, 88, 89, 97, 98];

function reconAll(candidates) {
  var reports = {};
  var listeners = [];
  candidates.forEach(function (vt) {
    var key = vt.toString();
    reports[key] = { vt: vt, slots: {} };
    RECON_SLOTS.forEach(function (slot) {
      var rep = reports[key];
      var l = safeAttach(fnAt(vt, slot), {
        onEnter: function (args) {
          var s = rep.slots[slot] || (rep.slots[slot] = { n: 0, blob: 0, obj: 0 });
          s.n++;
          try {
            if (isShaderBlob(args[1])) s.blob++;
            else if (slot === 88 || slot === 89 || slot === 97 || slot === 98) {
              if (looksLikeComObj(args[1])) s.obj++;
            }
          } catch (e) { }
        }
      }, 'recon@' + key + '@' + slot);
      if (l) listeners.push(l);
    });
  });
  log('侦察中（15s，' + candidates.length + ' 候选 × ' + RECON_SLOTS.length + ' 槽）——保持城市场景渲染……');
  setTimeout(function () {
    listeners.forEach(function (l) { l.detach(); });
    var any = false;
    var winner = null, winnerScore = -1;
    for (var k in reports) {
      var rep = reports[k];
      var parts = [];
      for (var s in rep.slots) {
        var info = rep.slots[s];
        if (info.n > 0) {
          any = true;
          parts.push(s + ':' + info.n + (info.blob ? 'b' : '') + (info.obj ? 'o' : ''));
        }
      }
      if (parts.length) log('候选 ' + k + ' → ' + parts.join(' '));
      // 设备判据：Present 候选槽（17/18）计数 = 帧频量级（50..5000），且有对象参数槽
      var p17 = rep.slots[17] ? rep.slots[17].n : 0;
      var p18 = rep.slots[18] ? rep.slots[18].n : 0;
      var presentish = (p17 >= 50 && p17 <= 5000) || (p18 >= 50 && p18 <= 5000);
      var hasObj = false;
      [88, 89, 97, 98].forEach(function (slot) {
        if (rep.slots[slot] && rep.slots[slot].obj > 0) hasObj = true;
      });
      if (presentish && hasObj) {
        var score = Math.min(p17, p18);
        if (score > winnerScore) { winnerScore = score; winner = k; }
      }
    }
    if (!any) { log('侦察：所有候选全零——回报此日志'); return; }
    // 数据驱动装载：侦察中所有"对象参数经过"的 (候选,槽) 全部装 bind hook，
    // "blob 经过"的装 create hook——hook 自带版本 token 校验，错误槽位自然零产出
    armed = true;
    var installed = 0;
    for (var k2 in reports) {
      var rep2 = reports[k2], vt2 = rep2.vt;
      for (var s2 = 0; s2 < RECON_SLOTS.length; s2++) {
        var slot = RECON_SLOTS[s2];
        var info = rep2.slots[slot];
        if (!info) continue;
        // blob 经过的槽全部装 create hook（v14：不再限定 88/97——
        // 实测 blob 流出现在非标准槽位，如 0x6c2f14ec@80）
        if (info.blob > 0) {
          hookCreate(vt2, slot);
          installed++;
          log('装载 create@' + slot + '（vftable ' + vt2 + '，侦察期 blob=' + info.blob + '）');
        }
        if (info.obj > 0 && (slot === 89 || slot === 98)) {
          hookBind(vt2, slot);
          installed++;
          log('装载 bind@' + slot + '（vftable ' + vt2 + '，侦察期 obj=' + info.obj + '）');
        }
      }
    }
    log('已按侦察数据装载 ' + installed + ' 个 hook——进城走动即可触发落盘');
  }, 15000);
}

// ---------- 槽位校准 ----------
function installSlotCounters(vt, seconds) {
  var report = {};
  var listeners = [];
  for (var s = 83; s <= 105; s++) {
    (function (slot) {
      var r = { blob: 0, obj: 0 };
      report[slot] = r;
      var l = safeAttach(fnAt(vt, slot), {
        onEnter: function (args) {
          try {
            if (isShaderBlob(args[1])) { r.blob++; return; }
            if (looksLikeComObj(args[1])) r.obj++;
          } catch (e) { }
        }
      }, 'counter@' + slot);
      if (l) listeners.push(l);
    })(s);
  }
  log('槽位校准中（' + seconds + 's）——请在城里走动/看场景……');
  setTimeout(function () {
    listeners.forEach(function (l) { l.detach(); });
    var createSlots = [], bindSlots = [];
    for (var slot in report) {
      if (report[slot].blob > 0) createSlots.push(+slot);
      if (report[slot].obj > 0) bindSlots.push(+slot);
    }
    log('校准结果：create 槽=' + JSON.stringify(createSlots) +
        ' bind 候选槽=' + JSON.stringify(bindSlots));
    if (createSlots.length === 0 && bindSlots.length === 0) {
      log('校准无命中——回报此日志');
      return;
    }
    armed = true;
    createSlots.forEach(function (s) { hookCreate(vt, s); });
    bindSlots.forEach(function (s) { hookBind(vt, s); });
    log('已装载 hooks（实测槽位）：create@' + JSON.stringify(createSlots) +
        ' bind@' + JSON.stringify(bindSlots));
  }, seconds * 1000);
}

// ---------- hooks ----------
function hookCreate(vt, slot) {
  var fn = fnAt(vt, slot);
  var key = fn.toString();
  if (hookedFns.has(key)) return;
  hookedFns.add(key);
  safeAttach(fn, {
    onEnter: function (args) {
      this.hit = isShaderBlob(args[1]);
      if (this.hit) { this.blob = args[1]; this.pp = args[2]; }
    },
    onLeave: function () {
      if (!this.hit) return;
      try {
        stats.create++;
        var len = this.blob.readU32();
        var ver = this.blob.add(4).readU32();
        var hi = ver >>> 16;
        var obj = 'null';
        if (!this.pp.isNull()) obj = this.pp.readPointer().toString();
        send({
          kind: 'shader', type: hi === 0xFFFF ? 'ps' : 'vs',
          source: 'create', slot: slot,
          obj: obj, ver: '0x' + ver.toString(16), size: len
        }, this.blob.readByteArray(len));
      } catch (e) { log('create 落盘失败: ' + e); }
    }
  }, 'create@' + slot);
}

function hookBind(vt, slot) {
  var fn = fnAt(vt, slot);
  var key = fn.toString();
  if (hookedFns.has(key)) return;
  hookedFns.add(key);
  safeAttach(fn, {
    onEnter: function (args) {
      var obj = args[1];
      if (obj.isNull() || obj.compare(SMALL_MAX) < 0) return;
      var k = obj.toString();
      if (seenBind.has(k)) return;
      if (!looksLikeComObj(obj)) return; // 槽位不准时在此拦下非 COM 对象
      seenBind.add(k); // 无论成败只试一次，防失败刷屏
      try {
        // shader/声明对象的 GetFunction/GetDeclaration 同在槽 4
        var getFn = new NativeFunction(obj.readPointer().add(16).readPointer(),
          'int32', ['pointer', 'pointer', 'pointer']);
        if (getFn(obj, ptr(0), sizeP) !== 0) return;
        var size = sizeP.readU32();
        if (size < 16 || size > (4 << 20) || (size & 3) !== 0) return;
        var bufp = Memory.alloc(size);
        if (getFn(obj, bufp, sizeP) !== 0) return;
        var ver = bufp.add(4).readU32();
        var hi = ver >>> 16;
        if (hi !== 0xFFFF && hi !== 0xFFFE) return; // 顶点声明等非 shader 字节码——丢弃
        stats.bind++;
        send({
          kind: 'shader', type: hi === 0xFFFF ? 'ps' : 'vs',
          source: 'bind', slot: slot, obj: k,
          ver: '0x' + ver.toString(16), size: size
        }, bufp.readByteArray(size));
      } catch (e) {
        if (getFnFailLog++ < 3) log('GetFunction 失败 ' + k + ': ' + e);
      }
    }
  }, 'bind@' + slot);
}

// ---------- F. 精确制导：从游戏自己的 decal 绘制函数里挖设备指针 ----------
// FUN_00437610 = Ghidra 已定谳的 decal draw 链函数；SimCity.exe 无 ASLR（基址
// 0x400000），该地址在活进程直接有效。其 this 对象体里必然持有 d3d9 COM 对象，
// 其中只有设备拥有 110+ 槽 vftable（纹理/顶点缓冲等被 spotCheck 过滤）。
function hookGameDraw() {
  var game = Process.findModuleByName('SimCity.exe');
  if (game === null) { log('F: 找不到 SimCity.exe 模块'); return; }
  var target = game.base.add(0x37610); // VA 0x437610
  var walked = false;
  var l = safeAttach(target, {
    onEnter: function (args) {
      if (deviceFound || walked) return;
      walked = true;
      var thiz = args[0];
      log('F: draw 函数首次触发，this=' + thiz);
      if (thiz.isNull()) return;
      try {
        var body = thiz.readByteArray(0x2000);
        var u32 = new Uint32Array(body);
        for (var i = 0; i < u32.length; i++) {
          var v = u32[i];
          if (v < 0x10000) continue;
          if (v < modLoN || v >= modHiN) continue;
          var vt = ptr(v);
          if (!spotCheck(vt)) continue;
          var rep = validateReport(vt);
          if (!rep.ok) continue;
          deviceFound = true;
          log('F: this+' + (i * 4) + ' 处发现设备对象 → vftable ' + vt +
              '（exec ' + rep.exec + '/' + rep.n + '）');
          installSlotCounters(vt, 20);
          return;
        }
        log('F: this 体 0x2000 字节内未发现设备 vftable——可扩大范围或换锚点');
      } catch (e) { log('F: 读取 this 失败 ' + e); }
    }
  }, 'gameDraw');
}

// ---------- 主流程 ----------
function arm() {
  computeExecRanges();
  if (execLo === null) {
    log('d3d9 内未找到 r-x 段——回报此日志');
    return;
  }
  log('exec 段: ' + execLo + ' - ' + execHi);
  lurkDirect3DCreate();
  hookGameDraw();
  // 15s 内 F/A 都没定位到设备 → 全堆扫描 + 侦察定槽
  setTimeout(function () {
    if (deviceFound || armed) return;
    log('15s 未定位设备，走全堆扫描+侦察…');
    var candidates = scanHeapV2();
    if (candidates.length === 0) {
      log('堆扫描未命中——回报此日志');
      return;
    }
    deviceFound = true;
    reconAll(candidates);
  }, 15000);
}

// d3d9.dll 可能尚未加载（spawn 模式）：轮询等待
var pollTries = 0;
var poller = setInterval(function () {
  var m = Process.findModuleByName('d3d9.dll');
  if (m) {
    clearInterval(poller);
    d3d9 = m;
    log('d3d9.dll @ ' + m.base + ' size=0x' + m.size.toString(16));
    arm();
    return;
  }
  if (++pollTries > 600) {
    clearInterval(poller);
    log('600s 内未见 d3d9.dll 加载，放弃');
  }
}, 250);

setInterval(function () {
  send({ kind: 'stats', stats: stats });
}, 5000);

recv('config', function onCfg(m) {
  cfg.spawn = !!m.spawn;
  recv('config', onCfg);
});
