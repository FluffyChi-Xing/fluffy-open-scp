/**
 * 叶卡/植被类贴图的 alpha 裁剪（cutout）。
 *
 * 背景：树等植被 prop 的漫反射图集背景像素是「白色 + alpha 0」，引擎用
 * alpha>0.5 clip 抠掉。工作台的树渲染器（propModels getTreeModelObject）
 * 自带 alphaTest，但打包后 lot 的直挂树模型 PROP_ID 不在脚本树资源集
 * （treeIds）里，落进通用 prop 材质路线——该路线此前不开 alphaTest，
 * 透明背景被画成白卡片（2026-10-30 用户实测）。
 *
 * 策略：对已加载的漫反射贴图做一次小尺寸 alpha 采样，存在低于阈值的
 * 像素才开 alphaTest——纯不透明贴图（建筑、车辆漆面）外观零变化，
 * 引擎同款 0.5 阈值语义（tint.A>0.5 clip）。
 */

const cutoutScanCache = new WeakMap<object, boolean>();

/** 结构化最小贴图接口（THREE.Texture 满足；测试可用假对象）。 */
export interface CutoutTexture {
  image?: unknown;
}

/** alpha 通道中是否存在需要裁剪的像素（任一 < threshold）。 */
export function hasTransparentPixels(
  alpha: ArrayLike<number>,
  threshold = 128,
): boolean {
  for (let index = 0; index < alpha.length; index += 1) {
    if (alpha[index] < threshold) return true;
  }
  return false;
}

/** 降采样读取贴图 alpha 通道；canvas 不可用（测试环境/跨域）时返回 false。 */
function scanTextureAlpha(texture: CutoutTexture, threshold = 128): boolean {
  const image = texture.image as
    | (CanvasImageSource & { width?: number; height?: number })
    | undefined;
  if (!image || !image.width || !image.height) return false;
  const canvas = document.createElement("canvas");
  const width = Math.min(64, image.width);
  const height = Math.min(64, image.height);
  if (width < 1 || height < 1) return false;
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) return false;
  try {
    context.drawImage(image, 0, 0, width, height);
    const { data } = context.getImageData(0, 0, width, height);
    const alpha = new Array<number>(width * height);
    for (let pixel = 0; pixel < width * height; pixel += 1) {
      alpha[pixel] = data[pixel * 4 + 3];
    }
    return hasTransparentPixels(alpha, threshold);
  } catch {
    return false;
  }
}

/**
 * 贴图含透明像素时给材质开 alphaTest 0.5（引擎 clip 同阈值）。
 * 扫描结果按贴图缓存（WeakMap，纹理释放后随 GC 回收）。
 * 返回是否判定为 cutout 贴图。
 */
export function applyAlphaCutout(
  material: { alphaTest: number; needsUpdate: boolean },
  texture: CutoutTexture | null | undefined,
  scan: (texture: CutoutTexture) => boolean = scanTextureAlpha,
): boolean {
  if (!texture) return false;
  let cutout = cutoutScanCache.get(texture);
  if (cutout === undefined) {
    cutout = scan(texture);
    cutoutScanCache.set(texture, cutout);
  }
  if (cutout) {
    material.alphaTest = 0.5;
    material.needsUpdate = true;
  }
  return cutout;
}
