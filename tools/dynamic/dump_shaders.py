#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
SimCity (2013) D3D9 shader 字节码抓取——主机端（frida 17.x）

用法：
  attach（游戏进城市视图后）： python tmp/dynamic/dump_shaders.py
  spawn（从头抓加载期编译）：   python tmp/dynamic/dump_shaders.py --spawn "D:\\ea-games\\simcity_offline\\SimCity：Cites of Tomorrow\\SimCity\\SimCity.exe"

产物：
  tmp/dynamic/shaders/*.bin    原始 SM 字节码（首 DWORD=总长）
  tmp/dynamic/shaders/*.asm    d3dcompiler_47!D3DDisassemble 反汇编（--no-disasm 关闭）
  tmp/dynamic/shader_manifest.jsonl  每个落盘 shader 一条记录（type/slot/source/obj/ver/size/sha1）
"""
import argparse
import ctypes
import hashlib
import json
import sys
import time
from pathlib import Path

import frida

HERE = Path(__file__).resolve().parent
# 产物统一落 tmp/dynamic（tools/ 保持干净）
OUT = HERE.parent.parent / "tmp" / "dynamic" / "shaders"
MANIFEST = HERE.parent.parent / "tmp" / "dynamic" / "shader_manifest.jsonl"


def log(*a):
    print(*a, flush=True)


def disasm(data: bytes):
    """d3dcompiler_47.dll（系统自带）反汇编 SM 字节码 → 文本"""
    try:
        d3dc = ctypes.WinDLL("d3dcompiler_47.dll")
    except OSError:
        return None
    fn = d3dc.D3DDisassemble
    # restype 不能用 ctypes.HRESULT：失败时 ctypes 会直接抛 OSError，拿不到返回值
    fn.restype = ctypes.c_long
    fn.argtypes = [ctypes.c_char_p, ctypes.c_size_t, ctypes.c_uint,
                   ctypes.c_char_p, ctypes.POINTER(ctypes.c_void_p)]
    blob = ctypes.c_void_p()
    hr = fn(data, len(data), 0, None, ctypes.byref(blob))
    if hr != 0 or not blob.value:
        return None
    # ID3DBlob vtable：0=QI 1=AddRef 2=Release 3=GetBufferPointer 4=GetBufferSize
    slots = ctypes.cast(ctypes.cast(blob, ctypes.POINTER(ctypes.c_void_p)).contents.value,
                        ctypes.POINTER(ctypes.c_void_p))
    get_ptr = ctypes.WINFUNCTYPE(ctypes.c_void_p, ctypes.c_void_p)(slots[3])
    get_size = ctypes.WINFUNCTYPE(ctypes.c_size_t, ctypes.c_void_p)(slots[4])
    release = ctypes.WINFUNCTYPE(ctypes.c_ulong, ctypes.c_void_p)(slots[2])
    try:
        text = ctypes.string_at(get_ptr(blob), get_size(blob))
    finally:
        release(blob)
    return text.decode("ascii", "replace")


class Saver:
    def __init__(self, do_disasm: bool):
        self.do_disasm = do_disasm
        self.seen = set()
        self.n_new = 0
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
        self.n_new += 1
        bin_path = OUT / f"{stem}.bin"
        bin_path.write_bytes(data)
        rec = dict(p, sha1=sha1, file=bin_path.name, t=time.strftime("%H:%M:%S"))
        with MANIFEST.open("a", encoding="utf-8") as f:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")
        note = ""
        if self.do_disasm:
            try:
                text = disasm(data)
            except Exception as e:
                text = None
                note = f" (disasm 异常: {e})"
            if text is not None:
                (OUT / f"{stem}.asm").write_text(text, encoding="ascii", errors="replace")
                note = " +asm"
        log(f"[{rec['source']}@slot{rec.get('slot')}] {model} size={len(data)} -> "
            f"{bin_path.name}{note}  (新 {self.n_new})")


def find_pids_by_name(name: str):
    """frida 17 的 enumerate_processes 会漏掉部分进程（实测漏 SimCity.exe），
    用 Windows 原生 Toolhelp 快照兜底，返回全部候选 PID"""
    import ctypes
    from ctypes import wintypes

    class PROCESSENTRY32W(ctypes.Structure):
        _fields_ = [("dwSize", wintypes.DWORD), ("cntUsage", wintypes.DWORD),
                    ("th32ProcessID", wintypes.DWORD),
                    ("th32DefaultHeapID", ctypes.c_size_t),
                    ("th32ModuleID", wintypes.DWORD), ("cntThreads", wintypes.DWORD),
                    ("th32ParentProcessID", wintypes.DWORD),
                    ("pcPriClassBase", ctypes.c_long), ("dwFlags", wintypes.DWORD),
                    ("szExeFile", wintypes.WCHAR * 260)]

    k32 = ctypes.WinDLL("kernel32")
    snap = k32.CreateToolhelp32Snapshot(0x2, 0)  # TH32CS_SNAPPROCESS
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
    """进程工作集字节数（用于区分真身与卡死僵尸实例）"""
    import ctypes
    from ctypes import wintypes

    class PMC(ctypes.Structure):
        _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD),
                    ("PeakWorkingSetSize", ctypes.c_size_t),
                    ("WorkingSetSize", ctypes.c_size_t),
                    ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                    ("QuotaPagedPoolUsage", ctypes.c_size_t),
                    ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
                    ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                    ("PagefileUsage", ctypes.c_size_t),
                    ("PeakPagefileUsage", ctypes.c_size_t)]

    k32 = ctypes.WinDLL("kernel32")
    psapi = ctypes.WinDLL("psapi")
    h = k32.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
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


def main():
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--name", default="SimCity.exe", help="attach 目标进程名")
    ap.add_argument("--pid", type=int, help="直接按 PID attach（优先于 --name）")
    ap.add_argument("--wait", action="store_true",
                    help="进程不在时等待其出现（重启游戏前先挂上，可随行抓设备创建）")
    ap.add_argument("--ws-mb", type=int, default=50,
                    help="--wait 模式下 attach 的工作集阈值 MB（800≈菜单渲染期）")
    ap.add_argument("--spawn", metavar="EXE", help="从进程创建抓（可抓加载期 CreateShader）")
    ap.add_argument("--no-disasm", action="store_true", help="不生成 .asm 反汇编")
    args = ap.parse_args()

    saver = Saver(not args.no_disasm)

    def on_message(msg, data):
        if msg.get("type") == "send":
            p = msg["payload"]
            kind = p.get("kind")
            if kind == "shader":
                saver.on_shader(p, data)
            elif kind == "log":
                if p.get("text", "").startswith("FHEX:") and data:
                    log(f"[FHEX]{p['text'][5:]}", data.hex(" ", 8))
                else:
                    log("[agent]", p["text"])
            elif kind == "stats":
                s = p["stats"]
                log(f"[stats] create={s.get('create', 0)} bind={s.get('bind', 0)} "
                    f"新文件={saver.n_new}")
            elif kind == "calibrate":
                log(f"[校准] 真实 create 槽位 slot={p['slot']} type={p['type']}"
                    f"（与硬编码不符！后续分析以实测槽位为准）")
        elif msg.get("type") == "error":
            log("[script-error]", msg.get("stack") or msg.get("description"))
        else:
            log(msg)

    # frida 17 移除了模块级便捷 API，统一走 LocalDevice
    device = frida.get_local_device()
    if args.spawn:
        exe = args.spawn
        pid = device.spawn([exe], cwd=str(Path(exe).parent))
        session = device.attach(pid)
    else:
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
        if args.wait and not pids:
            # 持续 watcher：每个新出现的进程（含引导父进程+真身子进程）都在
            # 出生瞬间挂载——随行观察需要早于子进程的设备创建
            log(f"等待 {args.name} 出现……（现在启动游戏即可，逐进程自动挂载）")
            attached = set()
            agent_src = (HERE / "hook_d3d9.js").read_text(encoding="utf-8")
            while True:
                time.sleep(1)
                for cand in find_pids_by_name(args.name):
                    if cand in attached:
                        continue
                    ws = working_set(cand)
                    if ws < 5 * 1024 * 1024:  # 跳过刚创建/僵尸
                        continue
                    attached.add(cand)
                    try:
                        sess = device.attach(cand)
                        script = sess.create_script(agent_src)
                        script.on("message", on_message)
                        script.load()
                        script.post({"type": "config", "spawn": bool(args.spawn)})
                        log(f"已 attach PID {cand}（工作集 {ws // (1024*1024)}MB）")
                    except Exception as e:
                        log(f"attach PID {cand} 失败：{e}")
                if attached:
                    break
            session = next(iter([sess]))
            pid = pids[0] if pids else None
            # 多会话模式：后续捕获都在 watcher 回调里，主循环只等待
            log("watcher 就绪，Ctrl+C 退出")
            try:
                while True:
                    time.sleep(1)
                    for cand in find_pids_by_name(args.name):
                        if cand in attached:
                            continue
                        ws = working_set(cand)
                        if ws < 5 * 1024 * 1024:
                            continue
                        attached.add(cand)
                        try:
                            sess2 = device.attach(cand)
                            sc2 = sess2.create_script(agent_src)
                            sc2.on("message", on_message)
                            sc2.load()
                            sc2.post({"type": "config", "spawn": bool(args.spawn)})
                            log(f"追加 attach PID {cand}（工作集 {ws // (1024*1024)}MB）")
                        except Exception as e:
                            log(f"attach PID {cand} 失败：{e}")
            except KeyboardInterrupt:
                raise SystemExit(0)
        # 多实例时优先真身（工作集最大），卡死僵尸排后面
        pids.sort(key=working_set, reverse=True)
        session = None
        for cand in pids:
            try:
                log(f"尝试 attach PID {cand}（工作集 {working_set(cand) // (1024 * 1024)}MB）...")
                session = device.attach(cand)
                pid = cand
                break
            except frida.ProcessNotRespondingError:
                log(f"attach PID {cand} 失败：进程未响应注入，换下一个候选")
            except Exception as e:
                log(f"attach PID {cand} 失败：{e}")
        if session is None:
            raise SystemExit("全部候选 PID attach 失败——游戏可能卡死，重启游戏后重试")

    script = session.create_script((HERE / "hook_d3d9.js").read_text(encoding="utf-8"))
    script.on("message", on_message)
    script.load()
    script.post({"type": "config", "spawn": bool(args.spawn)})
    if args.spawn:
        device.resume(pid)
    log("已挂载。Ctrl+C 停止。attach 模式请在城里走动触发 shader 绑定（招牌街/水边/破楼）。")
    try:
        while True:
            time.sleep(0.5)
    except KeyboardInterrupt:
        pass
    finally:
        session.detach()
        log(f"结束：共落盘 {saver.n_new} 个新 shader -> {OUT}")


if __name__ == "__main__":
    try:
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    except Exception:
        pass
    main()
