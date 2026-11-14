"""Read-only x86 EAWebKit object survey; no injection, hooks or remote calls."""
import argparse
import ctypes as c
from ctypes import wintypes as w
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import struct
import sys
import time

from eawebkit_audit import audit

EXE_HASH = "2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab"
TEXT = {
    "zh-CN": {
        "description": "只读检查运行中的零售 EAWebKit 宿主和网页对象。",
        "pid": "SimCity 进程 ID", "output": "JSON 报告路径", "views": "有界扫描网页对象候选，不调用引擎",
        "help": "显示帮助并退出", "lang": "输出语言", "options": "选项", "arguments": "位置参数",
        "error": "读取失败", "unsupported": "进程或 DLL 指纹不匹配", "invalid": "运行时结构未通过校验",
        "windows": "需要 Windows 和 64 位 Python", "missing": "尚未加载 SimCity 或 EAWebKit 模块",
    },
    "en-US": {
        "description": "Read-only survey of retail EAWebKit host and page objects.",
        "pid": "SimCity process ID", "output": "JSON report path", "views": "Bounded scan for page object candidates; no engine calls",
        "help": "Show help and exit", "lang": "Output language", "options": "Options", "arguments": "Positional arguments",
        "error": "Read failed", "unsupported": "Unsupported process or DLL fingerprint", "invalid": "Runtime structure validation failed",
        "windows": "Windows and 64-bit Python required", "missing": "SimCity or EAWebKit module not loaded yet",
    },
}


class Module(c.Structure):
    _fields_ = [("dwSize", w.DWORD), ("th32ModuleID", w.DWORD), ("th32ProcessID", w.DWORD),
                ("GlblcntUsage", w.DWORD), ("ProccntUsage", w.DWORD), ("modBaseAddr", c.c_void_p),
                ("modBaseSize", w.DWORD), ("hModule", w.HMODULE),
                ("szModule", w.WCHAR * 256), ("szExePath", w.WCHAR * 260)]


class MemoryRegion(c.Structure):
    _fields_ = [("BaseAddress", c.c_void_p), ("AllocationBase", c.c_void_p),
                ("AllocationProtect", w.DWORD), ("PartitionId", w.WORD),
                ("RegionSize", c.c_size_t), ("State", w.DWORD), ("Protect", w.DWORD), ("Type", w.DWORD)]


class Reader:
    def __init__(self, pid):
        if not hasattr(c, "WinDLL") or c.sizeof(c.c_void_p) != 8:
            raise ValueError("windows")
        self.k = c.WinDLL("kernel32", use_last_error=True)
        signatures = {
            "OpenProcess": ([w.DWORD, w.BOOL, w.DWORD], w.HANDLE),
            "CloseHandle": ([w.HANDLE], w.BOOL),
            "CreateToolhelp32Snapshot": ([w.DWORD, w.DWORD], w.HANDLE),
            "Module32FirstW": ([w.HANDLE, c.POINTER(Module)], w.BOOL),
            "Module32NextW": ([w.HANDLE, c.POINTER(Module)], w.BOOL),
            "ReadProcessMemory": ([w.HANDLE, c.c_void_p, c.c_void_p, c.c_size_t, c.POINTER(c.c_size_t)], w.BOOL),
            "VirtualQueryEx": ([w.HANDLE, c.c_void_p, c.POINTER(MemoryRegion), c.c_size_t], c.c_size_t),
        }
        for name, (args, result) in signatures.items():
            function = getattr(self.k, name)
            function.argtypes, function.restype = args, result
        self.handle = self.k.OpenProcess(0x410, False, pid)
        if not self.handle:
            raise c.WinError(c.get_last_error())
        self.modules = []
        try:
            snapshot = self.k.CreateToolhelp32Snapshot(0x18, pid)
            if snapshot == c.c_void_p(-1).value:
                raise c.WinError(c.get_last_error())
            try:
                item = Module()
                item.dwSize = c.sizeof(item)
                found = self.k.Module32FirstW(snapshot, c.byref(item))
                while found:
                    self.modules.append({"name": item.szModule, "path": item.szExePath,
                                         "base": item.modBaseAddr, "size": item.modBaseSize})
                    found = self.k.Module32NextW(snapshot, c.byref(item))
            finally:
                self.k.CloseHandle(snapshot)
        except Exception:
            self.close()
            raise

    def close(self):
        if self.handle:
            self.k.CloseHandle(self.handle)
            self.handle = None

    def read(self, address, size):
        if address < 0x10000 or size < 0 or size > 1048576 + 8 or address + size > 0x100000000:
            raise ValueError("invalid")
        buffer = c.create_string_buffer(size)
        received = c.c_size_t()
        if not self.k.ReadProcessMemory(self.handle, address, buffer, size, c.byref(received)) or received.value != size:
            raise OSError("unreadable_memory")
        return buffer.raw

    def u32(self, address):
        return struct.unpack("<I", self.read(address, 4))[0]

    def locate(self, address):
        for module in self.modules:
            if module["base"] <= address < module["base"] + module["size"]:
                return {"module": module["name"], "rva": hex(address - module["base"])}
        return None


def validate_view(reader, address, vtable):
    """Reject incidental vtable words using the private object's reverse pointer."""
    if address % 4 or reader.u32(address) != vtable:
        return None
    private = reader.u32(address + 4)
    if reader.u32(private) != address:
        return None
    page = reader.u32(private + 4)
    if not page:
        return None
    reader.read(page, 8)
    user_data = reader.u32(private + 0x234)
    # mURL is only a cache populated by GetURI; reading it cannot prove current URL.
    start, end = struct.unpack("<II", reader.read(private + 8, 8))
    cached = None
    if start and start <= end and (end - start) % 2 == 0 and end - start <= 2048:
        cached = reader.read(start, end - start).decode("utf-16-le", errors="replace") if end > start else ""
    result = {"view": hex(address), "private": hex(private), "page": hex(page),
              "userData": hex(user_data), "cachedURI": cached, "currentURIVerified": False}
    # Matched retail GetURI -> mainFrame -> WebFrame::url accessors, without calls.
    page_private = reader.u32(page)
    frame = reader.u32(page_private + 12)
    frame_private = reader.u32(frame)
    string = reader.u32(frame_private + 0x1C)
    if string:
        length, chars = struct.unpack("<II", reader.read(string + 4, 8))
        if length > 4096:
            raise ValueError("invalid")
        uri = reader.read(chars, length * 2).decode("utf-16-le", errors="replace") if length else ""
        if reader.u32(frame_private + 0x1C) != string or reader.u32(page_private + 12) != frame:
            raise ValueError("invalid")
        result.update(currentFrameURI=uri, currentURIVerified=True, frame=hex(frame))
    if user_data and reader.u32(user_data + 0xB0) == address:
        result["hostViewBackReferenceVerified"] = True
        result["inspectorObject"] = hex(reader.u32(user_data + 0x148))
        bound = reader.u32(user_data + 0x158)
        if bound and reader.u32(bound + 4) == user_data:
            bound_vtable = reader.u32(bound)
            invoke = reader.u32(bound_vtable + 8)
            result["clientBinding"] = {"object": hex(bound), "vtable": hex(bound_vtable),
                                       "invokeMethod": hex(invoke), "location": reader.locate(invoke)}
            result["registeredCallbacks"] = registered_callbacks(reader, user_data)
    if reader.u32(address + 4) != private or reader.u32(private) != address:
        raise ValueError("invalid")
    return result


def registered_callbacks(reader, host):
    # cEAWebKitWebView::InvokeJavaScriptCallback + find_as disassembly confirms
    # map +0x1c, bucket pointer +4/count +8, callback node +0x10/next +0x14.
    buckets, count, item_count = struct.unpack("<III", reader.read(host + 0x20, 12))
    if not 1 <= count <= 4096 or item_count > 512:
        raise ValueError("invalid")
    seen, methods = set(), []
    for bucket in range(count):
        node = reader.u32(buckets + bucket * 4)
        while node:
            if node in seen or len(seen) >= 512:
                raise ValueError("invalid")
            seen.add(node)
            start, end, _, _, callback, next_node = struct.unpack("<6I", reader.read(node, 24))
            if not start or not start <= end <= start + 256:
                raise ValueError("invalid")
            name = reader.read(start, end - start).decode("ascii")
            function = reader.u32(reader.u32(callback)) if callback else 0
            methods.append({"name": name, "callback": hex(callback), "onCallback": hex(function),
                            "location": reader.locate(function)})
            node = next_node
    if len(methods) != item_count or reader.read(host + 0x20, 12) != struct.pack("<III", buckets, count, item_count):
        raise ValueError("invalid")
    return methods


def scan_views(reader, base):
    # Cover the full x86 address space: live views can reside above 0xF0000000.
    # Keep the wall-clock deadline and all pointer/signature checks.
    pattern = struct.pack("<I", base + 0x85C2B8)
    address, scanned, started = 0, 0, time.monotonic()
    candidates = {}
    stop_reason = "complete"
    while address < 0x100000000 and time.monotonic() - started < 20 and scanned < 0x100000000:
        region = MemoryRegion()
        if not reader.k.VirtualQueryEx(reader.handle, address, c.byref(region), c.sizeof(region)):
            stop_reason = "query_failed"
            break
        begin, size = region.BaseAddress or 0, region.RegionSize
        if not size or begin + size <= address:
            stop_reason = "query_failed"
            break
        address = begin + size
        if region.State != 0x1000 or region.Type != 0x20000 or region.Protect & 0x101 or not region.Protect & 0xEE:
            continue
        for at in range(max(begin, 0x10000), min(begin + size, 0x100000000), 1048576):
            if time.monotonic() - started >= 20 or scanned >= 0x100000000:
                break
            try:
                length = min(1048576 + 8, begin + size - at, 0x100000000 - at)
                data = reader.read(at, length)
                scanned += length
                offset = data.find(pattern)
                while offset >= 0:
                    try:
                        item = validate_view(reader, at + offset, base + 0x85C2B8)
                        if item:
                            candidates[item["view"]] = item
                    except (ValueError, OSError):
                        pass
                    if len(candidates) >= 32:
                        break
                    offset = data.find(pattern, offset + 4)
            except (ValueError, OSError):
                pass
            if len(candidates) >= 32:
                break
        if len(candidates) >= 32:
            stop_reason = "candidate_limit"
            break
    if time.monotonic() - started >= 20:
        stop_reason = "time_limit"
    elif scanned >= 0x100000000:
        stop_reason = "byte_limit"
    return {"seconds": round(time.monotonic() - started, 3), "scannedBytes": scanned,
            "boundedScan": True, "stopReason": stop_reason, "scanEndAddress": hex(address),
            "byteLimit": 0x100000000, "snapshotIsAtomic": False, "candidates": list(candidates.values())}


def inspect(pid, views=False):
    reader = Reader(pid)
    try:
        modules = {m["name"].lower(): m for m in reader.modules}
        exe, dll = modules.get("simcity.exe"), modules.get("eawebkit.dll")
        if not exe or not dll:
            raise ValueError("missing")
        if hashlib.sha256(Path(exe["path"]).read_bytes()).hexdigest() != EXE_HASH:
            raise ValueError("unsupported")
        static = audit(Path(dll["path"]))
        base = dll["base"]
        # Check the live getter's relocated pointer operand before using the global.
        operand = reader.read(base + 0x59AD7D, 6)
        if operand != b"\x8b\x35" + struct.pack("<I", base + 0x8B2F34):
            raise ValueError("invalid")
        result = {"schema": "openscp.eawebkit-runtime-survey.v1", "pid": pid,
                  "capturedAt": datetime.now(timezone.utc).isoformat(),
                  "readOnly": True, "remoteCalls": False, "snapshotIsAtomic": False,
                  "dll": {"base": hex(base), "sha256": static["sha256"], "path": dll["path"]}}
        factory = reader.read(exe["base"] + 0x74BED0, 3)
        result["inspectorFactory"] = {"module": "SimCity.exe", "rva": "0x74bed0",
                                      "firstBytes": factory.hex(), "returnsNull": factory == b"\x33\xc0\xc3"}
        library_vtable = reader.u32(base + 0x8B39E0)
        client = reader.u32(base + 0x8B2F34)
        result.update(libraryInitialized=library_vtable == base + 0x85C378, client=hex(client))
        if client:
            vtable = reader.u32(client)
            result["clientVtable"] = {"address": hex(vtable), "location": reader.locate(vtable)}
            names = ["destructor", "GetLocalizedString", "LoadUpdate", "DebugLog", "NetworkError", "CreateView",
                     "DestroyView", "ViewUpdate", "CursorChanged", "CursorMoved", "LinkNotification",
                     "NavigationActionsUpdate", "ViewProcessStatus", "TextInputState", "ClipboardEvent",
                     "ProcessEvents", "ReportJSCallstack", "JSMessageBox", "MediaUpdate"]
            result["clientMethods"] = []
            for index, name in enumerate(names):
                function = reader.u32(vtable + index * 4)
                location = reader.locate(function)
                code = reader.read(function, 32) if location else b""
                result["clientMethods"].append({"slot": index, "name": name, "address": hex(function),
                                                 "location": location, "firstBytes": code.hex(),
                                                 "immediateReturn": code.startswith(b"\xc2\x04\x00") or code.startswith(b"\xc3")})
            if reader.u32(base + 0x8B2F34) != client or reader.u32(client) != vtable:
                raise ValueError("invalid")
        if views and result["libraryInitialized"]:
            result["views"] = scan_views(reader, base)
        return result
    finally:
        reader.close()


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    pre = argparse.ArgumentParser(add_help=False)
    pre.add_argument("--lang", choices=TEXT, default="zh-CN")
    selected, _ = pre.parse_known_args()
    labels = TEXT[selected.lang]
    parser = argparse.ArgumentParser(add_help=False, description=labels["description"])
    parser._positionals.title, parser._optionals.title = labels["arguments"], labels["options"]
    parser.add_argument("-h", "--help", action="help", help=labels["help"])
    parser.add_argument("--lang", choices=TEXT, default=selected.lang, help=labels["lang"])
    parser.add_argument("--pid", type=int, required=True, help=labels["pid"])
    parser.add_argument("--scan-views", action="store_true", help=labels["views"])
    parser.add_argument("--output", type=Path, required=True, help=labels["output"])
    args = parser.parse_args()
    try:
        result = inspect(args.pid, args.scan_views)
        args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    except (ValueError, OSError) as error:
        parser.error(labels["error"] + ": " + labels.get(str(error), str(error)))
    print(json.dumps({"report": str(args.output), "pid": args.pid, "readOnly": True,
                      "libraryInitialized": result["libraryInitialized"],
                      "viewCount": len(result.get("views", {}).get("candidates", [])) if args.scan_views else None},
                     ensure_ascii=False))


if __name__ == "__main__":
    main()
