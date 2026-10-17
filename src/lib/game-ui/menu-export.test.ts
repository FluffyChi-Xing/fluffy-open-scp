import { describe, expect, it } from "vitest";
import {
  HASH,
  MARQUEE_TYPE,
  buildMenuExport,
  buildMenuPropertyResource,
  iconInstanceFromAsset,
  marqueeInstanceFromAsset,
  type MenuExportEntry,
} from "./menu-export";

/** 菜单条目 property 的最小只读解析（字节布局镜像 prop-format.ts，大端）。 */
function decodeProperty(bytes: Uint8Array): Map<number, { fileType: number; array: boolean; data: Uint8Array }> {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let offset = 0;
  const count = view.getUint32(offset);
  offset += 4;
  const out = new Map();
  for (let i = 0; i < count; i++) {
    const hash = view.getUint32(offset);
    const fileType = view.getUint16(offset + 4);
    const flags = view.getUint16(offset + 6);
    offset += 8;
    const bodyStart = offset;
    const sizeOfType = (type: number): number => (type === 1 ? 1 : type === 32 ? 12 : type === 34 ? 8 : 4);
    if ((flags & 0x30) === 0x30) {
      const values = view.getInt32(offset);
      const itemSize = view.getInt32(offset + 4);
      offset += 8 + values * itemSize;
    } else {
      offset += sizeOfType(fileType);
    }
    out.set(hash, { fileType, array: (flags & 0x30) === 0x30, data: bytes.slice(bodyStart, offset) });
  }
  return out;
}

function keyInstance(data: Uint8Array): number {
  return new DataView(data.buffer, data.byteOffset, data.byteLength).getUint32(0);
}

const BASE: MenuExportEntry = {
  instance: 0x7dd8a627,
  uiCategories: [0xac12c904],
  uiPosition: 50,
  titleStringId: 0x1234,
};

describe("复刻资产路径 → 资源 instance 反推", () => {
  it("rollover 大图 tool_<instance>.jpg", () => {
    expect(marqueeInstanceFromAsset("/game-ui/replica/assets/tool_7DD8A627.jpg")).toBe(0x7dd8a627);
    expect(marqueeInstanceFromAsset("tool_AF256395.jpg")).toBe(0xaf256395);
    expect(marqueeInstanceFromAsset("/game-ui/replica/assets/tool_icon_2531E3C9.png")).toBeNull();
    expect(marqueeInstanceFromAsset("/game-ui/replica/assets/tool_placeholder.png")).toBeNull();
  });

  it("槽位图标 tool_icon_<instance>.png", () => {
    expect(iconInstanceFromAsset("/game-ui/replica/assets/tool_icon_2531E3C9.png")).toBe(0x2531e3c9);
    expect(iconInstanceFromAsset("/game-ui/replica/assets/tool_7DD8A627.jpg")).toBeNull();
    expect(iconInstanceFromAsset("/game-ui/replica/assets/tool_placeholder.png")).toBeNull();
  });
});

describe("buildMenuPropertyResource：marquee/图标键按需写入", () => {
  it("无 marqueeInstance 不写 marquee 键（新增无图/用户移除 → 无悬空引用）", () => {
    const entries = decodeProperty(buildMenuPropertyResource(BASE));
    expect(entries.has(HASH.marquee)).toBe(false);
  });

  it("marqueeInstance 指向原资源（拷贝条目共享原图）", () => {
    const entries = decodeProperty(
      buildMenuPropertyResource({ ...BASE, marqueeInstance: 0xaf256395 }),
    );
    expect(entries.has(HASH.marquee)).toBe(true);
    expect(keyInstance(entries.get(HASH.marquee)!.data)).toBe(0xaf256395);
  });

  it("iconInstance 不带字节时仍写图标键（引用基础游戏图标资源）", () => {
    const entries = decodeProperty(
      buildMenuPropertyResource({ ...BASE, iconInstance: 0x2531e3c9 }),
    );
    expect(entries.has(HASH.toolIconKey)).toBe(true);
    expect(entries.has(HASH.iconNormal)).toBe(true);
    const key = entries.get(HASH.iconNormal)!.data;
    const view = new DataView(key.buffer, key.byteOffset, key.byteLength);
    expect(view.getUint32(0)).toBe(0x2531e3c9);
    expect(view.getUint32(4)).toBe(0x2f7d0004);
    expect(view.getUint32(8)).toBe(0x40e02400);
  });
});

describe("buildMenuExport", () => {
  it("空 entries 产出空覆盖包且不读取首项（旧版崩溃回归）", () => {
    const result = buildMenuExport({ entries: [], locale: [] });
    expect(result.property.byteLength).toBe(0);
  });

  it("新大图字节落在 marqueeInstance 指向的资源上", () => {
    const bytes = new Uint8Array([1, 2, 3]);
    const result = buildMenuExport({
      entries: [{ ...BASE, marqueeInstance: 0x7dd8a627, marqueeJpg: bytes }],
      locale: [],
    });
    // 包内容不做完整 DBPF 解析；这里验证不抛错且属性里键存在
    expect(result.package.byteLength).toBeGreaterThan(0);
    expect(result.property.byteLength).toBeGreaterThan(0);
    const entries = decodeProperty(result.property);
    expect(entries.has(HASH.marquee)).toBe(true);
    expect(keyInstance(entries.get(HASH.marquee)!.data)).toBe(0x7dd8a627);
  });

  it("marquee 资源 type = 0x3F8662EA", () => {
    expect(MARQUEE_TYPE).toBe(0x3f8662ea);
  });
});
