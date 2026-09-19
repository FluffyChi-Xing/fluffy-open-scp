/**
 * DBPF 容器写出器 —— crates/dbpf `write_uncompressed_overlay` 的 TS 移植。
 *
 * 字节级镜像零售 SimCity 包：magic `DBPF`、major=3、96 字节头、
 * 索引前导 `4 + 0`、28 字节记录（type/group/instance/offset/size/mem/flags，
 * 小端），资源原样存储（不压缩）。资源按 (type, group, instance) 排序。
 *
 * fixture 对拍：crates/dbpf/examples/dbpf_fixture.rs —— 见 dbpf-writer.test.ts。
 */

export interface DbpfEntry {
  type: number;
  group: number;
  instance: number;
  data: Uint8Array;
}

export function writeUncompressedOverlay(input: DbpfEntry[]): Uint8Array {
  const entries = [...input].sort(
    (a, b) => a.type - b.type || a.group - b.group || a.instance - b.instance,
  );
  for (let i = 1; i < entries.length; i++) {
    const a = entries[i - 1];
    const b = entries[i];
    if (a.type === b.type && a.group === b.group && a.instance === b.instance) {
      throw new Error(
        `duplicate resource TGI 0x${b.type.toString(16)}/0x${b.group.toString(16)}/0x${b.instance.toString(16)}`,
      );
    }
  }

  const indexLen = 8 + entries.length * 28;
  const payloadBase = 96 + indexLen;
  const payloadLen = entries.reduce((sum, e) => sum + e.data.length, 0);
  const out = new Uint8Array(payloadBase + payloadLen);
  const view = new DataView(out.buffer);

  out.set([0x44, 0x42, 0x50, 0x46]); // "DBPF"
  view.setInt32(4, 3, true); // major = 3（零售包实测一致）
  view.setInt32(8, 0, true); // minor
  // [12..36) 保留零
  view.setUint32(36, entries.length, true);
  view.setUint32(40, 0, true);
  view.setUint32(44, indexLen, true);
  // [48..60) 保留零
  view.setUint32(60, 3, true); // reserved@0x3C
  view.setUint32(64, 96, true); // 索引偏移
  // [68..96) 保留零
  view.setInt32(96, 4, true); // 索引前导：shared unknown 数
  view.setUint32(100, 0, true);

  let offset = payloadBase;
  entries.forEach((entry, i) => {
    const record = 104 + i * 28;
    view.setUint32(record, entry.type >>> 0, true);
    view.setUint32(record + 4, entry.group >>> 0, true);
    view.setUint32(record + 8, entry.instance >>> 0, true);
    view.setUint32(record + 12, offset, true);
    view.setUint32(record + 16, entry.data.length, true);
    view.setUint32(record + 20, entry.data.length, true);
    view.setInt16(record + 24, 0, true);
    view.setUint16(record + 26, 0, true);
    out.set(entry.data, offset);
    offset += entry.data.length;
  });
  return out;
}
