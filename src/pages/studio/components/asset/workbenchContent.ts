/** Asset content identity excludes transient viewport controls and JSON layout. */
export function workbenchContentKey(document: Record<string, unknown> | null): string {
  if (!document) return "";
  const { editor: rawEditor, units: rawUnits, ...content } = document;
  const editor = Object.fromEntries(Object.entries((rawEditor ?? {}) as Record<string, unknown>)
    .filter(([key]) => !["groups", "unitVisibility", "selectedId"].includes(key)));
  const units = Array.isArray(rawUnits) ? rawUnits.map((row) => {
    return Object.fromEntries(Object.entries(row as Record<string, unknown>).filter(([key]) => key !== "visible"));
  }) : rawUnits;
  return JSON.stringify({ ...content, units, editor });
}
