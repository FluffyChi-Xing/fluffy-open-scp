import type * as ThreeNamespace from "three";

/**
 * PNG → 像素的 Worker 解码调度（imageDecodeWorker.ts 的编排侧）：
 * createImageBitmap + OffscreenCanvas 全程在 Worker 内，主线程只收
 * transferable 的像素 buffer——替换此前 2048² 级图集在主线程
 * drawImage + getImageData 的一次大块同步占用。
 *
 * 语义与主线程 Image→canvas→getImageData 对齐（同一浏览器解码管线）；
 * 首个接入点 = 地表共享图集（不透明 albedo tile，无预乘歧义）。
 * 失败回退主线程同款实现，调用方无感。
 */

export interface Pixels {
  /** ArrayBuffer 背书（getImageData 产物；transferable）。 */
  data: Uint8ClampedArray<ArrayBuffer>;
  width: number;
  height: number;
}

export interface ImageDecodeRequest {
  id: number;
  /** PNG/dataURL 的可 fetch 地址。 */
  url: string;
}

export interface ImageDecodeResponse {
  id: number;
  pixels?: Pixels;
  error?: string;
}

interface DecodeWorkerScope {
  postMessage(message: ImageDecodeResponse, transfer?: Transferable[]): void;
  onmessage: ((event: MessageEvent<ImageDecodeRequest>) => void) | null;
}

/** Worker 内执行：fetch → createImageBitmap → OffscreenCanvas → 像素。 */
export async function decodeImagePixels(
  url: string,
): Promise<Pixels> {
  const response = await fetch(url);
  const blob = await response.blob();
  const bitmap = await createImageBitmap(blob);
  const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) throw new Error("offscreen canvas 2d unavailable");
  context.drawImage(bitmap, 0, 0);
  const imageData = context.getImageData(0, 0, bitmap.width, bitmap.height);
  bitmap.close();
  return {
    data: imageData.data,
    width: imageData.width,
    height: imageData.height,
  };
}

export function runImageDecodeWorker(scope: DecodeWorkerScope): void {
  scope.onmessage = (event: MessageEvent<ImageDecodeRequest>) => {
    const { id, url } = event.data;
    void decodeImagePixels(url)
      .then((pixels) => {
        scope.postMessage({ id, pixels }, [pixels.data.buffer]);
      })
      .catch((error: unknown) => {
        scope.postMessage({
          id,
          error: error instanceof Error ? error.message : String(error),
        });
      });
  };
}
