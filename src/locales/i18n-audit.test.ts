import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { i18n } from "./index";

/**
 * i18n key 审计：源码里所有 t()/$t() 引用的静态 key（含动态模板前缀）
 * 必须在全部语言包中都存在——防止「界面直接显示 token」这类回归。
 * locales 目录自身（key 的定义处）与测试基建不参与扫描。
 */

const LOCALES = ["zh-CN", "en-US"] as const;
type LocaleName = (typeof LOCALES)[number];
const SRC_ROOT = join(process.cwd(), "src");
/** t()/\$t() 调用的第一个字符串参数；负向断言排除 split/at/count 等内建方法名。 */
const KEY_RE = /(?<![\w$])\$?t\(\s*(["'`])([^"'`]+)\1/g;

function collectFiles(dir: string, out: string[] = []): string[] {
  for (const name of readdirSync(dir)) {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) {
      if (name === "locales" || name === "test") continue;
      collectFiles(full, out);
    } else if (
      /\.(vue|ts)$/.test(name) &&
      !/\.(test|spec|d)\.ts$/.test(name)
    ) {
      out.push(full);
    }
  }
  return out;
}

function flatten(
  value: unknown,
  prefix = "",
  out: Set<string> = new Set(),
): Set<string> {
  if (value && typeof value === "object") {
    for (const [key, child] of Object.entries(
      value as Record<string, unknown>,
    )) {
      const path = prefix ? `${prefix}.${key}` : key;
      out.add(path);
      flatten(child, path, out);
    }
  }
  return out;
}

/** 仅收集值为字符串的叶子 key（vue-i18n 消息本体）。 */
function leafKeys(
  value: unknown,
  prefix = "",
  out: string[] = [],
): string[] {
  if (value && typeof value === "object") {
    for (const [key, child] of Object.entries(
      value as Record<string, unknown>,
    )) {
      const path = prefix ? `${prefix}.${key}` : key;
      if (child && typeof child === "object") leafKeys(child, path, out);
      else if (typeof child === "string") out.push(path);
    }
  }
  return out;
}

const localeKeys = new Map<LocaleName, Set<string>>(
  LOCALES.map((locale) => [
    locale,
    flatten(i18n.global.getLocaleMessage(locale)),
  ]),
);

const staticKeys: string[] = [];
const dynamicPrefixes: string[] = [];
for (const file of collectFiles(SRC_ROOT)) {
  const source = readFileSync(file, "utf8");
  for (const [, , raw] of source.matchAll(KEY_RE)) {
    const marker = raw.indexOf("${");
    if (marker === -1) {
      staticKeys.push(raw);
    } else if (marker > 0) {
      dynamicPrefixes.push(raw.slice(0, marker));
    }
  }
}

describe("i18n key audit", () => {
  it("静态 key 在每个语言包都存在", () => {
    const missing: string[] = [];
    for (const key of new Set(staticKeys)) {
      for (const locale of LOCALES) {
        if (!localeKeys.get(locale)!.has(key)) {
          missing.push(`${key} @ ${locale}`);
        }
      }
    }
    expect(missing, `缺失 key：\n${missing.join("\n")}`).toEqual([]);
  });

  it("动态 key 前缀在每个语言包都有落点", () => {
    const missing: string[] = [];
    for (const prefix of new Set(dynamicPrefixes)) {
      for (const locale of LOCALES) {
        const hit = [...localeKeys.get(locale)!].some((key) =>
          key.startsWith(prefix),
        );
        if (!hit) missing.push(`${prefix}… @ ${locale}`);
      }
    }
    expect(missing, `缺失前缀：\n${missing.join("\n")}`).toEqual([]);
  });

  it("全部消息可被 vue-i18n 编译（@/{ 等语法字符会让 t() 抛错）", () => {
    const broken: string[] = [];
    const originalLocale = i18n.global.locale.value;
    const originalWarn = console.warn;
    console.warn = () => {};
    try {
      for (const locale of LOCALES) {
        i18n.global.locale.value = locale;
        const messages = i18n.global.getLocaleMessage(locale);
        for (const key of leafKeys(messages)) {
          try {
            i18n.global.t(key);
          } catch (cause) {
            broken.push(
              `${key} @ ${locale}: ${cause instanceof Error ? cause.message : String(cause)}`,
            );
          }
        }
      }
    } finally {
      i18n.global.locale.value = originalLocale;
      console.warn = originalWarn;
    }
    expect(broken, `编译失败的消息：\n${broken.join("\n")}`).toEqual([]);
  });
});
