const agentTerms: Record<string, readonly string[]> = {
  police: ["patrolcar", "policecar", "police", "警车"],
  fire: ["firetruck", "fireengine", "消防车"],
  ambulance: ["ambulance", "救护车"],
  bus: ["schoolbus", "busstop", "citybus", "transitbus", "公交", "校车"],
  garbage: ["garbagetruck", "recyclingtruck", "垃圾车"],
};

export function matchesBridgeTarget(
  target: string | undefined,
  row: Record<string, unknown>,
) {
  if (!target || target === "all") return true;
  const terms = agentTerms[target] ?? [target.toLowerCase()];
  const normalized = JSON.stringify([
    row.requestPath, row.arguments, row.result, row.event, row.message, row.text,
  ]).toLowerCase().replace(/[ _-]+/g, "");
  return terms.some((term) => normalized.includes(term));
}

export function bridgeRequest(row: Record<string, unknown>) {
  const args = row.arguments as { text?: string }[] | undefined;
  return typeof row.requestPath === "string"
    ? row.requestPath
    : (args?.[0]?.text ?? String(row.requestName ?? ""));
}

export function bridgeResult(row: Record<string, unknown>) {
  const result = row.result;
  if (result && typeof result === "object" && "value" in result) {
    const value = result.value;
    if (
      typeof value === "number" ||
      typeof value === "boolean" ||
      typeof value === "string"
    )
      return String(value);
  }
}

export function bridgeEventValue(row: Record<string, unknown>, name: string) {
  const event = row.event as
    | {
        items?: [
          unknown,
          { properties?: { name: string; value?: { value?: unknown } }[] },
        ];
      }
    | undefined;
  return event?.items?.[1]?.properties?.find(
    (property) => property.name === name,
  )?.value?.value;
}
