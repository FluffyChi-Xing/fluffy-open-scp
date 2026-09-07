import { describe, expect, it } from "vitest";
import { decodeTextBytes, looksLikeText, stripLocaleJsonPrefix } from "./text-decode";

const utf8 = (text: string) => new TextEncoder().encode(text) as Uint8Array<ArrayBuffer>;

describe("decodeTextBytes", () => {
  it("严格 UTF-8 原样通过", () => {
    const result = decodeTextBytes(utf8("float4 worldPosition;\n// 中文注释"));
    expect(result.encoding).toBe("utf-8");
    expect(result.content).toBe("float4 worldPosition;\n// 中文注释");
  });

  it("UTF-16LE BOM 识别", () => {
    const bytes = new Uint8Array([0xff, 0xfe, 0x41, 0x00, 0x42, 0x00]);
    expect(decodeTextBytes(bytes)).toEqual({ content: "AB", encoding: "utf-16le" });
  });

  it("UTF-8 BOM 识别", () => {
    const bytes = new Uint8Array([0xef, 0xbb, 0xbf, ...utf8("hi")]);
    expect(decodeTextBytes(bytes)).toEqual({ content: "hi", encoding: "utf-8-bom" });
  });

  it("shader 容器二进制帧折叠为可见标记", () => {
    // "modelToClip" + NUL + [06 00 06 00 ...] + "NullVS"（模拟真实探针字节布局）
    const bytes = new Uint8Array([
      ...utf8("float4x4 modelToClip"),
      0x00, 0x06, 0x00, 0x06, 0x00, 0x04, 0x00, 0x00,
      0x00, 0x40, 0x01, 0x00, 0x00, 0x00, 0x00, 0x06,
      ...utf8("NullVS"),
    ]);
    const result = decodeTextBytes(bytes);
    expect(result.encoding).toBe("utf-8+binary");
    expect(result.content).toContain("float4x4 modelToClip");
    expect(result.content).toContain("NullVS");
    expect(result.content).toContain("⟦9B⟫@⟦6B⟫NullVS");
    // 除二进制标记外只能含可打印字符与 Tab/LF/CR
    expect(result.content).toMatch(/^(?:[ -~]|⟦\d+B⟫)+$/u);
  });

  it("制表符与换行不被折叠", () => {
    const bytes = new Uint8Array([...utf8("a\tb\nc\r\nd"), 0x00, 0x01, 0x02]);
    const result = decodeTextBytes(bytes);
    expect(result.content).toBe("a\tb\nc\r\nd⟦3B⟫");
  });
});

describe("looksLikeText", () => {
  it("全可打印 ASCII 为真", () => {
    expect(looksLikeText(utf8("plain text\r\nline2"))).toBe(true);
  });

  it("合法 UTF-8 多字节为真", () => {
    expect(looksLikeText(utf8("// 注释\nint x;"))).toBe(true);
  });

  it("含 NUL 的二进制为假", () => {
    expect(looksLikeText(new Uint8Array([0x00, 0x00, 0x00, 0x01, 0x9f, 0x0d]))).toBe(false);
  });

  it("空输入为假", () => {
    expect(looksLikeText(new Uint8Array())).toBe(false);
  });
});

describe("stripLocaleJsonPrefix", () => {
  it("3 字节前缀后紧跟 JSON 花括号时剥离", () => {
    const bytes = new Uint8Array([0x01, 0x00, 0x00, 0x7b, 0x22, 0x7d]);
    expect(Buffer.from(stripLocaleJsonPrefix(bytes)).toString()).toBe('{"}');
  });

  it("前缀后非花括号则原样返回", () => {
    const bytes = new Uint8Array([0x01, 0x02, 0x03, 0x04, 0x05]);
    expect(stripLocaleJsonPrefix(bytes)).toBe(bytes);
  });
});
