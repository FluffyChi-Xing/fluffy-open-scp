
var dumps = [];
var dumpCount = 0;
var slotStats = {};
var round = 0;

setTimeout(probe, 40000);
function probe() {
  var d3d9m = Process.findModuleByName('d3d9.dll');
  if (!d3d9m) { send({ kind: 'log', text: 'd3d9 未加载，10s 后重试' }); setTimeout(probe, 10000); return; }
  var lo = parseInt(d3d9m.base.toString(16), 16);
  var hi = parseInt(d3d9m.base.add(d3d9m.size).toString(16), 16);
  var cands = [], seen = {};
  var ranges = Process.enumerateRanges('rw-');
  var CHUNK = 64 << 20;
  ranges.forEach(function (r) {
    if (cands.length >= 60) return;
    for (var off = 0; off < r.size && cands.length < 60; off += CHUNK) {
      var len = Math.min(CHUNK, r.size - off);
      var buf;
      try { buf = r.base.add(off).readByteArray(len); } catch (e) { break; }
      var u32 = new Uint32Array(buf);
      for (var i = 0; i < u32.length && cands.length < 60; i++) {
        var v = u32[i];
        if (v < 0x10000 || (v & 3) !== 0) continue;
        if (v < lo || v >= hi) continue;
        var vt = ptr(v);
        var k = vt.toString();
        if (seen[k]) continue;
        var ok = true;
        try {
          for (var t = 0; t < 5; t++) {
            var fn = vt.add(t * 4).readU32();
            if (fn < lo || fn >= hi) { ok = false; break; }
          }
        } catch (e) { ok = false; }
        if (!ok) continue;
        seen[k] = true;
        cands.push(vt);
      }
    }
  });
  send({ kind: 'log', text: '第 ' + round + ' 轮扫描：候选 ' + cands.length });
  if (cands.length < 10) {
    round++;
    if (round <= 10) setTimeout(probe, 30000);
    else send({ kind: 'log', text: '10 轮仍少候选——回报' });
    return;
  }
  var SLOTS = [17, 18, 62];
  for (var d = 78; d <= 81; d++) SLOTS.push(d);
  for (var e = 86; e <= 101; e++) SLOTS.push(e);
  var hooks = [];
  cands.forEach(function (vt) { SLOTS.forEach(function (slot) { hooks.push([vt, slot]); }); });
  var idx = 0;
  function installBatch() {
    for (var n = 0; n < 300 && idx < hooks.length; n++, idx++) {
      (function (vt, slot) {
        try {
          Interceptor.attach(vt.add(slot * 4).readPointer(), {
            onEnter: function (args) {
              slotStats[slot] = (slotStats[slot] || 0) + 1;
              var obj = args[1];
              if (obj.isNull() || obj.compare(ptr('0x10000')) < 0) return;
              var kk = obj.toString();
              if (this.seen) return;
              this.seen = true;
              try {
                var objVt = obj.readPointer();
                var ov0 = parseInt(objVt.toString(16), 16);
                if (ov0 < lo || ov0 >= hi) return;
                var getFn = new NativeFunction(objVt.add(16).readPointer(),
                  'int32', ['pointer', 'pointer', 'pointer']);
                var sizeP = Memory.alloc(8);
                sizeP.writeU32(0);
                if (getFn(obj, ptr(0), sizeP) !== 0) return;
                var size = sizeP.readU32();
                if (size < 16 || size > 0x10000 || (size & 3) !== 0) return;
                var bufp = Memory.alloc(size);
                bufp.writeU32(0); bufp.add(4).writeU32(0);
                if (getFn(obj, bufp, sizeP) !== 0) return;
                var ver = bufp.add(4).readU32();
                var hiw = ver >>> 16;
                if (hiw !== 0xFFFF && hiw !== 0xFFFE) return;
                dumpCount++;
                var bytes = new Uint8Array(bufp.readByteArray(size));
                var bin = '';
                for (var i = 0; i < bytes.length; i += 0x8000) {
                  bin += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
                }
                send({ kind: 'shader', vt: vt.toString(), slot: slot, obj: kk,
                       ver: '0x' + ver.toString(16), b64: btoa(bin) });
              } catch (err) {}
            }
          });
        } catch (err) {}
      })(hooks[idx][0], hooks[idx][1]);
    }
    if (idx < hooks.length) setTimeout(installBatch, 50);
    else {
      send({ kind: 'log', text: '全槽 sweep 就绪：' + hooks.length + ' 钩子——捕获 180s' });
      setTimeout(function () { send({ kind: 'done', slotStats: slotStats, dumps: dumpCount }); }, 180000);
    }
  }
  installBatch();
}
