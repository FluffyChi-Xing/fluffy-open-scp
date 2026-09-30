#!/usr/bin/env python3
"""external_device_scan.py — 外部内存走行找 d3d9 接口对象（不注入、零游戏干扰）。

用 ReadProcessMemory/VirtualQueryEx 从进程外读堆：对每个 4 对齐位置，若其值
指向 d3d9 模块内 → 读该 vftable 的 130 槽做密度校验（≥110 可读且 ≥100 指向
d3d9 exec）→ 计为设备/接口候选。按 vftable 聚合输出实例数与地址样本。
"""
import ctypes, sys
from ctypes import wintypes

PID = int(sys.argv[1]) if len(sys.argv) > 1 else None
MOD_LO, MOD_HI = 0x6C2B0000, 0x6C42A000   # d3d9.dll（今日各会话实测基址一致）
EXEC_LO, EXEC_HI = 0x6C2B1000, 0x6C40D000

k32 = ctypes.WinDLL("kernel32", use_last_error=True)
psapi = ctypes.WinDLL("psapi", use_last_error=True)

PROCESS_VM_READ = 0x0010
PROCESS_QUERY_INFORMATION = 0x0400
MEM_COMMIT = 0x1000
READABLE = {0x02, 0x04, 0x06, 0x20, 0x40, 0x80}  # PAGE_READONLY/READWRITE/EXECUTE_READ/READWRITE/EXECUTE_WRITEWATCH equivalence 简化


class MBI(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_void_p),
                ("AllocationBase", ctypes.c_void_p),
                ("AllocationProtect", wintypes.DWORD),
                ("RegionSize", ctypes.c_size_t),
                ("State", wintypes.DWORD),
                ("Protect", wintypes.DWORD),
                ("Type", wintypes.DWORD)]


def regions(h):
    addr = 0
    out = []
    mbi = MBI()
    size = ctypes.sizeof(mbi)
    while addr < 0x7FFF0000:
        if not k32.VirtualQueryEx(h, ctypes.c_void_p(addr), ctypes.byref(mbi), size):
            addr += 0x1000
            continue
        if mbi.State == MEM_COMMIT and mbi.Protect in READABLE and mbi.Type != 0x20000:
            out.append((mbi.BaseAddress, mbi.RegionSize))
        addr = (mbi.BaseAddress or addr) + mbi.RegionSize
    return out


def main():
    h = k32.OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_INFORMATION, False, PID)
    if not h:
        print(f"OpenProcess({PID}) 失败")
        sys.exit(1)
    regs = regions(h)
    total = sum(sz for _, sz in regs) / 1048576
    print(f"可读区域 {len(regs)} 段 / {total:.0f}MB")

    read = k32.ReadProcessMemory
    read.argtypes = [wintypes.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]

    from collections import defaultdict
    vft_hits = defaultdict(list)
    candidates = 0
    CHUNK = 1 << 20
    for base, size in regs:
        off = 0
        while off < size:
            n = min(CHUNK, size - off)
            buf = (ctypes.c_char * n)()
            got = ctypes.c_size_t(0)
            if not read(h, ctypes.c_void_p(base + off), buf, n, ctypes.byref(got)) or got.value == 0:
                break
            n = got.value
            b = buf
            for i in range(0, n - 4, 4):
                v = int.from_bytes(b[i:i + 4], "little")
                # 排除模块映像内部的伪指针：只统计堆/非模块区位置
                if MOD_LO <= (base + off + i) < MOD_HI:
                    continue
                if MOD_LO <= v < MOD_HI:
                    candidates += 1
                    # 读 vftable 密度（130 槽）
                    vtbuf = (ctypes.c_char * (130 * 4))()
                    if read(h, ctypes.c_void_p(v), vtbuf, 130 * 4, ctypes.byref(got)) and got.value == 130 * 4:
                        execn = 0
                        for s in range(130):
                            fp = int.from_bytes(bytes(vtbuf[s * 4:s * 4 + 4]), "little")
                            if fp and EXEC_LO <= fp < EXEC_HI:
                                execn += 1
                        if execn >= 100:
                            obj = base + off + i
                            vft_hits[v].append(hex(obj))
            off += n

    print(f"\nd3d9 模块指针候选 {candidates} 个；密度达标的 vftable：")
    for v, objs in sorted(vft_hits.items(), key=lambda kv: -len(kv[1])):
        print(f"  vftable 0x{v:08x}: {len(objs)} 实例  样本 {objs[:4]}")
    k32.CloseHandle(h)


if __name__ == "__main__":
    main()
