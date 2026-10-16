export type Raw = Record<string, unknown>;
export type Source =
  "resource" | "ui" | "simulation" | "engine" | "marker" | "unknown";
export interface Evidence {
  id: number;
  raw: Raw;
  source: Source;
  stage: string;
  fields: Raw;
  pair?: number;
  changed?: string[];
  issue?: "ambiguous" | "unpaired";
}
export interface DiagnosticSession {
  name: string;
  rows: Evidence[];
  invalid: number[];
  captureStatus?: Raw;
}
const object = (v: unknown): Raw =>
  v !== null && typeof v === "object" && !Array.isArray(v) ? (v as Raw) : {};

/** Decode only the tagged values emitted by the validated UI probe. */
export function decode(v: unknown, depth = 0): unknown {
  if (depth > 20) return { unavailable: "depth" };
  const x = object(v);
  if (x.truncated || x.type === "unavailable") return x;
  if (Array.isArray(x.items))
    return x.items.map((item) => decode(item, depth + 1));
  if (Array.isArray(x.properties))
    return Object.fromEntries(
      x.properties.map((p) => {
        const prop = object(p);
        return [String(prop.name), decode(prop.value, depth + 1)];
      }),
    );
  if ("value" in x) return x.value;
  if ("text" in x) return x.text;
  return v;
}
export function tgi(value: unknown): string | undefined {
  const key = object(value);
  const parts = [key.type, key.group, key.instance].map((v) =>
    typeof v === "number" && Number.isInteger(v) && v >= 0 && v <= 0xffffffff
      ? v.toString(16)
      : v,
  );
  if (
    !parts.every(
      (v) => typeof v === "string" && /^(?:0x)?[\da-f]{1,8}$/i.test(v),
    )
  )
    return;
  return parts
    .map((v) => String(v).replace(/^0x/i, "").padStart(8, "0").toUpperCase())
    .join(":");
}
function normalize(raw: Raw, id: number): Evidence {
  const channel = String(raw.channel ?? "");
  const db = object(raw.database);
  const helper = object(raw.helper);
  let source: Source = "unknown";
  let stage = String(raw.phase ?? raw.channel ?? "unknown");
  let fields: Raw = {};
  if (channel === "resource.provenance" && raw.schema === 1) {
    source = "resource";
    stage = `rule-resource-${raw.phase}`;
    fields = {
      requestId: raw.requestId,
      abstractResourceTgi: tgi(raw.key),
      recordTgi: tgi(raw.recordKey),
      path: db.path,
      provenance: raw.provenance,
      success: raw.success,
      materialize: raw.materialize,
      record: raw.record,
      resource: raw.resource,
    };
  } else if (
    [
      "directory",
      "scan",
      "open",
      "admission",
      "register",
      "find",
      "findRecord",
      "record",
    ].includes(String(raw.phase))
  ) {
    source = "resource";
    if (raw.phase === "record")
      stage = raw.materialize === true ? "materialize" : "probe";
    fields = {
      tgi: tgi(raw.key),
      path: db.path ?? raw.path,
      priority: raw.priority,
      result: raw.result,
      register: raw.register,
      manager: raw.manager,
    };
  } else if (
    channel === "simulation.condition" &&
    raw.schema === 1 &&
    raw.phase === "global-eligibility"
  ) {
    source = "simulation";
    fields = {
      game: raw.game,
      simTicks: raw.simTicks,
      ruleId: raw.ruleId,
      ruleIndex: raw.ruleIndex,
      eligibility: {
        nativeAllowed: raw.nativeAllowed,
        diagnosis: raw.diagnosis,
        operations: raw.operations,
        control: raw.control,
        countCapture: raw.countCapture,
        resourceCount: raw.resourceCount,
        minApply: raw.minApply,
        configuredMaxApply: raw.configuredMaxApply,
        finalApplyCount: raw.finalApplyCount,
      },
      readErrors: raw.readErrors,
    };
  } else if (
    channel === "simulation.effect" &&
    raw.schema === 1 &&
    ["global-resource-update", "unit-local-resource-update"].includes(
      String(raw.phase),
    )
  ) {
    source = "simulation";
    fields = {
      game: raw.game,
      simTicks: raw.simTicks,
      ruleId: raw.ruleId,
      ruleIndex: raw.ruleIndex,
      applyCount: raw.applyCount,
      nativeSuccess: raw.nativeSuccess,
      unitSlot: raw.unitSlot,
      unitPointer: raw.unitPointer,
      unitStamp: raw.unitStamp,
      resourceOwner: raw.resourceOwner,
      resourceEffects: raw.operations,
      readErrors: raw.readErrors,
    };
  } else if (
    channel === "simulation.rule" &&
    raw.schema === 1 &&
    raw.phase === "log-entry" &&
    ["global", "unit"].includes(String(raw.scope))
  ) {
    source = "simulation";
    stage = `${raw.scope}-rule-log-entry`;
    const mapping = object(raw.mapping);
    fields = {
      scope: raw.scope,
      simTicks: raw.simTicks,
      stateIndex: raw.stateIndex,
      unitSlot: raw.unitSlot,
      unitStamp: raw.unitStamp,
      ruleIndex: mapping.ruleIndex,
      ruleSetIndex: mapping.ruleSetIndex,
      ruleWithinSet: mapping.ruleWithinSet,
      ruleId: mapping.ruleIdHex,
      baseRuleWithinSet: mapping.baseRuleWithinSet,
      abstractResourceTgi: tgi(mapping.resourceKey),
      applyCount: raw.applyCount,
      maxApply: raw.maxApply,
      maxPossibleMapApply: raw.maxPossibleMapApply,
      selectedUnits: raw.selectedUnits,
      ruleDefinition: raw.ruleDefinition,
      readErrors: raw.readErrors,
    };
  } else if (
    channel === "delta.envelope" &&
    raw.format === "retail-sldelta-v4"
  ) {
    source = "simulation";
    stage = "delta-envelope";
    fields = {
      deltaId: raw.deltaId,
      simTicks: raw.simTicks,
      kind: raw.kind,
      offset: raw.offset,
      size: raw.size,
      payloadBytes: raw.payloadBytes,
      payloadDecoded: false,
      idRegressed: raw.idRegressed,
      previousDeltaId: raw.previousDeltaId,
    };
  } else if (channel.startsWith("engine.")) {
    source = "engine";
    fields = {
      group: decode(helper.group),
      level: helper.level,
      message: decode(raw.text ?? raw.format),
      sink: raw.sink,
    };
  } else if (channel === "host.marker") {
    source = "marker";
    fields = { message: raw.message };
  } else if (["invoke", "result", "event", "console"].includes(channel)) {
    source = "ui";
    fields = {
      name: raw.name,
      request: raw.requestName,
      binding: raw.binding,
      success: raw.success,
    };
    if (channel === "invoke")
      fields.arguments = Array.isArray(raw.arguments)
        ? raw.arguments.map((v) => decode(v))
        : raw.arguments;
    if (channel === "result") fields.result = decode(raw.result);
    if (channel === "console") fields.message = decode(raw.message);
    if (channel === "event") {
      fields.event = decode(raw.event);
      fields.eventID = raw.eventID;
      if (raw.eventID === 199450092 || raw.eventID === 248287250) {
        source = "simulation";
        stage = raw.eventID === 199450092 ? "unit-ui-event" : "global-ui-event";
        const event = fields.event;
        const payload = Array.isArray(event) ? object(event[1]) : {};
        fields = {
          ...fields,
          unit: payload.unitRef,
          unitEventID: payload.eventID,
          position: payload.eventPosition,
        };
      }
    }
  }
  return {
    id,
    raw,
    source,
    stage,
    fields: Object.fromEntries(
      Object.entries(fields).filter(([, v]) => v !== undefined),
    ),
  };
}
function signature(row: Raw, sequence: unknown) {
  return JSON.stringify([
    row.binding,
    row.page,
    row.name,
    row.requestName,
    sequence,
  ]);
}
function changes(
  before: unknown,
  after: unknown,
  path = "$",
  depth = 0,
): string[] {
  if (JSON.stringify(before) === JSON.stringify(after)) return [];
  if (
    depth < 12 &&
    before &&
    after &&
    typeof before === "object" &&
    typeof after === "object"
  ) {
    const a = before as Raw,
      b = after as Raw;
    return [...new Set([...Object.keys(a), ...Object.keys(b)])].flatMap((k) =>
      changes(a[k], b[k], `${path}.${k}`, depth + 1),
    );
  }
  return [path];
}
export function analyze(
  rawRows: Raw[],
  name: string,
  invalid: number[] = [],
): DiagnosticSession {
  const rows = rawRows.map(normalize);
  const invokes = new Map<string, Evidence[]>();
  for (const row of rows)
    if (row.raw.channel === "invoke" && typeof row.raw.sequence === "number") {
      const key = signature(row.raw, row.raw.sequence);
      invokes.set(key, [...(invokes.get(key) ?? []), row]);
      row.issue = "unpaired";
    }
  const previous = new Map<string, unknown>();
  const resultCounts = new Map<string, number>();
  for (const row of rows)
    if (row.raw.channel === "result") {
      const key = signature(row.raw, row.raw.callSequence);
      resultCounts.set(key, (resultCounts.get(key) ?? 0) + 1);
    }
  for (const row of rows)
    if (row.raw.channel === "result") {
      const keySignature = signature(row.raw, row.raw.callSequence);
      const candidates = invokes.get(keySignature) ?? [];
      if (candidates.length !== 1 || resultCounts.get(keySignature) !== 1) {
        row.issue = candidates.length ? "ambiguous" : "unpaired";
        continue;
      }
      const call = candidates[0];
      row.pair = call.id;
      call.pair = row.id;
      delete call.issue;
      // Include full arguments: resourceAmount/1 must not be compared with resourceAmount/2.
      const key = JSON.stringify([
        call.raw.binding,
        call.raw.page,
        call.raw.name,
        call.fields.arguments,
      ]);
      if (row.raw.success === true) {
        if (previous.has(key))
          row.changed = changes(previous.get(key), row.fields.result);
        previous.set(key, row.fields.result);
      }
    }
  return { name, rows, invalid };
}
export async function parseTrace(
  text: string,
  name: string,
): Promise<DiagnosticSession> {
  const rows: Raw[] = [],
    invalid: number[] = [];
  const lines = text.replace(/^\uFEFF/, "").split("\n");
  let metadata: Raw | undefined;
  for (let i = 0; i < lines.length; i++) {
    if (!lines[i].trim()) continue;
    try {
      const value: unknown = JSON.parse(lines[i]);
      if (value === null || typeof value !== "object" || Array.isArray(value))
        throw new Error();
      const row = value as Raw;
      if (row.channel === "openscp.session" && row.version === 1) {
        if (metadata || rows.length) throw new Error();
        metadata = row;
      } else rows.push(row);
    } catch {
      invalid.push(i + 1);
    }
    if (i % 5000 === 4999)
      await new Promise((resolve) => setTimeout(resolve, 0));
  }
  const session = analyze(rows, name, invalid);
  if (
    metadata?.captureStatus &&
    typeof metadata.captureStatus === "object" &&
    !Array.isArray(metadata.captureStatus)
  )
    session.captureStatus = metadata.captureStatus as Raw;
  return session;
}
export function serializeSession(session: DiagnosticSession): string[] {
  const header = session.captureStatus
    ? [
        JSON.stringify({
          channel: "openscp.session",
          version: 1,
          captureStatus: session.captureStatus,
        }) + "\n",
      ]
    : [];
  return [...header, ...session.rows.map((r) => JSON.stringify(r.raw) + "\n")];
}
/** A TGI query includes path snapshots for its observed databases, never pointer guesses. */
export function selectEvidence(
  rows: Evidence[],
  source: string,
  query: string,
): Evidence[] {
  const q = query.trim().toLowerCase();
  const selected = rows.filter((r) => source === "all" || r.source === source);
  if (!q) return selected;
  const paths = new Set(
    selected
      .filter(
        (r) =>
          r.source === "resource" && String(r.fields.tgi).toLowerCase() === q,
      )
      .map((r) => r.fields.path)
      .filter(Boolean),
  );
  return selected.filter(
    (r) =>
      JSON.stringify(r.fields).toLowerCase().includes(q) ||
      (r.source === "resource" && !r.fields.tgi && paths.has(r.fields.path)),
  );
}
