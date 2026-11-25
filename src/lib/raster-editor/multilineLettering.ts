/** Fit explicit line breaks as one block, with a common font size and line spacing. */
export function layoutMultilineLettering(
  text: string,
  width: number,
  height: number,
  measure: (line: string, fontSize: number) => number,
) {
  const lines = text.trim().split(/\r\n?|\n/);
  if (lines.length < 2) return null;
  const lineSpacing = 1.15;
  const heightFit = height * 0.9 / (1 + (lines.length - 1) * lineSpacing);
  const longest = Math.max(...lines.map((line) => measure(line, heightFit)));
  const fontSize = longest > width * 0.86 ? heightFit * (width * 0.86 / longest) : heightFit;
  return {
    fontSize,
    lines: lines.map((text, index) => ({
      text,
      y: height / 2 + (index - (lines.length - 1) / 2) * fontSize * lineSpacing,
    })),
  };
}
