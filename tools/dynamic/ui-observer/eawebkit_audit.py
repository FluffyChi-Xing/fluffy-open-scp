"""Fingerprint-specific static EAWebKit audit. Never loads or executes the DLL."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys


DLL_HASH = "0be0e0c3104739cb56208f91d643c429c8262daff768b2afdcd1799444350e86"
TEXT = {
    "zh-CN": {
        "description": "只读核对零售 EAWebKit DLL 的接口和日志路径，不加载 DLL。",
        "dll": "EAWebKit.dll 路径", "output": "保存 JSON 证据的路径",
        "unsupported": "DLL 指纹不匹配，停止地址分析", "invalid": "二进制结构与已验证版本不符",
        "help": "显示帮助并退出", "lang": "输出语言", "arguments": "位置参数", "options": "选项",
    },
    "en-US": {
        "description": "Read-only retail EAWebKit interface and log audit; never loads the DLL.",
        "dll": "Path to EAWebKit.dll", "output": "Path for JSON evidence",
        "unsupported": "Unsupported DLL fingerprint; address analysis stopped", "invalid": "Binary structure differs from the verified version",
        "help": "Show help and exit", "lang": "Output language", "arguments": "Positional arguments", "options": "Options",
    },
}


def audit(path):
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != DLL_HASH:
        raise ValueError("unsupported")
    nt = struct.unpack_from("<I", data, 60)[0]
    machine, count = struct.unpack_from("<HH", data, nt + 4)
    optional_size = struct.unpack_from("<H", data, nt + 20)[0]
    if data[nt:nt + 4] != b"PE\0\0" or machine != 0x14C:
        raise ValueError("invalid")
    base = struct.unpack_from("<I", data, nt + 52)[0]
    sections = []
    for index in range(count):
        offset = nt + 24 + optional_size + index * 40
        _, rva, size, raw = struct.unpack_from("<4I", data, offset + 8)
        sections.append((rva, size, raw))

    def read(rva, size):
        for start, length, raw in sections:
            if start <= rva and rva + size <= start + length:
                offset = raw + rva - start
                return data[offset:offset + size]
        raise ValueError("invalid")

    names = ["Init", "Shutdown", "Destroy", "GetEAWebKitClient", "GetAllocator",
             "GetFileSystem", "GetTextSystem", "GetThreadSystem", "CreateView", "DestroyView"]
    slots = []
    for index, name in enumerate(names):
        rva = struct.unpack("<I", read(0x85C378 + index * 4, 4))[0] - base
        slots.append({"index": index, "nameFromMatchedSDK": name, "rva": hex(rva)})
    # Slot 3 wraps the global getter in FPU precision preservation.
    getter = read(int(slots[3]["rva"], 16), 40)
    if getter[13:15] != b"\x8b\x35":  # mov esi,[absolute client pointer]
        raise ValueError("invalid")
    client_global = struct.unpack_from("<I", getter, 15)[0] - base
    # Source formats JS logs then dispatches EAWebKitClient::DebugLog at +0x0c.
    # Verify that same sequence in this binary rather than infer from strings alone.
    log_code = read(0x5AC560, 80)
    if b"\x68\x64\xca\x85\x10" not in log_code or b"\x8b\x52\x0c" not in log_code or b"\xff\xd2" not in log_code:
        raise ValueError("invalid")
    return {
        "schema": "openscp.eawebkit-static-audit.v1", "path": str(path.resolve()),
        "sha256": digest, "readOnly": True, "runtimeCalls": False,
        "preferredImageBase": hex(base), "matchedSDKVersion": "12.4.2.2.2",
        "factoryRva": "0x5a4750", "librarySingletonRva": "0x8b39e0",
        "libraryVtableRva": "0x85c378", "librarySlots": slots,
        "clientPointerGlobalRva": hex(client_global),
        "consoleLogDispatch": {"verifiedCodeWindowRva": "0x5ac560", "clientVtableOffset": "0xc"},
        "liveClientAndViewVerified": False,
    }


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    language = argparse.ArgumentParser(add_help=False)
    language.add_argument("--lang", choices=TEXT, default="zh-CN")
    selected, _ = language.parse_known_args()
    labels = TEXT[selected.lang]
    parser = argparse.ArgumentParser(add_help=False, description=labels["description"])
    parser.add_argument("-h", "--help", action="help", help=labels["help"])
    parser.add_argument("--lang", choices=TEXT, default=selected.lang, help=labels["lang"])
    parser._positionals.title = labels["arguments"]
    parser._optionals.title = labels["options"]
    parser.add_argument("dll", type=Path, help=labels["dll"])
    parser.add_argument("--output", type=Path, help=labels["output"])
    args = parser.parse_args()
    try:
        result = audit(args.dll)
    except ValueError as error:
        parser.error(labels[str(error)])
    encoded = json.dumps(result, ensure_ascii=False, indent=2)
    if args.output:
        args.output.write_text(encoded + "\n", encoding="utf-8")
    print(encoded)


if __name__ == "__main__":
    main()
