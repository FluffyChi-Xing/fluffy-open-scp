import { runGroundComposeWorker } from "./groundCompose";

/** 精细地面合成 Worker 入口（Vite module worker）。 */
runGroundComposeWorker(
  self as unknown as Parameters<typeof runGroundComposeWorker>[0],
);
