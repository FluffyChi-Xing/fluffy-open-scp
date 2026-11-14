import {
  computed,
  onActivated,
  onBeforeUnmount,
  onDeactivated,
  onMounted,
  shallowRef,
} from "vue";
import { tauriApi } from "@/api";
import {
  runtimeObserver,
  type GameProcess,
  type ObserverStatus,
} from "@/api/runtimeObserver";
import { command } from "@/api/tauri";

function errorCode(cause: unknown) {
  if (cause && typeof cause === "object" && "message" in cause) {
    try {
      return JSON.parse(String(cause.message)).code ?? "failed";
    } catch {
      if ("code" in cause && cause.code !== "command_failed")
        return String(cause.code);
      return String(cause.message);
    }
  }
  return "failed";
}

export function useAssetCapture(project: string) {
  const processes = shallowRef<GameProcess[]>([]);
  const builds = shallowRef<{ path: string; ids: number[] }[]>([]);
  const targetKey = shallowRef("");
  const pid = shallowRef<number>();
  const status = shallowRef<ObserverStatus>();
  const busy = shallowRef(false);
  const loading = shallowRef(false);
  const error = shallowRef("");
  const epoch = shallowRef(0);
  const targets = computed(() =>
    builds.value.flatMap(({ path, ids }) =>
      ids.map((iid) => ({
        key: `${path}/${iid}`,
        iid,
        label: `${path.split("/").slice(-3).join("/")} · 0x${iid.toString(16).toUpperCase().padStart(8, "0")}`,
      })),
    ),
  );
  let active = false;
  let directory: string | undefined;
  let version = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function scan() {
    if (loading.value) return;
    loading.value = true;
    error.value = "";
    try {
      const [games, summary] = await Promise.all([
        runtimeObserver.listProcesses(),
        tauriApi.studio.projects.repoSummary(project),
      ]);
      const results = await Promise.allSettled(
        summary.releases.slice(0, 8).map(async ({ relativePath }) => {
          const ids = await command<number[]>("code_package_observable_units", {
            request: { project, relativePath },
          });
          return { path: relativePath, ids };
        }),
      );
      if (!active) return;
      const found = results.flatMap((result) =>
        result.status === "fulfilled" ? [result.value] : [],
      );
      const failed = results.find((result) => result.status === "rejected");
      if (failed?.status === "rejected") error.value = errorCode(failed.reason);
      processes.value = games;
      builds.value = found;
      if (!targets.value.some((target) => target.key === targetKey.value))
        targetKey.value = targets.value[0]?.key ?? "";
      if (!games.some((game) => game.pid === pid.value))
        pid.value = games[0]?.pid;
    } catch (cause) {
      if (active) error.value = errorCode(cause);
    } finally {
      loading.value = false;
    }
  }

  async function poll(session: string, run: number) {
    try {
      const next = await command<ObserverStatus>("observer_status", {
        assetDirectory: session,
      });
      if (!active || run !== version) return;
      if (next.directory !== session) {
        directory = undefined;
        if (status.value) status.value = { ...status.value, running: false };
        return;
      }
      status.value = next;
      if (next.running) timer = setTimeout(() => void poll(session, run), 750);
      else {
        await stop();
      }
    } catch (cause) {
      if (active && run === version) {
        error.value = errorCode(cause);
        timer = setTimeout(() => void poll(session, run), 1000);
      }
    }
  }

  async function start() {
    const target = targets.value.find((entry) => entry.key === targetKey.value);
    if (!pid.value || !target || busy.value || directory) return;
    busy.value = true;
    error.value = "";
    try {
      const next = await command<ObserverStatus>("observer_start", {
        pid: pid.value,
        seconds: 300,
        unitEntry: target.iid,
      });
      if (!next.directory) throw new Error("observer_session_missing");
      if (!active) {
        await command("observer_stop", { assetDirectory: next.directory });
        return;
      }
      directory = next.directory;
      epoch.value += 1;
      status.value = next;
      const run = ++version;
      const session = directory;
      timer = setTimeout(() => void poll(session, run), 750);
    } catch (cause) {
      if (active) error.value = errorCode(cause);
    } finally {
      busy.value = false;
    }
  }

  async function stop() {
    if (!directory || busy.value) return;
    busy.value = true;
    clearTimeout(timer);
    const run = ++version;
    const session = directory;
    try {
      const next = await command<ObserverStatus>("observer_stop", {
        assetDirectory: session,
      });
      directory = undefined;
      status.value = next;
    } catch (cause) {
      if (active) {
        error.value = errorCode(cause);
        timer = setTimeout(() => void poll(session, run), 1000);
      }
    } finally {
      busy.value = false;
    }
  }

  function activate() {
    if (active) return;
    active = true;
    void scan();
  }
  function release() {
    if (!active) return;
    active = false;
    void stop();
  }
  onMounted(activate);
  onActivated(activate);
  onDeactivated(release);
  onBeforeUnmount(release);

  return {
    processes,
    targets,
    targetKey,
    pid,
    status,
    busy,
    loading,
    error,
    epoch,
    scan,
    start,
    stop,
  };
}
