#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
渲染上下文捕获——主机端（frida 17.x）

配套 agent：tools/dynamic/capture_render_context.js
产物：
  tmp/dynamic/context/shader_*.bin + shader_manifest.jsonl
      编译后最终 shader 字节码（引擎片段链组合的产物，组合表的终局替代）
  tmp/dynamic/context/render_context.jsonl
      渲染上下文事件流：['rs',state,value,ps,vs] 渲染态变更（影子表去重）
                       ['cf',slot,start,count,ps,vs] + 16×count 字节常量
                       ['draw',ps,vs,decalActive] 绘制标记
                       ['mark','decalOn'|'decalOff'] decal pass 边界
      —— ps/vs 为对象指针串，与 shader_manifest 的 obj 字段 join 即得
         「每个 shader 族的状态/常量剖面」；decalActive=1 的行即 decal 族。

用法：
  # attach（游戏进城后）：
  python tools/dynamic/capture_states.py
  # 等待模式（随行）：
  python tools/dynamic/capture_states.py --wait
  # spawn（抓加载期编译）：
  python tools/dynamic/capture_states.py --spawn "<SimCity.exe 路径>"

  启动后回车开一个捕获窗口（默认 20s）；输入数字回车 = 自定义时长。
  捕获窗口内把镜头对准招牌街/破楼，缓慢转动视角即可。

防崩纪律（README）：单会话锁；被动计数器全槽位（v7 惯例）、有源 hook
仅 RS×1/常量×2/绘制×1/帧×1/bind 身份跟踪 + 静态 decal 分发 1 处；
事件 in-window 发送 + 影子表去重。
"""
import argparse
import hashlib
import json
import sys
import threading
import time
from pathlib import Path

import frida

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
OUT = ROOT / "tmp" / "dynamic" / "context"
MANIFEST = OUT / "shader_manifest.jsonl"
CTX = OUT / "render_context.jsonl"
LOCK_PATH = ROOT / "tmp" / "dynamic" / "capture.lock"


def log(*a):
    print(*a, flush=True)


def acquire_capture_lock():
    import ctypes
    import os
    if LOCK_PATH.exists():
        old = 0
        try:
            old = int(LOCK_PATH.read_text(encoding="utf-8").strip())
        except (ValueError, OSError):
            old = 0
        if old and old != os.getpid():
            alive = False
            try:
                h = ctypes.windll.kernel32.OpenProcess(0x1000, False, old)
                if h:
                    alive = True
                    ctypes.windll.kernel32.CloseHandle(h)
            except Exception:
                alive = False
            if alive:
                raise SystemExit(
                    f"已有捕获会话在运行（PID {old}）——单会话铁律：先停止它再启动。")
    LOCK_PATH.parent.mkdir(parents=True, exist_ok=True)
    LOCK_PATH.write_text(str(os.getpid()), encoding="utf-8")


def find_pids_by_name(name: str):
    import ctypes
    from ctypes import wintypes

    class PROCESSENTRY32W(ctypes.Structure):
        _fields_ = [("dwSize", wintypes.DWORD), ("cntUsage", wintypes.DWORD),
                    ("th32ProcessID", wintypes.DWORD),
                    ("th32DefaultHeapID", ctypes.c_size_t),
                    # DWORD！写成 c_size_t 会使 szExeFile 错位 +4B，进程名
                    # 整体被截掉前 2 字符（SimCity.exe → mCity.exe，永不匹配）
                    ("th32ModuleID", wintypes.DWORD), ("cntThreads", wintypes.DWORD),
                    ("th32ParentProcessID", wintypes.DWORD),
                    ("pcPriClassBase", ctypes.c_long), ("dwFlags", wintypes.DWORD),
                    ("szExeFile", wintypes.WCHAR * 260)]

    k32 = ctypes.WinDLL("kernel32")
    snap = k32.CreateToolhelp32Snapshot(0x2, 0)
    pe = PROCESSENTRY32W()
    pe.dwSize = ctypes.sizeof(pe)
    pids = []
    if k32.Process32FirstW(snap, ctypes.byref(pe)):
        while True:
            if pe.szExeFile.lower() == name.lower():
                pids.append(pe.th32ProcessID)
            if not k32.Process32NextW(snap, ctypes.byref(pe)):
                break
    k32.CloseHandle(snap)
    return pids


def working_set(pid: int) -> int:
    import ctypes
    from ctypes import wintypes

    class PMC(ctypes.Structure):
        _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD),
                    ("WorkingSetSize", ctypes.c_size_t),
                    ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                    ("QuotaPagedPoolUsage", ctypes.c_size_t),
                    ("PagefileUsage", ctypes.c_size_t)]

    k32 = ctypes.WinDLL("kernel32")
    psapi = ctypes.WinDLL("psapi")
    h = k32.OpenProcess(0x1000, False, pid)
    if not h:
        return 0
    try:
        pmc = PMC()
        pmc.cb = ctypes.sizeof(pmc)
        if psapi.GetProcessMemoryInfo(h, ctypes.byref(pmc), pmc.cb):
            return pmc.WorkingSetSize
        return 0
    finally:
        k32.CloseHandle(h)


class Store:
    def __init__(self):
        self.seen = set()
        self.n_shader = 0
        self.n_ctx = 0
        self.last_script = None
        OUT.mkdir(parents=True, exist_ok=True)

    def on_shader(self, p: dict, data):
        if not data:
            return
        sha1 = hashlib.sha1(data).hexdigest()[:12]
        ver = int(p.get("ver", "0"), 16)
        model = f"{p['type']}{(ver >> 8) & 0xFF}{ver & 0xFF}"
        stem = f"{model}_{sha1}"
        if stem in self.seen:
            return
        self.seen.add(stem)
        self.n_shader += 1
        bin_path = OUT / f"{stem}.bin"
        bin_path.write_bytes(data)
        rec = dict(p, sha1=sha1, file=bin_path.name, t=time.strftime("%H:%M:%S"))
        with MANIFEST.open("a", encoding="utf-8") as f:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")
        log(f"[shader] {model} size={len(data)} obj={p.get('obj')} -> {bin_path.name}")

    def on_ctx(self, p: dict, data):
        self.n_ctx += 1
        rec = dict(p)
        if data:
            rec["hex"] = data.hex()
        with CTX.open("a", encoding="utf-8") as f:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")
        e = p.get("e", [])
        if e and e[0] in ("mark", "draw"):
            log(f"[ctx f{p.get('f')}] {e}")


def make_on_message(store: Store, script_holder):
    def on_message(msg, data):
        if msg.get("type") == "send":
            p = msg["payload"]
            kind = p.get("kind")
            if kind == "shader":
                store.on_shader(p, data)
            elif kind == "ctx":
                store.on_ctx(p, data)
            elif kind == "log":
                log("[agent]", p.get("text", ""))
            elif kind == "stats":
                s = p["stats"]
                log(f"[stats] shader={s.get('shaders', 0)} ctx事件={s.get('ctx', 0)} "
                    f"draw={s.get('draws', 0)} frame={p.get('frame')} "
                    f"decalPass={'是' if p.get('decal') else '否'}")
        elif msg.get("type") == "error":
            log("[script-error]", msg.get("stack") or msg.get("description"))
        else:
            log(msg)
    return on_message


def input_loop(script, stop_event):
    """回车 = 开一个捕获窗口；数字回车 = 自定义时长；q 回车 = 退出。
    无 TTY（后台任务）时 stdin 立即 EOF——只禁用交互，不停止捕获。"""
    try:
        while True:
            line = input()
            line = line.strip()
            if line.lower() == "q":
                stop_event.set()
                return
            secs = int(line) if line.isdigit() else 20
            script.post({"type": "control", "cmd": "start", "seconds": secs})
    except EOFError:
        log("无交互终端（后台模式）——捕获由 --loop/--auto 驱动")
    except Exception:
        pass


def main():
    acquire_capture_lock()
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--name", default="SimCity.exe")
    ap.add_argument("--pid", type=int)
    ap.add_argument("--wait", action="store_true",
                    help="进程不在时等待其出现（随行抓设备创建）")
    ap.add_argument("--spawn", metavar="EXE", help="从进程创建抓")
    ap.add_argument("--auto", type=int, default=0, metavar="SEC",
                    help="attach 后自动开一个 SEC 秒的捕获窗口（默认等回车）")
    ap.add_argument("--loop", nargs=2, type=int, metavar=("SEC", "EVERY"),
                    help="循环捕获：每 EVERY 秒开一个 SEC 秒窗口（后台免交互）")
    ap.add_argument("--device", metavar="VT_HEX",
                    help="已知设备 vftable 直连（跳过发现，重挂同一活进程用）")
    args = ap.parse_args()

    device = frida.get_local_device()
    pids = []
    if args.pid:
        pids = [args.pid]
    else:
        for pr in device.enumerate_processes():
            if pr.name.lower() == args.name.lower():
                pids.append(pr.pid)
        for extra in find_pids_by_name(args.name):
            if extra not in pids:
                pids.append(extra)

    store = Store()
    agent_src = (HERE / "capture_render_context.js").read_text(encoding="utf-8")

    if args.spawn:
        exe = args.spawn
        pid = device.spawn([exe], cwd=str(Path(exe).parent))
        session = device.attach(pid)
        script = session.create_script(agent_src)
        script.on("message", make_on_message(store, {"script": script}))
        script.load()
        script.post({"type": "config", "spawn": True, "deviceVt": args.device})
        device.resume(pid)
    elif args.wait and not pids:
        # 随行观察（推荐主路径）：先挂等待，游戏一出现（工作集≥5MB，加载
        # 早期）即注入——lurk 从 Direct3DCreate9 跟随设备创建，完全绕开
        # 堆扫描路径（2026-10-01 崩溃教训：attach 堆扫描垃圾候选致崩）。
        log(f"等待 {args.name} 出现……现在启动游戏即可，逐进程自动挂载")
        attached = set()
        sessions = []

        def attach_one(cand):
            ws = working_set(cand)
            try:
                sess = device.attach(cand)
                sc = sess.create_script(agent_src)
                sc.on("message", make_on_message(store, {"script": sc}))
                sc.load()
                sc.post({"type": "config", "spawn": False, "deviceVt": args.device})
                attached.add(cand)
                sessions.append(sess)
                store.last_script = sc
                log(f"已 attach PID {cand}（工作集 {ws // (1024*1024)}MB）")
                return sess
            except Exception as e:
                log(f"attach PID {cand} 失败：{e}")
                return None

        while True:
            for cand in find_pids_by_name(args.name):
                if cand in attached:
                    continue
                ws = working_set(cand)
                # ws==0 = OpenProcess 查询失败（实测本机对 SimCity 即如此，
                # attach 仍可成功）——不能当挂载门槛
                if ws != 0 and ws < 5 * 1024 * 1024:
                    continue
                attach_one(cand)
            if attached:
                break
            time.sleep(1)
        # 主进程挂上后，继续监视后续子进程（引导器→真身）
        def watch_more():
            while True:
                time.sleep(1)
                for cand in find_pids_by_name(args.name):
                    if cand in attached:
                        continue
                    ws = working_set(cand)
                    if ws != 0 and ws < 5 * 1024 * 1024:
                        continue
                    attach_one(cand)
        threading.Thread(target=watch_more, daemon=True).start()
        # --wait 分支的 script 在 attach_one 内创建——取最新会话供主循环用
        script = store.last_script
    else:
        if not pids:
            raise SystemExit("未找到目标进程——用 --wait 随行或先启动游戏")
        pids.sort(key=working_set, reverse=True)
        session = None
        for cand in pids:
            try:
                log(f"尝试 attach PID {cand}（工作集 {working_set(cand) // (1024*1024)}MB）...")
                session = device.attach(cand)
                break
            except Exception as e:
                log(f"attach PID {cand} 失败：{e}")
        if session is None:
            raise SystemExit("全部候选 PID attach 失败")
        script = session.create_script(agent_src)
        script.on("message", make_on_message(store, {"script": script}))
        script.load()
        script.post({"type": "config", "spawn": False, "deviceVt": args.device})
    log("已挂载。回车=开 20s 捕获窗口；数字回车=自定义时长；q 回车=退出。")

    if args.auto:
        script.post({"type": "control", "cmd": "start", "seconds": args.auto})
    stop_event = threading.Event()
    if args.loop:
        secs, every = args.loop
        def loop_windows():
            time.sleep(8)  # 等设备定位/钩子武装
            while not stop_event.is_set():
                script.post({"type": "control", "cmd": "start", "seconds": secs})
                stop_event.wait(max(every, secs + 5))
        threading.Thread(target=loop_windows, daemon=True).start()
        log(f"循环捕获：每 {every}s 开 {secs}s 窗口")

    t = threading.Thread(target=input_loop, args=(script, stop_event), daemon=True)
    t.start()
    try:
        # 主循环与交互线程解耦：EOF/输入异常不影响捕获（后台模式教训）
        while not stop_event.is_set():
            time.sleep(0.5)
    except KeyboardInterrupt:
        pass
    finally:
        try:
            session.detach()
        except Exception:
            pass
        log(f"结束：shader {store.n_shader} 个、ctx 事件 {store.n_ctx} 条 → {OUT}")


if __name__ == "__main__":
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass
    main()
