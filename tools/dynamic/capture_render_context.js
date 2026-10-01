'use strict';
// =====================================================================
// capture_render_context.js — 渲染上下文捕获（frida 17.x，x86）
// 配套主机端：tools/dynamic/capture_states.py
//
// 目标（2026-10-01，decal Tier-3 缺口）：
//   1. 编译后最终 shader 字节码（组合产物——引擎的片段链组合结果，
//      免去静态重组表）；
//   2. 每族渲染态（SetRenderState 流，含 blend/depth/cull）；
//   3. 常量实值（SetVertexShaderConstantF/SetPixelShaderConstantF，
//      含 decalMaterialData/模型矩阵/texXform）；
//   4. **decal pass 标记**：hook cVolumeDecalManager::FUN_006fd730
//      （VA 0x6FD730，Ghidra 已定谳的 draw 分发）——窗口内每条 draw/常量
//      事件带 decalActive 标记，离线即可精确提取 decal 族的状态剖面。
//
// 设备定位复用 hook_d3d9.js v7 的实测机制（不猜槽位）：
//   A. 随行观察 Direct3DCreate9 → CreateDevice；C2. 多设备永久监视
//   （15s 巡视器，谁家有流量武装谁）；G/C. .data 行走 + 全堆扫描兜底。
//   在 v7 的被动计数器上扩展签名分类（RS/常量/绘制/帧），巡视器按
//   频次+纯度择优武装有源 hook。
//
// 防崩纪律（tools/dynamic/README.md）：被动计数器照 v7 惯例全槽位；
// 有源 hook 仅 5 类（RS×1 / 常量×2 / 绘制×1 / 帧×1）+ bind 身份跟踪
// + 静态 decal 分发 1 处；事件全部 in-window 发送 + 去重，防烧 CPU。
// =====================================================================

var SMALL_MAX = ptr('0x10000');

var d3d9 = null;
var execLo = null, execHi = null;
var modLoN = 0, modHiN = 0;
var deviceFound = false;
var cfg = { spawn: false };

// ---------- 捕获状态 ----------
var win = { until: 0 };          // 捕获窗口（主机端控制 start/seconds）
var frame = 0;                   // 帧计数（Present/EndScene 槽递增）
var decalDepth = 0;              // FUN_006fd730 重入深度（>0 = decal pass 内）
var curPS = 'null', curVS = 'null'; // 当前绑定 shader 身份（join key = obj 指针串）
var shadowRS = {};               // SetRenderState 影子表（只记录变更）
var d3dShadow = {};              // 真 D3D9 SetRenderState 影子表（双层同录）
var lastConst = {};              // 常量去重（tag:start:count → 签名）
var seenBindState = {};          // bind 对象首见去重（字节码 dump 一次）
var stats = { ctx: 0, shaders: 0, draws: 0 };
var DRAW_SAMPLE = 1;             // 窗口内 draw 采样步长（量大时可调）

function log(text) { send({ kind: 'log', text: text }); }
function inWindow() { return Date.now() < win.until; }
function emitCtx(ev) {
  if (!inWindow()) return;
  stats.ctx++;
  ev.f = frame;
  send(ev);
}

// ---------- 基础工具（v7 同款） ----------
function inMod(p) {
  return p.compare(d3d9.base) >= 0 && p.compare(d3d9.base.add(d3d9.size)) < 0;
}
var gameRange = null;
function inGameMod(p) {
  if (gameRange === null) {
    var g = Process.findModuleByName('SimCity.exe');
    gameRange = g
      ? { lo: parseInt(g.base.toString(16), 16), hi: parseInt(g.base.add(g.size).toString(16), 16) }
      : { lo: 1, hi: 0 };
  }
  var a = parseInt(p.toString(16), 16);
  return a >= gameRange.lo && a < gameRange.hi;
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
function isShaderBlob(p) {
  if (p.isNull() || p.compare(SMALL_MAX) < 0) return false;
  try {
    var len = p.readU32();
    if (len < 16 || len > (4 << 20) || (len & 3) !== 0) return false;
    var hi = p.add(4).readU32() >>> 16;
    return hi === 0xFFFF || hi === 0xFFFE;
  } catch (e) { return false; }
}
function structComObj(p) {
  if (p.isNull() || p.compare(SMALL_MAX) < 0) return false;
  try {
    var vt = p.readPointer();
    if (vt.isNull()) return false;
    var n = 0;
    for (var i = 0; i < 8; i++) {
      if (!vt.add(i * 4).readPointer().isNull()) n++;
    }
    return n >= 4;
  } catch (e) { return false; }
}
// quickComObj（v7）：对象自身 vtable 前 24 槽 exec 密度 ≥80%——GetFunction
// 这类 NativeFunction 调用前的强制预检。2026-10-01 崩溃教训：仅 structComObj
// （8 槽非空）挡不住堆垃圾对象，GetFunction 打到垃圾内存 = illegal instruction
// = 游戏崩溃（README 铁律 2）。
// 执行归属 = d3d9 exec **或 SimCity.exe 代码段**——破解包装层的 shader 对象
// vtable 条目指向游戏模块（2026-10-01 实测：只认 d3d9 会把真 shader 全误杀，
// shader=0 的根因）。
function quickComObj(p) {
  if (p.isNull() || p.compare(SMALL_MAX) < 0) return false;
  try {
    var vt = p.readPointer();
    var exec = 0;
    for (var i = 0; i < 24; i++) {
      var fn = vt.add(i * 4).readPointer();
      if (!fn.isNull() && (inExec(fn) || inGameMod(fn))) exec++;
    }
    return exec >= 19;
  } catch (e) { return false; }
}
function validateReport(vt) {
  var exec = 0, n = 0;
  try {
    for (var i = 0; i < 130; i++) {
      var fn = vt.add(i * 4).readPointer();
      n++;
      if (!fn.isNull() && inExec(fn)) exec++;
    }
  } catch (e) { }
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
function toI(p) { try { return p.toInt32(); } catch (e) { return 0x7fffffff; } }

// ---------- A/C2. 设备定位（v7 精简移植） ----------
function lurkDirect3DCreate() {
  ['Direct3DCreate9', 'Direct3DCreate9Ex'].forEach(function (name) {
    var exp = null;
    try { exp = getExport(d3d9, name); } catch (e) { return; }
    safeAttach(exp, {
      onLeave: function (ret) {
        if (!ret.isNull() && !deviceFound) {
          log(name + ' -> IDirect3D9* = ' + ret);
          lurkCreateDevice(ret);
        }
      }
    }, name);
  });
}
function earlyLurk() {
  try {
    var k32 = Process.findModuleByName('kernel32.dll');
    if (k32 === null) return;
    var CreateThread = new NativeFunction(getExport(k32, 'CreateThread'),
      'pointer', ['pointer', 'uint', 'pointer', 'pointer', 'uint32', 'pointer']);
    var WaitForSingleObject = new NativeFunction(getExport(k32, 'WaitForSingleObject'),
      'uint32', ['pointer', 'uint32']);
    var GetExitCodeThread = new NativeFunction(getExport(k32, 'GetExitCodeThread'),
      'uint32', ['pointer', 'pointer']);
    var CloseHandle = new NativeFunction(getExport(k32, 'CloseHandle'), 'int32', ['pointer']);
    var tid = Memory.alloc(4);
    var hThread = CreateThread(ptr(0), 0, getExport(d3d9, 'Direct3DCreate9'), ptr(32), 0, tid);
    if (hThread.isNull()) return;
    WaitForSingleObject(hThread, 15000);
    var exitCode = Memory.alloc(4);
    GetExitCodeThread(hThread, exitCode);
    CloseHandle(hThread);
    var pD3D = ptr(exitCode.readU32());
    log('E: 抢先自建 IDirect3D9=' + pD3D);
    if (!pD3D.isNull()) lurkCreateDevice(pD3D);
  } catch (e) { log('E: 异常 ' + e); }
}
function lurkCreateDevice(pD3D) {
  var vt = pD3D.readPointer();
  for (var s = 3; s <= 16; s++) {
    (function (slot) {
      safeAttach(fnAt(vt, slot), {
        onEnter: function (args) { this.p6 = args[6]; this.p7 = args[7]; },
        onLeave: function () {
          if (deviceFound) return;
          var self = this;
          [self.p6, self.p7].forEach(function (pp) {
            if (deviceFound || pp === undefined || pp.isNull()) return;
            try {
              var obj = pp.readPointer();
              if (obj.isNull()) return;
              var ovt = obj.readPointer();
              var rep = validateReport(ovt);
              if (!rep.ok) return;
              deviceFound = true;
              watchDevice(ovt, rep);
              // 真 D3D9 设备：额外武装公共接口 SetRenderState（slot 54）——
              // 与引擎层状态（包装层槽）双层同录，离线对齐即得翻译表
              var l54 = safeAttach(fnAt(ovt, 54), {
                onEnter: function (args) {
                  try {
                    var st = toI(args[1]);
                    if (st < 0 || st > 255) return;
                    var va = toI(args[2]) >>> 0;
                    if (d3dShadow[st] === va) return;
                    d3dShadow[st] = va;
                    if (inWindow()) emitCtx({ kind: 'ctx', e: ['d3drs', st, va, curPS] });
                  } catch (e) { }
                }
              }, 'd3drs@54');
              if (l54) log('真设备 D3D9 SetRenderState(54) 已武装（双层同录）');
            } catch (e) { }
          });
        }
      }, 'lurk@' + slot);
    })(s);
  }
}

// ---------- C2. 永久监视 + 签名分类（本脚本核心扩展） ----------
// 计数器字段：n=总流量 blob/obj=v7 原有分类；rsP/rs=SetRenderState 签名
// 命中/纯命中；cf=常量签名；draw=绘制签名。
var watchedDevices = {};
function classify(slot, r, args) {
  r.n++;
  try {
    var a1 = args[1];
    if (isShaderBlob(a1)) { r.blob++; return; }
    if (structComObj(a1)) { r.obj++; }
  } catch (e) { }
  try {
    var v1 = toI(args[1]);
    if (v1 >= 0 && v1 < 256) {
      var v2 = toI(args[2]) >>> 0;
      if (v2 < 0x10000) { r.rs++; r.rsPure++; }
      else { r.rs++; }
    }
    if (v1 >= 0 && v1 < 128 && !args[2].isNull() && args[2].compare(SMALL_MAX) >= 0) {
      var v3 = toI(args[3]);
      if (v3 >= 1 && v3 <= 64) r.cf++;
    }
    // 绘制签名：args[3..7] 全为有界小整数（DrawIndexedPrimitive 形态）
    var drawish = true;
    for (var i = 3; i <= 7; i++) {
      var vi = toI(args[i]);
      if (vi > (1 << 24) || vi < -(1 << 24)) { drawish = false; break; }
    }
    if (drawish) r.draw++;
  } catch (e) { }
}
function watchDevice(ovt, rep) {
  var key = ovt.toString();
  if (watchedDevices[key]) return;
  var w = { counters: {}, armed: false, armedBind: [], stateArmed: false };
  watchedDevices[key] = w;
  for (var s = 3; s <= 129; s++) {
    (function (slot) {
      var r = { n: 0, blob: 0, obj: 0, rs: 0, rsPure: 0, cf: 0, draw: 0 };
      w.counters[slot] = r;
      var l = safeAttach(fnAt(ovt, slot), {
        onEnter: function (args) {
          try { classify(slot, r, args); } catch (e) { }
        }
      }, 'watch@' + key + '@' + slot);
    })(s);
  }
  log('实测设备 ' + key + '（exec ' + rep.exec + '/' + rep.n + '）进入监视——15s 巡视器将按流量武装');
}

var trafficTicks = 0;
setInterval(function () {
  for (var key in watchedDevices) {
    var w = watchedDevices[key];
    var vt = ptr(key);
    var createSlots = [], bindSlots = [];
    for (var s in w.counters) {
      var r = w.counters[s];
      if (r.blob > 0) createSlots.push(+s);
      if (r.obj > 0) bindSlots.push(+s);
    }
    if (!w.armed && createSlots.length > 0) {
      w.armed = true;
      createSlots.forEach(function (s2) { hookCreate(vt, s2); });
      log('武装 create@' + JSON.stringify(createSlots) + '（设备 ' + key + '）');
    }
    var newBind = bindSlots.filter(function (s3) { return w.armedBind.indexOf(+s3) < 0; });
    if (newBind.length > 0) {
      newBind.forEach(function (s4) { w.armedBind.push(+s4); hookBind(vt, +s4); });
      log('武装 bind@' + JSON.stringify(newBind));
    }
    // 状态槽武装（频次+纯度择优，每设备一次）。
    // attach 模式下 create 流量可能迟迟不来（shader 已编译完）——bind 对象
    // 流量（=渲染进行中）同样满足武装条件。
    if (!w.stateArmed && (w.armed || bindSlots.length > 0)) {
      w.stateArmed = true;
      armStateHooks(vt, w.counters);
    }
    trafficTicks++;
    if (trafficTicks % 4 === 0) {
      var hot = [];
      for (var s5 in w.counters) {
        var r2 = w.counters[s5];
        if (r2.n > 0) hot.push(s5 + ':' + r2.n + '/rs' + r2.rs + '/cf' + r2.cf + '/d' + r2.draw);
      }
      hot.sort(function (a, b) {
        return parseInt(b.split(':')[1], 10) - parseInt(a.split(':')[1], 10);
      });
      log('交通图[' + key + '] ' + hot.slice(0, 12).join(' '));
    }
  }
}, 15000);

// ---------- 状态槽择优武装 ----------
function armStateHooks(vt, counters) {
  var rsBest = null, cf1 = null, cf2 = null, drawBest = null, frameSlot = null;
  for (var s in counters) {
    var r = counters[+s];
    if (r.rs >= 200) {
      // RS：纯度优先（全部命中 0..255/值域），其次频次
      var purity = r.rsPure / Math.max(r.rs, 1);
      if (rsBest === null || purity > rsBest.purity ||
          (purity === rsBest.purity && r.rs > rsBest.r.rs)) {
        rsBest = { slot: +s, r: r, purity: purity };
      }
    }
    if (r.cf >= 100) {
      if (cf1 === null || r.cf > counters[cf1].cf) { cf2 = cf1; cf1 = +s; }
      else if (cf2 === null || r.cf > counters[cf2].cf) { cf2 = +s; }
    }
    if (r.draw >= 500) {
      if (drawBest === null || r.draw > counters[drawBest].draw) drawBest = +s;
    }
    // 帧槽：流量在帧频量级（17/18 为标准 Present 槽）
    if (r.n >= 50 && r.n <= 5000 && (+s === 17 || +s === 18)) frameSlot = +s;
  }
  if (rsBest !== null) { hookRS(vt, rsBest.slot); log('武装 RS@' + rsBest.slot + '（rs=' + rsBest.r.rs + ' 纯度=' + (rsBest.purity * 100).toFixed(0) + '%）'); }
  else log('未找到 RS 槽（交通图回报分析）');
  if (cf1 !== null) { hookConstF(vt, cf1, 'cfA'); log('武装 常量@' + cf1 + '（cf=' + counters[cf1].cf + '）'); }
  if (cf2 !== null) { hookConstF(vt, cf2, 'cfB'); log('武装 常量@' + cf2 + '（cf=' + counters[cf2].cf + '）'); }
  if (drawBest !== null) { hookDraw(vt, drawBest); log('武装 Draw@' + drawBest + '（draw=' + counters[drawBest].draw + '）'); }
  if (frameSlot !== null) { hookFrame(vt, frameSlot); log('武装 帧@' + frameSlot); }
  else log('未找到帧槽（帧号将以 0 记）');
}

// ---------- 有源 hooks ----------
function hookRS(vt, slot) {
  safeAttach(fnAt(vt, slot), {
    onEnter: function (args) {
      try {
        var st = toI(args[1]);
        if (st < 0 || st > 255) return;
        var va = toI(args[2]) >>> 0;
        if (shadowRS[st] === va) return;   // 影子表去重：只记变更
        shadowRS[st] = va;
        if (inWindow()) emitCtx({ kind: 'ctx', e: ['rs', st, va, curPS, curVS] });
      } catch (e) { }
    }
  }, 'rs@' + slot);
}

function constSig(p, count) {
  // 便宜签名：首/尾各一个 u32（避免每调用整段读→烧 CPU）
  var head = p.readU32();
  var tail = count > 1 ? p.add((count - 1) * 16).readU32() : 0;
  return head + ':' + tail;
}
function hookConstF(vt, slot, tag) {
  safeAttach(fnAt(vt, slot), {
    onEnter: function (args) {
      try {
        var start = toI(args[1]);
        var count = toI(args[3]);
        if (start < 0 || start > 4096 || count < 1 || count > 64) return;
        var p = args[2];
        var key = tag + ':' + start + ':' + count;
        var sig = constSig(p, count);
        if (lastConst[key] === sig) return;
        var buf = p.readByteArray(count * 16);
        // 全量字节比较防签名碰撞误报（频次低，可负担）
        var hex = Array.prototype.map.call(new Uint8Array(buf),
          function (b) { return ('0' + b.toString(16)).slice(-2); }).join('');
        if (lastConst[key] === hex) { lastConst[key] = hex; return; }
        lastConst[key] = hex;
        if (inWindow()) {
          send({ kind: 'ctx', e: ['cf', tag, start, count, curPS, curVS], f: frame }, buf);
        }
      } catch (e) { }
    }
  }, tag + '@' + slot);
}

function hookDraw(vt, slot) {
  safeAttach(fnAt(vt, slot), {
    onEnter: function (args) {
      if (!inWindow()) return;
      stats.draws++;
      if (stats.draws % DRAW_SAMPLE !== 0) return;
      emitCtx({ kind: 'ctx', e: ['draw', curPS, curVS, decalDepth > 0 ? 1 : 0] });
    }
  }, 'draw@' + slot);
}

function hookFrame(vt, slot) {
  safeAttach(fnAt(vt, slot), {
    onEnter: function () { frame++; }
  }, 'frame@' + slot);
}

// bind = v7 的字节码落盘 + 本脚本新增的「当前身份」跟踪
function setCur(type, id) { if (type === 'ps') curPS = id; else curVS = id; }
var sizeP = Memory.alloc(4);
var getFnFailLog = 0;
function hookBind(vt, slot) {
  safeAttach(fnAt(vt, slot), {
    onEnter: function (args) {
      try {
        var obj = args[1];
        if (obj.isNull() || obj.compare(SMALL_MAX) < 0) return;
        var k = obj.toString();
        // 粗判 VS/PS：由版本 token 定（首见时），身份跟踪先按对象记
        var isPS = null;
        if (!seenBindState[k]) {
          // 双重预检：结构（8 槽非空）+ 执行密度（24 槽 exec≥80%）——
          // 不过者绝不调 GetFunction（崩溃教训，见 quickComObj 注释）
          if (!structComObj(obj) || !quickComObj(obj)) return;
          seenBindState[k] = '?';
          try {
            var getFn = new NativeFunction(obj.readPointer().add(16).readPointer(),
              'int32', ['pointer', 'pointer', 'pointer']);
            if (getFn(obj, ptr(0), sizeP) !== 0) return;
            var size = sizeP.readU32();
            if (size < 16 || size > (4 << 20) || (size & 3) !== 0) return;
            var bufp = Memory.alloc(size);
            bufp.writeU32(0); bufp.add(4).writeU32(0);
            if (getFn(obj, bufp, sizeP) !== 0) return;
            var ver = bufp.add(4).readU32();
            var hi = ver >>> 16;
            if (hi !== 0xFFFF && hi !== 0xFFFE) return; // 顶点声明等非 shader——丢弃
            seenBindState[k] = hi === 0xFFFF ? 'ps' : 'vs';
            stats.shaders++;
            send({ kind: 'shader', type: seenBindState[k], source: 'bind', slot: slot,
                   obj: k, ver: '0x' + ver.toString(16), size: size },
                 bufp.readByteArray(size));
          } catch (e) {
            if (getFnFailLog++ < 3) log('GetFunction 失败 ' + k + ': ' + e);
            return;
          }
        }
        var t = seenBindState[k];
        if (t === 'ps' || t === 'vs') setCur(t, k);
      } catch (e) { }
    }
  }, 'bind@' + slot);
}

function hookCreate(vt, slot) {
  safeAttach(fnAt(vt, slot), {
    onEnter: function (args) {
      this.hit = isShaderBlob(args[1]);
      if (this.hit) { this.blob = args[1]; this.pp = args[2]; }
    },
    onLeave: function () {
      if (!this.hit) return;
      try {
        var len = this.blob.readU32();
        var ver = this.blob.add(4).readU32();
        var hi = ver >>> 16;
        var obj = 'null';
        if (!this.pp.isNull()) obj = this.pp.readPointer().toString();
        stats.shaders++;
        send({ kind: 'shader', type: hi === 0xFFFF ? 'ps' : 'vs', source: 'create',
               slot: slot, obj: obj, ver: '0x' + ver.toString(16), size: len },
             this.blob.readByteArray(len));
      } catch (e) { }
    }
  }, 'create@' + slot);
}

// ---------- 静态制导：decal pass 标记 ----------
// FUN_006fd730（VA，Ghidra SC_cVolumeDecalManager.c 绘制分发）——SimCity.exe
// 无 ASLR，运行时地址 = 模块基址 + (VA - 0x400000)。重入计数判定 decal pass。
function hookDecalDispatch() {
  var game = Process.findModuleByName('SimCity.exe');
  if (game === null) { log('decal 分发：无 SimCity.exe 模块'); return; }
  var target = game.base.add(0x6FD730 - 0x400000);
  var l = safeAttach(target, {
    onEnter: function () {
      decalDepth++;
      if (decalDepth === 1 && inWindow()) emitCtx({ kind: 'ctx', e: ['mark', 'decalOn'] });
    },
    onLeave: function () {
      decalDepth--;
      if (decalDepth === 0 && inWindow()) emitCtx({ kind: 'ctx', e: ['mark', 'decalOff'] });
    }
  }, 'decalDispatch@0x6FD730');
  if (l) log('decal 分发 hook 已装 @ ' + target + '（VA 0x6FD730）');
}

// ---------- G/C. 兜底探测（v7 移植） ----------
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
  var candidates = [], seen = {};
  var ranges = Process.enumerateRanges('rw-');
  var totalMB = 0;
  ranges.forEach(function (r) { totalMB += r.size / (1024 * 1024); });
  log('C: 全堆分块扫描 ' + ranges.length + ' 段 / ' + Math.round(totalMB) + 'MB…');
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
        if (v < 0x10000 || v < modLoN || v >= modHiN) continue;
        var vt = ptr(v);
        var key = vt.toString();
        if (seen[key]) continue;
        if (!spotCheck(vt)) continue;
        seen[key] = true;
        candidates.push(vt);
      }
    }
  }
  log('C: 候选 ' + candidates.length);
  return candidates;
}
function walkGameData() {
  var game = Process.findModuleByName('SimCity.exe');
  if (game === null) return [];
  var out = [], seenVt = {};
  var ranges = Process.enumerateRanges('rw-').filter(function (r) {
    return r.base.compare(game.base) >= 0 && r.base.compare(game.base.add(game.size)) < 0;
  });
  ranges.forEach(function (r) {
    if (out.length >= 30) return;
    var buf;
    try { buf = r.base.readByteArray(r.size); } catch (e) { return; }
    var u32 = new Uint32Array(buf);
    var tried = {};
    for (var i = 0; i < u32.length && out.length < 30; i++) {
      var P = u32[i];
      if (P < 0x10000 || (P & 3) !== 0 || (P >= modLoN && P < modHiN)) continue;
      var pk = P.toString(16);
      if (tried[pk]) continue;
      tried[pk] = true;
      var body;
      try { body = ptr(P).readByteArray(0x2000); } catch (e) { continue; }
      var b32 = new Uint32Array(body);
      for (var j = 0; j < b32.length; j++) {
        var D = b32[j];
        if (D < modLoN || D >= modHiN) continue;
        var vt = ptr(D);
        var k2 = vt.toString();
        if (seenVt[k2]) continue;
        if (!spotCheck(vt)) continue;
        seenVt[k2] = true;
        var rep = validateReport(vt);
        if (rep.ok) out.push(vt);
      }
    }
  });
  return out;
}
var probeRounds = 0;
function fallbackProbe() {
  if (deviceFound) return;
  probeRounds++;
  if (probeRounds > 8) { log('探测 8 轮无果——回报此日志'); return; }
  log('探测第 ' + probeRounds + ' 轮：.data 对象图行走…');
  var vts = walkGameData();
  var heapVts = scanHeapV2();
  heapVts.forEach(function (vt) {
    if (vts.indexOf(vt) < 0) vts.push(vt);
  });
  if (vts.length > 0) {
    // attach 模式主力路径：强校验候选（exec≥100/130）进永久监视；bind sweep
    // 只扫强校验候选且上限 4 个——2026-10-01 崩溃教训：堆垃圾候选全槽 sweep
    // + GetFunction = illegal instruction 崩溃
    var strong = vts.filter(function (vt2) {
      var rep = validateReport(vt2);
      return rep.ok;
    });
    strong.slice(0, 4).forEach(function (vt2) { watchDevice(vt2, validateReport(vt2)); });
    var swept = 0;
    strong.slice(0, 4).forEach(function (vt3) {
      for (var s = 0; s <= 110; s++) { hookBind(vt3, s); swept++; }
    });
    log('强校验候选 ' + strong.length + '/' + vts.length + '：前 4 进入监视 + bind sweep（' + swept + '）');
    return;
  }
  log('第 ' + probeRounds + ' 轮未命中，30s 后重试…');
  setTimeout(fallbackProbe, 30000);
}

// ---------- 主流程 ----------
function arm() {
  computeExecRanges();
  if (execLo === null) { log('d3d9 内未找到 r-x 段'); return; }
  log('exec 段: ' + execLo + ' - ' + execHi);
  // 已知设备直连（--device）：跳过全部发现逻辑——重挂同一活进程时
  // vftable 已知（如 d3d9 模块内 0x6b501000），直接监视+武装
  if (cfg.deviceVt) {
    var vt = ptr(cfg.deviceVt);
    var rep = validateReport(vt);
    log('已知设备直连 ' + vt + '（exec ' + rep.exec + '/' + rep.n + '）');
    if (rep.ok) {
      deviceFound = true;
      watchDevice(vt, rep);
      for (var s = 0; s <= 110; s++) hookBind(vt, s);
      // 渲染进行中流量即时可得——下轮巡视器即武装状态钩子
    } else {
      log('已知设备校验失败——回退自动发现');
      lurkDirect3DCreate();
    }
    hookDecalDispatch();
    return;
  }
  lurkDirect3DCreate();
  if (cfg.spawn) earlyLurk();
  hookDecalDispatch();
  setTimeout(fallbackProbe, 15000);
}

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
  if (++pollTries > 600) { clearInterval(poller); log('600s 内未见 d3d9.dll'); }
}, 250);

setInterval(function () {
  send({ kind: 'stats', stats: stats, frame: frame, decal: decalDepth > 0,
         ps: curPS, vs: curVS });
}, 5000);

recv('config', function onCfg(m) {
  cfg.spawn = !!m.spawn;
  if (m.deviceVt) cfg.deviceVt = m.deviceVt;
  recv('config', onCfg);
});
recv('control', function onCtl(m) {
  if (m.cmd === 'start') {
    var secs = m.seconds || 15;
    win.until = Date.now() + secs * 1000;
    log('★ 捕获窗口开启 ' + secs + 's——请把镜头对准目标（招牌街/破楼），缓慢转动视角');
  } else if (m.cmd === 'reset') {
    shadowRS = {}; lastConst = {}; seenBindState = {};
    log('影子表已重置（下次窗口将全量重记）');
  }
  recv('control', onCtl);
});
