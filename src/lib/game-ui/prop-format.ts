/**
 * SimCity 属性资源（0x00B1B104）二进制写出器。
 *
 * 字节布局镜像 crates/sc-properties `PropertyFile::encode_canonical`（BE）：
 *   count:u32
 *   每条目 { hash:u32, fileType:u16, flags:u16, values }
 *     flags bit0x30 = 数组、bit0x40 = 空数组；数组前写 count:i32 + itemSize:i32
 *     值序：bool 1B / int32·uint32 4B / key = instance,typeId,group 12B /
 *           text = tableId,instanceId 8B（全部大端）
 *
 * fixture 对拍：crates/sc-properties/examples/prop_fixture.rs（Rust encode_canonical
 * 输出的十六进制）——见 binary-writer.test.ts。
 */

export type PropValue =
  | { kind: "bool"; value: boolean }
  | { kind: "int32"; value: number }
  | { kind: "uint32"; value: number }
  | { kind: "key"; instance: number; typeId: number; group: number }
  | { kind: "text"; tableId: number; instanceId: number };

export interface PropEntry {
  hash: number;
  /** 元素类型码（决定 fileType 与值编码），空数组也必须给出。 */
  valueType: PropValue["kind"];
  /** 缺省 false = 标量条目；true = 数组条目（空数组合法）。 */
  array?: boolean;
  /** 标量时恰好 1 个；数组时 0..n。 */
  values: PropValue[];
}

const FILE_TYPE: Record<PropValue["kind"], number> = {
  bool: 1,
  int32: 9,
  uint32: 10,
  key: 32,
  text: 34,
};

const FIXED_SIZE: Record<PropValue["kind"], number> = {
  bool: 1,
  int32: 4,
  uint32: 4,
  key: 12,
  text: 8,
};

/** 数组/空数组标志位（与 Rust parse/encode 一致）。 */
const FLAG_ARRAY = 0x30;
const FLAG_EMPTY = 0x40;

export function encodePropertyFile(input: PropEntry[]): Uint8Array {
  const entries = [...input].sort((a, b) => a.hash - b.hash);
  const seen = new Set<number>();
  for (const entry of entries) {
    if (seen.has(entry.hash)) throw new Error(`duplicate property hash 0x${entry.hash.toString(16)}`);
    seen.add(entry.hash);
  }

  const out: number[] = [];
  const u8 = (v: number): void => {
    out.push(v & 0xff);
  };
  const u16 = (v: number): void => {
    out.push((v >>> 8) & 0xff, v & 0xff);
  };
  const u32 = (v: number): void => {
    out.push((v >>> 24) & 0xff, (v >>> 16) & 0xff, (v >>> 8) & 0xff, v & 0xff);
  };
  const i32 = (v: number): void => {
    u32(v >>> 0);
  };

  u32(entries.length);
  for (const entry of entries) {
    u32(entry.hash >>> 0);
    u16(FILE_TYPE[entry.valueType]);
    const isEmptyArray = entry.array === true && entry.values.length === 0;
    u16(isEmptyArray ? FLAG_ARRAY | FLAG_EMPTY : entry.array === true ? FLAG_ARRAY : 0);

    if (entry.array !== true) {
      writeValue(u8, u32, entry.values[0] as PropValue);
      continue;
    }
    if (!isEmptyArray) {
      i32(entry.values.length);
      i32(FIXED_SIZE[entry.valueType]);
      for (const value of entry.values) writeValue(u8, u32, value);
    }
  }
  return Uint8Array.from(out);
}

function writeValue(u8: (v: number) => void, u32: (v: number) => void, value: PropValue): void {
  switch (value.kind) {
    case "bool":
      u8(value.value ? 1 : 0);
      break;
    case "int32":
      u32(value.value >>> 0);
      break;
    case "uint32":
      u32(value.value >>> 0);
      break;
    case "key":
      u32(value.instance >>> 0);
      u32(value.typeId >>> 0);
      u32(value.group >>> 0);
      break;
    case "text":
      u32(value.tableId >>> 0);
      u32(value.instanceId >>> 0);
      break;
  }
}
