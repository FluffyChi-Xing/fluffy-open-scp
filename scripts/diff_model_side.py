#!/usr/bin/env python3
"""模型侧字段对拍：解出 DBPF 包内的模型/材质/图形单元字段，供 Packager 与我们
的产物逐项比较（用户要求：LOTM 由 SC5 Asset Editor 迁移、模型构建由 Packager
迁移，因此同一资源两侧结果应完全一致）。

用法：
    python -I scripts/diff_model_side.py <package> [<package2> ...]

输出：每个包的
  LOD 模型资源（组 2F4E681B）：instance / 字节数
  贴图 Raster（组 2F4E681B 之外、type 2F4E681C 等）
  图形单元（组 42E1C000）：parent / bbox / 模型引用 / 内景开关
  Lot（组 40E1C000）模型引用列 00F9EFBB..BE
  Asset 字典（组 40E1C000）内景开关 0F169C46
只读，不修改任何包。
"""
import struct
import sys
import hashlib
from pathlib import Path

SIZES = {
    1: 1, 9: 4, 10: 4, 13: 4, 18: 8, 19: 16, 32: 12, 34: 8,
    48: 8, 49: 12, 50: 12, 51: 16, 52: 16, 56: 48, 57: 24,
}


def refpack(data: bytes, size: int) -> bytes:
    p = 2 + (4 if data[0] & 128 else 3) * (2 if data[0] & 1 else 1)
    out = bytearray()
    while p < len(data):
        b = data[p]
        p += 1
        stop = False
        count = distance = 0
        if b >= 0xE0:
            if b >= 0xFC:
                literal = b & 3
                stop = True
            else:
                literal = ((b & 31) + 1) * 4
        elif b >= 0xC0:
            x, y, z = data[p:p + 3]
            p += 3
            literal = b & 3
            count = ((b & 12) << 6 | z) + 5
            distance = (((b & 16) << 4 | x) << 8 | y) + 1
        elif b >= 0x80:
            x, y = data[p:p + 2]
            p += 2
            literal = x >> 6
            count = (b & 63) + 4
            distance = ((x & 63) << 8 | y) + 1
        else:
            x = data[p]
            p += 1
            literal = b & 3
            count = ((b & 28) >> 2) + 3
            distance = ((b & 96) << 3 | x) + 1
        out.extend(data[p:p + literal])
        p += literal
        if count:
            for _ in range(count):
                out.append(out[-distance])
        if stop:
            break
    return bytes(out)


def entries(path: Path):
    data = path.read_bytes()
    count = struct.unpack_from('<I', data, 0x24)[0]
    index_off = struct.unpack_from('<I', data, 0x40)[0]
    out = []
    for k in range(count):
        v = struct.unpack_from('<6I', data, index_off + 8 + 28 * k)
        t, g, i, off, flag, size = v
        stored = flag & 0x7FFFFFFF
        out.append((t, g, i, off, stored, size, bool(flag & 0x80000000)))
    return data, out


def payload(data, entry):
    t, g, i, off, stored, size, compressed = entry
    blob = data[off:off + stored]
    if not compressed:
        return blob
    try:
        return refpack(blob, size)
    except Exception:
        # 非属性资源（RW4/PNG 等）压缩流不适用于本解码器；对拍只关心属性表。
        return b""


def props(blob):
    """PropertyFile → {hash: (type, payload)}；失败返回 None。"""
    try:
        count = struct.unpack_from('>I', blob)[0]
    except struct.error:
        return None
    pos = 4
    out = {}
    for _ in range(count):
        if pos + 8 > len(blob):
            return out
        h = struct.unpack_from('>I', blob, pos)[0]
        ty = struct.unpack_from('>H', blob, pos + 4)[0]
        size = SIZES.get(ty)
        if size is None:
            return out
        out[h] = (ty, blob[pos + 8:pos + 8 + size])
        pos += 8 + size
    return out


def key_of(raw):
    if len(raw) < 12:
        return None
    i, t, g = struct.unpack('>III', raw[:12])
    return f"{t:08X}:{g:08X}:{i:08X}"


def report(path: Path):
    data, entries_ = entries(path)
    print(f"== {path.name}")
    print(f"   sha256 {hashlib.sha256(data).hexdigest()[:16]}  entries {len(entries_)}")

    print("   LOD 模型（组 2F4E681B）：")
    for e in entries_:
        if e[0] == 0x2F4E681B:
            print(f"     {e[2]:08X}  {e[5]:>9}B")

    for group, label in ((0x42E1C000, "图形单元 42E1C000"), (0x40E1C000, "Lot/Asset 40E1C000")):
        for e in entries_:
            if e[1] != group:
                continue
            p = props(payload(data, e))
            if not p:
                continue
            print(f"   {label} {e[2]:08X}:")
            parent = p.get(0x00B2CCCB)
            if parent:
                print(f"     parent        {key_of(parent[1])}")
            bbox = p.get(0x00F9EFBA)
            if bbox:
                vals = struct.unpack('>6f', bbox[1][:24])
                print(f"     bbox          {[round(v, 3) for v in vals]}")
            for off in range(4):
                lod = p.get(0x00F9EFBB + off)
                if lod:
                    print(f"     lod{off + 1}         {key_of(lod[1])}")
            for h, name in ((0x0F169C46, "interior-ctrl"), (0x0E0BE186, "light-switch")):
                v = p.get(h)
                if v:
                    print(f"     {name:<13} {bool(v[1][0])}")
            if 0x0E0BE186 in p or 0x0F169C46 in p:
                print()


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(__doc__)
        raise SystemExit(2)
    for arg in sys.argv[1:]:
        report(Path(arg))
