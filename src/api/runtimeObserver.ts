import { command } from "./tauri";

export interface GameProcess { pid: number; name: string; memoryKb?: number }
export interface ObserverStatus { running: boolean; pid?: number; directory?: string; records: number; synced: number; status?: Record<string, unknown>; recent: Record<string, unknown>[]; eventCounts: Record<string, number>; gameTimeSeconds?: number; gameClockSeconds?: number; gameClockSource?: string; gamePaused?: boolean }
export interface GameFrame { mime: string; dataBase64: string; capturedAt: number; width?: number; height?: number }

export const runtimeObserver = {
  listProcesses: () => command<GameProcess[]>("observer_list_processes"),
  start: (pid: number, seconds = 60, profile = "balanced", observation = "ui:all") => command<ObserverStatus>("observer_start", { pid, seconds, profile, observation }),
  stop: () => command<ObserverStatus>("observer_stop"),
  status: () => command<ObserverStatus>("observer_status"),
  sync: () => command<number>("observer_sync"),
  frame: (pid: number) => command<GameFrame>("observer_frame", { pid }),
};
