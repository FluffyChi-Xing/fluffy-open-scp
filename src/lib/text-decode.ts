export interface DecodedText {
  content: string;
  /** utf-8 | utf-8-bom | utf-16le | utf-8+binary（二进制段已标记化）。 */
  encoding: string;
}

/**
 * 资源文本解码：很多"文本类型"资源并非纯文本——如 SimCity shader 容器
 * （NUL 结尾的名字 + 二进制元数据 + 源码块）。策略：
 * 1. BOM 识别（UTF-16LE / UTF-8）
 * 2. 严格 UTF-8，失败退宽松解码（非法序列 → U+FFFD）
 * 3. 解码结果含控制字符（NUL/C0/C1，Tab/LF/CR 除外）或 U+FFFD 即认定含
 *    二进制帧——NUL 是合法 UTF-8，仅靠严格解码失败判定不住——把连续段
 *    折叠为 ⟦N B⟫ 可见标记，源码主体保持可读且如实标注二进制位置
 */
export function decodeTextBytes(bytes: Uint8Array<ArrayBuffer>): DecodedText {
  if (bytes.length >= 2 && bytes[0] === 0xff && bytes[1] === 0xfe) {
    return finish(new TextDecoder("utf-16le").decode(bytes), "utf-16le");
  }
  const bomUtf8 = bytes.length >= 3 && bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf;
  const body = bomUtf8 ? bytes.subarray(3) : bytes;
  try {
    return finish(
      new TextDecoder("utf-8", { fatal: true }).decode(body),
      bomUtf8 ? "utf-8-bom" : "utf-8",
    );
  } catch {
    return finish(new TextDecoder("utf-8").decode(body), "utf-8");
  }
}

function finish(content: string, encoding: string): DecodedText {
  if (BINARY_RUN_TEST.test(content)) {
    return {
      content: content.replace(BINARY_RUN, (run) => `⟦${run.length}B⟫`),
      encoding: "utf-8+binary",
    };
  }
  return { content, encoding };
}

const BINARY_RUN_TEST = /[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F-\u009F\uFFFD]+/;
const BINARY_RUN = /[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F-\u009F\uFFFD]+/g;

/**
 * 未知类型的文本嗅探（首 4KB）：全可打印 ASCII，或严格 UTF-8 且
 * 不含控制字符（NUL/C0/C1）。纯二进制查表资源 → 落回十六进制视图。
 */
export function looksLikeText(bytes: Uint8Array): boolean {
  if (bytes.length === 0) return false;
  let printable = true;
  for (const byte of bytes) {
    if (byte === 9 || byte === 10 || byte === 13 || (byte >= 32 && byte <= 126)) {
      continue;
    }
    printable = false;
    break;
  }
  if (printable) return true;
  try {
    return !BINARY_RUN_TEST.test(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
  } catch {
    return false;
  }
}

/**
 * locale(0x0a98eaf0) 资源 = 3 字节前缀 + JSON 字符串表（见 migration.md）。
 * 保守剥离：仅当前缀后紧跟 "{" 时认定成立。
 */
export function stripLocaleJsonPrefix(bytes: Uint8Array<ArrayBuffer>): Uint8Array<ArrayBuffer> {
  if (
    bytes.length > 3 &&
    bytes[0] !== 0x7b &&
    (bytes[3] === 0x7b || bytes[3] === 0x20 || bytes[3] === 0x0a || bytes[3] === 0x0d)
  ) {
    return bytes.subarray(3) as Uint8Array<ArrayBuffer>;
  }
  return bytes;
}
