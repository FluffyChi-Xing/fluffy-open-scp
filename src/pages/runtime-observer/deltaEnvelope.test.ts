import { describe, expect, it } from "vitest";
import { webcrypto } from "node:crypto";
import { parseDelta } from "./deltaEnvelope";
import { analyze, parseTrace, serializeSession } from "./diagnostics";
import { captureQuality } from "./captureQuality";
import { useDiagnosticSession } from "./useDiagnosticSession";

// Browser test environment may lack SubtleCrypto; use the same WebCrypto API.
Object.defineProperty(globalThis, "crypto", {
  value: webcrypto,
  configurable: true,
});
function envelope(ids = [54377n, 54396n]) {
  const data = new ArrayBuffer(4 + 24 * ids.length),
    v = new DataView(data);
  v.setUint32(0, 4, true);
  ids.forEach((id, i) => {
    const p = 4 + i * 24;
    v.setUint32(p, 0xff020300);
    v.setUint32(p + 4, 24);
    v.setBigUint64(p + 8, id);
    v.setUint32(p + 16, 109790);
    v.setUint32(p + 20, 7);
  });
  return data;
}
describe("retail delta envelope", () => {
  it("retains unsigned 64-bit IDs, offsets, ticks and source digest without inferring actions", async () => {
    const s = await parseDelta(envelope([9007199254740993n]), "copy.mfs");
    expect(s.rows[0].fields).toMatchObject({
      deltaId: "9007199254740993",
      offset: 4,
      simTicks: 109790,
      payloadDecoded: false,
    });
    expect(s.rows[0].raw.provenance).toMatchObject({
      file: "copy.mfs",
      sha256: expect.stringMatching(/^[0-9a-f]{64}$/),
    });
    expect(s.rows[0].fields).not.toHaveProperty("rule");
    expect(s.rows[0].raw).not.toHaveProperty("time");
  });
  it("rejects unsupported headers, malformed sizes and truncation", async () => {
    const version = envelope();
    new DataView(version).setUint32(0, 5, true);
    await expect(parseDelta(version, "x")).rejects.toMatchObject({
      code: "deltaVersion",
      header: "05 00 00 00",
    });
    const size = envelope();
    new DataView(size).setUint32(8, 23);
    await expect(parseDelta(size, "x")).rejects.toMatchObject({
      code: "deltaInvalid",
      offset: 4,
    });
    await expect(
      parseDelta(envelope().slice(0, -1), "x"),
    ).rejects.toMatchObject({ code: "deltaInvalid", offset: 28 });
  });
  it("preserves decreasing IDs in file order, as observed in real save deltas", async () => {
    const session = await parseDelta(
      envelope([63528n, 63475n]),
      "SLDelta8_1.mfs",
    );
    expect(session.rows.map((r) => r.fields.deltaId)).toEqual([
      "63528",
      "63475",
    ]);
    expect(session.rows[1].fields).toMatchObject({
      idRegressed: true,
      previousDeltaId: "63528",
      offset: 28,
    });
  });
  it("preserves the previous imported session when a delta fails", async () => {
    const state = useDiagnosticSession();
    const previous = analyze([{ channel: "invoke", sequence: 1 }], "previous");
    state.session.value = previous;
    await state.importFile({
      name: "broken.mfs",
      size: 2,
      arrayBuffer: async () => new ArrayBuffer(2),
    } as File);
    expect(state.session.value).toBe(previous);
    expect(state.error.value).toBe("deltaInvalid");
  });
});
describe("capture quality", () => {
  it("keeps unknown values unknown and avoids double counting channel drops", () => {
    expect(captureQuality({}, 1).dropped).toBeUndefined();
    const q = captureQuality(
      {
        records: 3,
        dropped: 9,
        decodeErrors: 0,
        agentErrors: [],
        channels: { query: { hits: 12, accepted: 3, dropped: 9, queued: 0 } },
      },
      2,
    );
    expect(q.dropped).toBe(9);
    expect(q.errors).toBe(0);
    expect(q.countMatches).toBe(false);
    expect(q.channels[0].dropped).toBe(9);
  });
  it("keeps engine host errors separate from a successful agent summary", () => {
    expect(
      captureQuality(
        {
          summary: { total: 384, dropped: 0, errors: 0 },
          errors: ["detach failure"],
        },
        384,
      ),
    ).toMatchObject({
      reported: 384,
      errors: 0,
      hostErrors: 1,
      countMatches: true,
    });
  });
  it("round-trips selected status through marker insertion and session export", async () => {
    const state = useDiagnosticSession();
    state.session.value = analyze([{ channel: "engine.console" }], "engine");
    await state.importStatus({
      size: 10,
      text: async () =>
        JSON.stringify({ summary: { total: 1, dropped: 0, errors: 0 } }),
    } as File);
    state.mark("test");
    const imported = await parseTrace(
      serializeSession(state.session.value!).join(""),
      "export",
    );
    expect(imported.rows).toHaveLength(2);
    expect(imported.captureStatus).toEqual(state.session.value?.captureStatus);
    expect(
      captureQuality(
        imported.captureStatus!,
        imported.rows.filter((r) => r.source !== "marker").length,
      ).countMatches,
    ).toBe(true);
  });
});
