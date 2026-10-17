import { analyze, type DiagnosticSession, type Raw } from "./diagnostics";

export class DeltaImportError extends Error {
  constructor(
    public code: "deltaVersion" | "deltaInvalid",
    public offset: number,
    public header?: string,
  ) {
    super(code);
  }
}

/** Retail v4 envelope only. The payload is NOT an EncodeActionHeader stream. */
export async function parseDelta(
  data: ArrayBuffer,
  name: string,
): Promise<DiagnosticSession> {
  const view = new DataView(data);
  if (view.byteLength < 4) throw new DeltaImportError("deltaInvalid", 0);
  if (view.getUint32(0, true) !== 4)
    throw new DeltaImportError(
      "deltaVersion",
      0,
      Array.from(new Uint8Array(data, 0, 4), (n) =>
        n.toString(16).padStart(2, "0"),
      ).join(" "),
    );
  const sha256 = Array.from(
    new Uint8Array(await crypto.subtle.digest("SHA-256", data)),
    (n) => n.toString(16).padStart(2, "0"),
  ).join("");
  const rows: Raw[] = [];
  let offset = 4;
  let previous: bigint | undefined;
  while (offset < data.byteLength) {
    if (offset + 24 > data.byteLength || view.getUint32(offset) !== 0xff020300)
      throw new DeltaImportError("deltaInvalid", offset);
    const size = view.getUint32(offset + 4);
    if (size < 24 || offset + size > data.byteLength)
      throw new DeltaImportError("deltaInvalid", offset);
    const deltaId = view.getBigUint64(offset + 8);
    rows.push({
      channel: "delta.envelope",
      format: "retail-sldelta-v4",
      provenance: { file: name, sha256 },
      offset,
      size,
      deltaId: deltaId.toString(),
      idRegressed: previous !== undefined && deltaId < previous,
      previousDeltaId: previous?.toString(),
      simTicks: view.getUint32(offset + 16),
      kind: view.getUint32(offset + 20),
      payloadBytes: size - 24,
      payloadDecoded: false,
    });
    previous = deltaId;
    offset += size;
    if (rows.length % 5000 === 0)
      await new Promise((resolve) => setTimeout(resolve, 0));
  }
  return analyze(rows, name);
}
