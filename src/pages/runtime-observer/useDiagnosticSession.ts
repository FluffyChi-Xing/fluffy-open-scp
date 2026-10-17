import { shallowRef } from "vue";
import {
  analyze,
  parseTrace,
  serializeSession,
  type DiagnosticSession,
} from "./diagnostics";
import { DeltaImportError, parseDelta } from "./deltaEnvelope";

export function useDiagnosticSession() {
  const session = shallowRef<DiagnosticSession>();
  const busy = shallowRef(false);
  const error = shallowRef("");
  const errorOffset = shallowRef<number>();
  const errorFile = shallowRef("");
  const errorHeader = shallowRef("");
  async function importFile(file: File) {
    if (busy.value) return;
    error.value = "";
    errorFile.value = file.name;
    errorHeader.value = "";
    errorOffset.value = undefined;
    if (file.size > 256 * 1024 * 1024) {
      error.value = "limit";
      return;
    }
    busy.value = true;
    try {
      session.value = /\.mfs$/i.test(file.name)
        ? await parseDelta(await file.arrayBuffer(), file.name)
        : await parseTrace(await file.text(), file.name);
    } catch (e) {
      error.value = e instanceof DeltaImportError ? e.code : "failed";
      errorOffset.value = e instanceof DeltaImportError ? e.offset : undefined;
      errorHeader.value = e instanceof DeltaImportError ? (e.header ?? "") : "";
    } finally {
      busy.value = false;
    }
  }
  async function importStatus(file: File) {
    if (!session.value || busy.value) return;
    const current = session.value;
    error.value = "";
    errorFile.value = file.name;
    errorHeader.value = "";
    errorOffset.value = undefined;
    busy.value = true;
    try {
      if (file.size > 4 * 1024 * 1024) throw new Error();
      const status: unknown = JSON.parse(await file.text());
      if (!status || typeof status !== "object" || Array.isArray(status))
        throw new Error();
      session.value = {
        ...current,
        captureStatus: status as Record<string, unknown>,
      };
    } catch {
      error.value = "statusFailed";
    } finally {
      busy.value = false;
    }
  }
  function mark(message: string) {
    if (busy.value || !session.value || !message.trim()) return;
    const current = session.value;
    session.value = analyze(
      [
        ...current.rows.map((r) => r.raw),
        {
          channel: "host.marker",
          time: Date.now(),
          message: message.trim(),
          afterSequence: current.rows.at(-1)?.raw.sequence,
        },
      ],
      current.name,
      current.invalid,
    );
    session.value.captureStatus = current.captureStatus;
  }
  function download() {
    if (!session.value) return;
    const blob = new Blob(serializeSession(session.value), {
      type: "application/x-ndjson",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "diagnostic-session.jsonl";
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  return {
    session,
    busy,
    error,
    errorOffset,
    errorFile,
    errorHeader,
    importFile,
    importStatus,
    mark,
    download,
  };
}
