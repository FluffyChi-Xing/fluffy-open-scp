/**
 * 文档字数统计（编辑器状态栏用，口径对齐 Obsidian）：
 * - 词数：中日韩（汉字/假名）逐字计一词（无空格分词），西文与韩文按
 *   连续字母/数字串计一词；
 * - 字符数：不含空白字符（空格/换行/制表符）。
 */
export interface DocumentStats {
  words: number;
  characters: number;
}

/**
 * 词元匹配：单个汉字/假名各成一词；韩文音节、西文字母数字连续串合并
 * 为一词。连字符/撇号仅在与字母数字相邻时并入词内（well-known 算一词，
 * 而列表标记 “- ” 不算词）。
 */
const WORD_TOKEN_RE =
  /[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}]|[\p{Script=Hangul}A-Za-z0-9_’']+(?:(?:-|')[\p{Script=Hangul}A-Za-z0-9_’']+)*/gu;

export function countWords(text: string): number {
  let words = 0;
  for (const match of text.matchAll(WORD_TOKEN_RE)) {
    if (match[0]) words += 1;
  }
  return words;
}

export function countCharacters(text: string): number {
  return text.replace(/\s/g, "").length;
}

export function documentStats(text: string): DocumentStats {
  return { words: countWords(text), characters: countCharacters(text) };
}
