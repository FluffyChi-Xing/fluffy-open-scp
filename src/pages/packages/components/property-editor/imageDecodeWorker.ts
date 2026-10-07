import { runImageDecodeWorker } from "./imageDecode";

/** 图像像素解码 Worker 入口（Vite module worker）。 */
runImageDecodeWorker(
  self as unknown as Parameters<typeof runImageDecodeWorker>[0],
);
