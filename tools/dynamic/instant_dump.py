#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
instant_dump.py — 安全模式标准工具：扫到即 dump，即时落盘。

安全规则（防游戏崩溃，实测教训 §65.6）：
  - 禁用 Memory.scanSync（枚举后堆页可能被释放 → 原生级崩溃）
  - 分块 readByteArray（默认 16MB）+ JS 内匹配
  - 发现实例立即读取 + send（主机端即时写盘，扫描中途崩也不丢）
  - 全只读，不写游戏内存，不装钩子（除非 --hook 显式指定）

用法：
  python tools/dynamic/instant_dump.py --vft 0xd9f7bc,0xd9f7a4 --size 0x400
  python tools/dynamic/instant_dump.py --list targets.json   # [{name,va},...]
  python tools/dynamic/instant_dump.py --addr 0x42ba310c --size 0x400  # 直接 dump 已知地址
"""
import argparse, base64, ctypes, json, time, pathlib
from ctypes import wintypes
import frida

LIVE = Path(r'D:\rust\packages\fluffy-open-scp\tmp\dynamic\live')
RTTI = Path(r'D:\rust\packages\fluffy-open-scp\tmp\rtti_vftables.json')

JS = '''
var tlist = __TARGETS__;
var directAddrs = __DIRECT__;
var DUMP_SIZE = __DUMPSIZE__;
var CHUNK = __CHUNK__;
setTimeout(function () {
  var game = Process.findModuleByName('SimCity.exe');
  var glo = parseInt(game.base.toString(16), 16);
  var ghi = parseInt(game.base.add(game.size).toString(16), 16);
  var vtset = {};
  tlist.forEach(function (t) { vtset[t.va] = t.name; });

  // 直传 dump（已知地址，先做——最稳）
  directAddrs.forEach(function (t) {
    try { send({ kind: 'dump', name: t.name, addr: t.addr }, ptr(t.addr).readByteArray(DUMP_SIZE)); }
    catch (e) { send({ kind: 'log', text: t.name + '@' + t.addr + ' err: ' + e }); }
  });

  // 分块扫描（只读），扫到即 dump
  if (tlist.length) {
    var CHUNK2 = CHUNK;
    var ranges = Process.enumerateRanges('rw-');
    var pending = ranges.slice();
    var instances = {}, dumpCount = 0, doneMB = 0, totalMB = 0;
    ranges.forEach(function (r) { totalMB += r.size / 1048576; });
    function processChunk() {
      if (!pending.length) {
        var out = {};
        for (var v in instances) out[vtset[v] + ' @vft 0x' + (+v).toString(16)] = instances[v];
        send({ kind: 'result', map: out });
        send({ kind: 'alldone', dumps: dumpCount });
        return;
      }
      var r = pending[0];
      var off = r._off || 0;
      var len = Math.min(CHUNK2, r.size - off);
      var base = r.base.add(off);
      var buf;
      try { buf = base.readByteArray(len); } catch (e) {
        r._off = off + len;
        if (r._off >= r.size) pending.shift();
        setImmediate(processChunk); return;
      }
      var u32 = new Uint32Array(buf);
      for (var i = 0; i < u32.length; i++) {
        var v = u32[i];
        if (v >= glo && v < ghi && vtset[v] !== undefined) {
          var arr = instances[v] || (instances[v] = []);
          var addrStr = base.add(i * 4).toString();
          if (arr.length < 6 && arr.indexOf(addrStr) < 0) {
            arr.push(addrStr);
            try {
              send({ kind: 'dump', name: vtset[v], addr: addrStr }, ptr(addrStr).readByteArray(DUMP_SIZE));
              dumpCount++;
            } catch (e) {}
          }
        }
      }
      r._off = off + len;
      if (r._off >= r.size) pending.shift();
      doneMB += len / 1048576;
      send({ kind: 'log', text: Math.round(doneMB) + '/' + Math.round(totalMB) + 'MB dumps=' + dumpCount });
      setImmediate(processChunk);
    }
    processChunk();
  } else {
    send({ kind: 'alldone', dumps: 0 });
  }
}, 100);
'''


def find_pid():
    k32 = ctypes.WinDLL("kernel32")

    class PE(ctypes.Structure):
        _fields_ = [("dwSize", wintypes.DWORD), ("cntUsage", wintypes.DWORD),
                    ("th32ProcessID", wintypes.DWORD), ("th32DefaultHeapID", ctypes.c_size_t),
                    ("th32ModuleID", wintypes.DWORD), ("cntThreads", wintypes.DWORD),
                    ("th32ParentProcessID", wintypes.DWORD), ("pcPriClassBase", ctypes.c_long),
                    ("dwFlags", wintypes.DWORD), ("szExeFile", wintypes.WCHAR * 260)]

    snap = k32.CreateToolhelp32Snapshot(0x2, 0)
    pe = PE()
    pe.dwSize = ctypes.sizeof(pe)
    pids = []
    if k32.Process32FirstW(snap, ctypes.byref(pe)):
        while True:
            if pe.szExeFile.lower() == 'simcity.exe':
                pids.append(pe.th32ProcessID)
            if not k32.Process32NextW(snap, ctypes.byref(pe)):
                break
    k32.CloseHandle(snap)
    return pids[0] if pids else None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--vft', help='逗号分隔的 vftable VA 列表（0xd9f7bc,0x...）')
    ap.add_argument('--list', help='目标 JSON 文件 [{name,va},...]')
    ap.add_argument('--addr', action='append', default=[],
                    help='直接 dump 的地址 name=0xVA（可多次）')
    ap.add_argument('--size', default='0x400')
    ap.add_argument('--chunk', default='16')  # MB
    ap.add_argument('--pid', type=int)
    ap.add_argument('--out', default=str(LIVE))
    args = ap.parse_args()

    OUT = Path(args.out)
    OUT.mkdir(parents=True, exist_ok=True)
    tlist = []
    if args.vft:
        rtti = json.load(open(RTTI))
        vt2name = {}
        for n, vs in rtti.items():
            for v in vs:
                vt2name[int(v, 16)] = n.replace('.?A', '').split('@')[0].lstrip('UVV')[:50]
        for v in args.vft.split(','):
            va = int(v, 16)
            tlist.append({'name': vt2name.get(va, f'vft{v}'), 'va': va})
    if args.list:
        tlist.extend(json.load(open(args.list)))
    direct = []
    for a in args.addr:
        name, _, va = a.partition('=')
        direct.append({'name': name or ('addr' + va), 'addr': va})

    pid = args.pid or find_pid()
    if not pid:
        raise SystemExit('SimCity 不在运行')
    print('PID:', pid, flush=True)

    js = (JS.replace('__TARGETS__', json.dumps(tlist))
            .replace('__DIRECT__', json.dumps(direct))
            .replace('__DUMPSIZE__', str(int(args.size, 16) if args.size.startswith('0x') else int(args.size)))
            .replace('__CHUNK__', str(int(args.chunk) << 20)))

    saved = [0]
    done = [False]
    inst = [None]

    def on_msg(m, data):
        if m.get('type') == 'send':
            p = m['payload']
            k = p.get('kind')
            if k == 'dump' and data:
                fn = OUT / (p['name'].replace('/', '_').replace(' ', '_').replace('@', '_')
                            .replace('?', '').replace('$', '')[:60] + '_' + p['addr'][2:] + '.bin')
                fn.write_bytes(data)
                saved[0] += 1
                print(f'[dump #{saved[0]}] {p["name"][:50]} @ {p["addr"]} → {fn.name}', flush=True)
            elif k == 'log':
                print('[agent]', p['text'], flush=True)
            elif k == 'result':
                inst[0] = p['map']
                fn = OUT / 'instances_latest.json'
                json.dump(p['map'], open(fn, 'w'), indent=1, ensure_ascii=False)
                tot = sum(len(v) for v in p['map'].values())
                print(f'[实例表] 类 {len(p["map"])} 实例 {tot} → {fn.name}', flush=True)
            elif k == 'alldone':
                done[0] = True
                print(f'[完成] 总 dump {saved[0]}', flush=True)
        elif m.get('type') == 'error':
            print('[脚本错误]', m.get('description', '')[:150], flush=True)

    s = d.attach(pid)
    script = s.create_script(js)
    script.on('message', on_msg)
    script.load()
    print('已挂载（扫到即 dump 模式）…', flush=True)
    for _ in range(300):
        time.sleep(2)
        if done[0]:
            break
    s.detach()
    print('结束。落盘:', saved[0], flush=True)


if __name__ == '__main__':
    main()
