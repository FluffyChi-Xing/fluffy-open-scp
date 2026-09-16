import type { SemanticCard, SemanticKeyRef, SemanticTextRef } from "@/api/tauri";

/** RGBA(0-1) → css rgba() 串。 */
function rgbaColor(c: number[]): string {
  const clamp = (v: number) => Math.round(Math.min(1, Math.max(0, v)) * 255);
  const [r, g, b, a] = c;
  return `rgba(${clamp(r)},${clamp(g)},${clamp(b)},${a ?? 1})`;
}

/** 图层卡数据条色带 → CSS 渐变背景；无色带时返回 null。 */
export function barGradientCss(barColors: number[][]): string | null {
  if (!barColors.length) return null;
  const last = Math.max(barColors.length - 1, 1);
  const stops = barColors
    .map(
      (c, i) =>
        `${rgbaColor(c)} ${((i / last) * 100).toFixed(1)}%`,
    )
    .join(", ");
  return `linear-gradient(to right, ${stops})`;
}

/** text 解析失败时回退显示 `#表:实例` 引用。 */
export function textOf(t: SemanticTextRef): string {
  return (
    t.text ??
    `#${t.tableId.toString(16).toUpperCase()}:${t.instanceId
      .toString(16)
      .toUpperCase()}`
  );
}

/** 键引用 → TGI 串。 */
export function keyText(k: SemanticKeyRef): string {
  const hex32 = (value: number) =>
    `0x${value.toString(16).padStart(8, "0").toUpperCase()}`;
  return `${hex32(k.typeId)}:${hex32(k.groupId)}:${hex32(k.instanceId)}`;
}

/** 三卡共有的 Parent 引用（联合类型收窄）。 */
export function cardParent(
  card: SemanticCard,
): SemanticKeyRef | null {
  return "parent" in card ? card.parent : null;
}
