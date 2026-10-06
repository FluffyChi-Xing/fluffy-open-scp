#!/usr/bin/env python3
"""test_read_primitive.py — 诊断 frida 对 32 位游戏的大块堆读失败问题。"""
import frida, sys, time

JS = r"""
var results = {};
function testRanges() {
  var ranges = Process.enumerateRanges('rw-');
  var totalMB = 0;
  ranges.forEach(function (r) { totalMB += r.size / 1048576; });
  send({ kind: 'log', text: 'rw- 范围 ' + ranges.length + ' 段 / 共 ' + Math.round(totalMB) + 'MB' });

  [16 * 1048576, 1 * 1048576, 65536].forEach(function (CHUNK) {
    var ok = 0, fail = 0, okMB = 0, failMB = 0;
    ranges.forEach(function (r) {
      var off = 0;
      while (off < r.size) {
        var len = Math.min(CHUNK, r.size - off);
        try {
          Memory.readByteArray(r.base.add(off), len);
          ok++; okMB += len / 1048576;
        } catch (e) { fail++; failMB += len / 1048576; }
        off += len;
      }
    });
    send({ kind: 'log', text: 'chunk=' + Math.round(CHUNK / 1048576) + 'MB: 成功 ' + ok + ' 块(' + Math.round(okMB) + 'MB) 失败 ' + fail + ' 块(' + Math.round(failMB) + 'MB)' });
  });
  send({ kind: 'done' });
}
setTimeout(testRanges, 200);
"""

def on_message(m, _):
    if m.get("type") == "send":
        print("[agent]", m["payload"].get("text", ""))

dev = frida.get_local_device()
pid = None
for p in dev.enumerate_processes():
    if p.name.lower() == "simcity.exe":
        pid = p.pid
        break
if not pid:
    print("SimCity.exe 不在运行")
    sys.exit(1)
s = dev.attach(pid)
sc = s.create_script(JS)
sc.on("message", on_message)
sc.load()
time.sleep(30)
