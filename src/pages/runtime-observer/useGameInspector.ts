import { onBeforeUnmount, onDeactivated, ref, watch, type Ref } from "vue";
import { command, subscribe } from "@/api/tauri";
import type { DomFrame, DomNode } from "./elements";
interface InspectorEvent {
  session: string;
  kind: string;
  pid: number;
  node?: number;
  sequence?: number;
  time?: number;
  message?: string;
}
export function useGameInspector(
  pid: () => number | undefined,
  frame: Ref<DomFrame | null>,
  onSnapshot: () => Promise<void>,
  onSelect: (id: number) => void,
) {
  const picking = ref(false);
  const connected = ref(false),
    connecting = ref(false),
    error = ref("");
  let generation = 0,
    unlisten: (() => void) | undefined;
  let attached: number | undefined;
  async function disconnect() {
    generation++;
    connected.value = false;
    picking.value = false;
    connecting.value = false;
    unlisten?.();
    unlisten = undefined;
    const old = attached;
    attached = undefined;
    if (old) await command("devtool_disconnect", { pid: old }).catch(() => {});
  }
  async function connect() {
    await disconnect();
    const target = pid();
    if (!target) return;
    const ticket = ++generation;
    const session = crypto.randomUUID();
    connecting.value = true;
    error.value = "";
    try {
      const stop = await subscribe<InspectorEvent>("devtool-event", (e) => {
        if (ticket !== generation || e.pid !== target || e.session !== session)
          return;
        if (e.kind === "closed") {
          connected.value = false;
          connecting.value = false;
          picking.value = false;
        } else if (e.kind === "ready") {
          connected.value = true;
          connecting.value = false;
        } else if (e.kind === "picking") picking.value = true;
        else if (e.kind === "cancelled" || e.kind === "cleared")
          picking.value = false;
        else if (e.kind === "snapshot") {
          error.value = "";
          picking.value = false;
          void onSnapshot();
        } else if (
          e.kind === "selected" &&
          e.node !== undefined &&
          e.sequence === frame.value?.sequence &&
          e.time === frame.value?.time
        ) {
          picking.value = false;
          onSelect(e.node);
        } else if (
          ["error", "read-error", "command-error", "stale"].includes(e.kind)
        ) {
          error.value = e.message ?? e.kind;
          if (e.kind === "error") {
            connected.value = false;
            connecting.value = false;
          }
        }
      });
      if (ticket !== generation) {
        stop();
        return;
      }
      unlisten = stop;
      attached = target;
      await command("devtool_connect", { pid: target, session });
    } catch (e) {
      if (ticket === generation) {
        const message = String((e as { message?: string }).message ?? e);
        await disconnect();
        error.value = message;
      }
    }
  }
  async function control(
    action: "highlight" | "clear" | "pick",
    node?: DomNode,
  ) {
    if (!connected.value || !attached) return;
    try {
      await command("devtool_control", {
        pid: attached,
        action,
        node: node?.i ?? null,
        sequence: frame.value?.sequence ?? null,
        time: frame.value?.time ?? null,
      });
    } catch (e) {
      error.value = String((e as { message?: string }).message ?? e);
    }
  }
  watch(pid, () => {
    void disconnect();
    error.value = "";
  });
  onDeactivated(() => {
    void disconnect();
  });
  onBeforeUnmount(() => {
    void disconnect();
  });
  return {
    picking,
    connected,
    connecting,
    error,
    connect,
    disconnect,
    control,
  };
}
