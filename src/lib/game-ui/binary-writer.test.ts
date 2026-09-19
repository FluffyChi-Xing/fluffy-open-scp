/**
 * 二进制写出器对拍测试：期望字节由 Rust 侧权威实现生成 ——
 *   property: crates/sc-properties/examples/prop_fixture.rs（encode_canonical）
 *   dbpf:     crates/dbpf/examples/dbpf_fixture.rs（write_uncompressed_overlay）
 * Rust 实现或字段布局变更时，重新运行对应示例并更新 fixture 十六进制。
 */
import { describe, expect, it } from "vitest";
import { encodePropertyFile, type PropEntry } from "./prop-format";
import { writeUncompressedOverlay, type DbpfEntry } from "./dbpf-writer";

const PROP_FIXTURE =
  "000000050977aa8f0020000083f06c632f7d000440e024000a09f5fa002200006c969dee64ec016b0dc1e3e0000900000000012c0eb1fc0500220030000000020000000850aa0bea0eb1fe4150aa0bea0eb1fe440f1a181d0001000001";

const DBPF_FIXTURE =
  "444250460300000000000000000000000000000000000000000000000000000000000000020000000000000040000000000000000000000000000000030000006000000000000000000000000000000000000000000000000000000000000000040000000000000004b1b100018a870934800864a0000000030000000300000000000000f0ea980a01bffa0201000000a30000000200000002000000000000000102034142";

describe("encodePropertyFile（对拍 Rust encode_canonical）", () => {
  const entries: PropEntry[] = [
    { hash: 0x0dc1e3e0, valueType: "int32", values: [{ kind: "int32", value: 300 }] },
    {
      hash: 0x0a09f5fa,
      valueType: "text",
      values: [{ kind: "text", tableId: 0x6c969dee, instanceId: 0x64ec016b }],
    },
    {
      hash: 0x0977aa8f,
      valueType: "key",
      values: [
        { kind: "key", instance: 0x83f06c63, typeId: 0x2f7d0004, group: 0x40e02400 },
      ],
    },
    {
      hash: 0x0eb1fc05,
      valueType: "text",
      array: true,
      values: [
        { kind: "text", tableId: 0x50aa0bea, instanceId: 0x0eb1fe41 },
        { kind: "text", tableId: 0x50aa0bea, instanceId: 0x0eb1fe44 },
      ],
    },
    { hash: 0x0f1a181d, valueType: "bool", values: [{ kind: "bool", value: true }] },
  ];

  it("与 Rust encode_canonical 字节一致", () => {
    const bytes = encodePropertyFile(entries);
    expect(Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("")).toBe(
      PROP_FIXTURE,
    );
  });

  it("输入顺序无关（canonical 排序）", () => {
    const reversed = encodePropertyFile([...entries].reverse());
    expect(Array.from(reversed, (b) => b.toString(16).padStart(2, "0")).join("")).toBe(
      PROP_FIXTURE,
    );
  });

  it("拒绝重复哈希", () => {
    expect(() =>
      encodePropertyFile([
        { hash: 0x01, valueType: "int32", values: [{ kind: "int32", value: 1 }] },
        { hash: 0x01, valueType: "int32", values: [{ kind: "int32", value: 2 }] },
      ]),
    ).toThrow();
  });

  it("空数组写空数组标志", () => {
    const bytes = encodePropertyFile([
      { hash: 0x0eb1fc05, valueType: "text", array: true, values: [] },
    ]);
    // hash + type34 + flags(0x50)
    expect(Array.from(bytes.slice(4, 12), (b) => b.toString(16).padStart(2, "0")).join("")).toBe(
      "0eb1fc0500220070",
    );
  });
});

describe("writeUncompressedOverlay（对拍 Rust write_uncompressed_overlay）", () => {
  const entries: DbpfEntry[] = [
    { type: 0x0a98eaf0, group: 0x02fabf01, instance: 0x00000001, data: new Uint8Array([0x41, 0x42]) },
    { type: 0x00b1b104, group: 0x09878a01, instance: 0x64088034, data: new Uint8Array([1, 2, 3]) },
  ];

  it("与 Rust write_uncompressed_overlay 字节一致", () => {
    const bytes = writeUncompressedOverlay(entries);
    expect(Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("")).toBe(
      DBPF_FIXTURE,
    );
  });

  it("记录按 (type, group, instance) 排序", () => {
    const bytes = writeUncompressedOverlay([...entries].reverse());
    expect(Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("")).toBe(
      DBPF_FIXTURE,
    );
  });

  it("拒绝重复 TGI", () => {
    expect(() => writeUncompressedOverlay([entries[0], entries[0]])).toThrow();
  });
});
