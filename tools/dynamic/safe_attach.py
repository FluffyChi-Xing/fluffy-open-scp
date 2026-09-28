#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
安全模式活体 attach 模板（SimCity 破解版专用）。

安全规则（防脚本崩溃传染游戏）：
  1. 钩子数 ≤10，只观察（计数），不在回调里做重活
  2. 禁止对未验证对象调 NativeFunction（GetFunction 实验已致崩溃）
  3. 回调全部 try/catch
  4. 大扫描只在游戏空闲期跑，且只读

用法：
  python tools/dynamic/safe_attach.py                 # attach 已运行实例
  python tools/dynamic/safe_attach.py --wait          # 等进程出现自动挂
  python tools/dynamic/safe_attach.py --pid 12345     # 指定 PID
自定义钩子：改 HOOKS 列表（VA, 标签名），回调逻辑在 on_hit。
"""
import argparse, ctypes, sys, time, json
from pathlib import Path
from collections import Counter

import frida

HERE = Path(__file__).resolve().parent
LIVE = Path(r'D:\rust\packages\fluffy-open-scp\tmp\dynamic\live')
LIVE.mkdir(parents=True, exist_ok=True)

# —— 默认观察点（按需增删，保持 ≤10 个）——
HOOKS = [
    (0x437610, 'draw 函数'),
    (0x4376f0, 'SP::Draw1'),
    (0x437750, 'SP::Draw3'),
    (0x4377c0, 'SP::Draw5'),
    (0x7ebe70, 'DecalDrawBatch 渲染'),
]

JS = r'''
var game = Process.findModuleByName('SimCity.exe');
var HOOKS = %s;
var counts = {};
var firstArgs = {};
HOOKS.forEach(function (h) {
  var va = h[0], tag = h[1];
  counts[tag] = 0;
  try {
    Interceptor.attach(game.base.add(va), {
      onEnter: function (args) {
        try {
          counts[tag]++;
          if (!firstArgs[tag]) {
            firstArgs[tag] = [args[0].toString(), args[1].toString(), args[2].toString(), args[3].toString()];
            send({ kind: 'first', tag: tag, args: firstArgs[tag] });
          }
        } catch (e) {}
      }
    });
  } catch (e) { send({ kind: 'log', text: 'hook 失败 ' + tag + ': ' + e }); }
});
send({ kind: 'log', text: '安全模式：' + HOOKS.length + ' 个观察点已挂' });
setInterval(function () {
  var hot = {};
  for (var k in counts) if (counts[k] > 0) hot[k] = counts[k];
  send({ kind: 'stats', counts: hot, first: firstArgs });
}, 10000);
''' % json.dumps([[hex(va), tag] for va, tag in HOOKS])


def find_pids(name='SimCity.exe'):
    import ctypes
    from ctypes import wintypes
    class PE(ctypes.Structure):
        _fields_ = [("dwSize", wintypes.DWORD), ("cntUsage", wintypes.DWORD),
                    ("th32ProcessID", wintypes.DWORD), ("th32DefaultHeapID", ctypes.c_size_t),
                    ("th32ModuleID", wintypes.DWORD), ("cntThreads", wintypes.DWORD),
                    ("th32ParentProcessID", wintypes.DWORD), ("pcPriClassBase", ctypes.c_long),
                    ("dwFlags", wintypes.DWORD), ("szExeFile", wintypes.WCHAR * 260)]
    k32 = ctypes.WinDLL("kernel32")
    snap = k32.CreateToolhelp32Snapshot(0x2, 0)
    pe = PE(); pe.dwSize = ctypes.sizeof(pe)
    out = []
    if k32.Process32FirstW(snap, ctypes.byref(pe)):
        while True:
            if pe.szExeFile.lower() == name.lower(): out.append(pe.th32ProcessID)
            if not k32.Process32NextW(snap, ctypes.byref(pe)): break
    k32.CloseHandle(snap)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--name', default='SimCity.exe')
    ap.add_argument('--pid', type=int)
    ap.add_argument('--wait', action='store_true')
    args = ap.parse_args()

    d = frida.get_local_device()
    pid = args.pid
    if pid is None:
        pids = find_pids(args.name)
        if not pids and args.wait:
            print(f'等待 {args.name}……', flush=True)
            while not pids:
                time.sleep(2)
                pids = find_pids(args.name)
        if not pids:
            raise SystemExit('未找到进程')
        pid = pids[0]  # 单实例场景

    print(f'attach PID {pid}', flush=True)
    s = d.attach(pid)
    script = s.create_script(JS)
    hits = Counter()

    def on_msg(m, data):
        if m.get('type') == 'send':
            p = m['payload']
            k = p.get('kind')
            if k == 'log':
                print('[agent]', p['text'], flush=True)
            elif k == 'first':
                print(f"[首次] {p['tag']}: args={p['args']}", flush=True)
            elif k == 'stats':
                for tag, n in sorted(p['counts'].items(), key=lambda kv: -kv[1]):
                    hits[tag] = n
                print('[stats]', dict(hits), flush=True)
        elif m.get('type') == 'error':
            print('[script-error]', m.get('description', '')[:150], flush=True)

    script.on('message', on_msg)
    script.load()
    print('Ctrl+C 停止', flush=True)
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        pass
    finally:
        s.detach()


if __name__ == '__main__':
    main()
