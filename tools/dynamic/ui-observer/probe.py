"""Bounded, read-only Windows process probe. No injection, hooks or remote calls."""
import ctypes
from ctypes import wintypes
from pathlib import Path
import time


def inspect(pid, baseline_path, build):
    if not hasattr(ctypes, "WinDLL"):
        raise OSError("windows_only")
    c = ctypes
    k = c.WinDLL("kernel32", use_last_error=True)
    k.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    k.OpenProcess.restype = wintypes.HANDLE
    k.VirtualQueryEx.argtypes = [wintypes.HANDLE, c.c_void_p, c.c_void_p, c.c_size_t]
    k.VirtualQueryEx.restype = c.c_size_t
    k.ReadProcessMemory.argtypes = [wintypes.HANDLE, c.c_void_p, c.c_void_p, c.c_size_t, c.POINTER(c.c_size_t)]
    k.ReadProcessMemory.restype = wintypes.BOOL
    k.CloseHandle.argtypes = [wintypes.HANDLE]

    class MBI(c.Structure):
        _fields_ = [("BaseAddress", c.c_void_p), ("AllocationBase", c.c_void_p),
                    ("AllocationProtect", wintypes.DWORD), ("PartitionId", wintypes.WORD),
                    ("RegionSize", c.c_size_t), ("State", wintypes.DWORD),
                    ("Protect", wintypes.DWORD), ("Type", wintypes.DWORD)]

    # Query information and read memory only. Exclude mapped files to avoid treating
    # a mapped package's bytes as evidence that its script was consumed.
    handle = k.OpenProcess(0x410, False, pid)
    if not handle:
        raise c.WinError(c.get_last_error())
    baseline = Path(baseline_path).read_bytes()
    markers = ["__OpenSCPObserver", build, "scrui.project", "000_openscp_ui_observer.package",
               "zzz_openscp_ui_observer.package"]
    patterns = [(key, encoding, key.encode(encoding)) for key in markers for encoding in ("ascii", "utf-16-le")]
    hits = {key: [] for key in markers}
    address, scanned = 0, 0
    started = time.monotonic()
    baseline_match = False
    source_offset = baseline.find(b"scrui.project")

    def read(at, size):
        buffer = c.create_string_buffer(size)
        received = c.c_size_t()
        if not k.ReadProcessMemory(handle, c.c_void_p(at), buffer, size, c.byref(received)):
            return b""
        return buffer.raw[:received.value]

    try:
        while address < 0x80000000 and time.monotonic() - started < 20:
            region = MBI()
            if not k.VirtualQueryEx(handle, c.c_void_p(address), c.byref(region), c.sizeof(region)):
                break
            base, size = region.BaseAddress or 0, region.RegionSize
            if size == 0:
                break
            address = base + size
            if region.State != 0x1000 or region.Type != 0x20000 or region.Protect & 0x101 or not region.Protect & 0xEE:
                continue
            for at in range(base, base + size, 1048576):
                if time.monotonic() - started >= 20 or scanned >= 1536 * 1048576:
                    break
                data = read(at, min(1048576 + 160, base + size - at))
                scanned += len(data)
                for key, encoding, pattern in patterns:
                    offset = data.find(pattern)
                    if offset >= 0 and len(hits[key]) < 16:
                        hits[key].append({"address": hex(at + offset), "encoding": encoding})
                        if key == "scrui.project" and encoding == "ascii" and source_offset >= 0:
                            start = at + offset - source_offset
                            if start >= base:
                                candidate = read(start, len(baseline) + 100)
                                # The observer retains the baseline as a prefix: require
                                # no appended observer/config text before calling it unpatched.
                                tail = candidate[len(baseline):]
                                if candidate[:len(baseline)] == baseline and b"OpenSCPObserver" not in tail:
                                    baseline_match = True
            if scanned >= 1536 * 1048576:
                break
        return {"pid": pid, "readOnly": True, "scannedBytes": scanned,
                "seconds": round(time.monotonic() - started, 3), "privateMemoryMarkers": hits,
                "unpatchedSourceCopyFound": baseline_match,
                "observerMarkerFound": bool(hits["__OpenSCPObserver"]),
                "note": "Marker presence alone does not prove script execution; absence is bounded-scan evidence only."}
    finally:
        k.CloseHandle(handle)
