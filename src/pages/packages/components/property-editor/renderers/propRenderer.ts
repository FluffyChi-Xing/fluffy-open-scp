import type * as ThreeNamespace from "three";
import type { LotUnitDto } from "@/api/tauri";
import { pngBlobUrl } from "@/lib/three-gltf";
import { unitMatrix } from "../unitGizmos";
import { getPropModelObject } from "../propModels";
import type { UnitRenderContext, UnitRenderer } from "./types";

/**
 * prop 渲染器（车辆/杂件真实模型，P2 精细替换）：脚本资源表反查的
 * LOTM 载荷装配（车漆 lerp / tint 调色链在 getPropModelObject 内）。
 *
 * ⚠ 已知开放问题（隔离在本渲染器）：消防车等车型轮胎被车漆染红——
 * 引擎口径 diffuse.a≈0 区域吃 paintColor，该车型的轮胎纹理区 alpha
 * 语义与此不符；修复时只动本文件/getPropModelObject，勿涉他族渲染器。
 * 仅精细模式且 resourceID 有载荷时适用。
 */
export const renderPropUnit: UnitRenderer = async (
  ctx: UnitRenderContext,
  unit: LotUnitDto,
): Promise<ThreeNamespace.Object3D | null> => {
  if (ctx.renderMode !== "refined" || unit.kind !== "prop") return null;
  const payload =
    typeof unit.resourceId === "number"
      ? (ctx.propModels.get(unit.resourceId) ??
        ctx.addedModelPayloads?.get(unit.resourceId))
      : undefined;
  if (!payload) return null;
  const propModel = await getPropModelObject(
    ctx.THREE,
    payload,
    ctx.envRefs ?? undefined,
    unit.index,
  );
  if (!propModel) return null;
  if (unit.transform) {
    unitMatrix(ctx.THREE, unit.transform).decompose(
      propModel.position,
      propModel.quaternion,
      propModel.scale,
    );
  }
  // flags==15 hack：Scale 存于 Transform.Unknown（DTO 独立 scale 字段，
  // 半宽语义）——锥体可忽略，真模型必须叠加，否则广告牌等超出构架
  if (typeof unit.scale === "number") {
    propModel.scale.multiplyScalar(unit.scale);
  }
  // 放置的直挂模型（树部件家族实测，props 文档 §3.1）：纹理槽位语义与
  // 杂件相反——tex[0]=法线/占位（乳白）、tex[1]=漫反射图集（叶/皮彩色）。
  // 漫反射按 PNG 字节数取大者（彩色图集压缩后恒大于法线小图），叶卡
  // alphaTest 抠透。
  // 高度归一 7.5m：lot 数据对拍（消防局 0xE917279C）树道具 scale 字段
  // 0.06-0.30 两组（≈3m/9-15m，即用户目测“5-10m 两级”），7.5m 取两级
  // 中值；缩放手柄可调。
  if (unit.modelTgi) {
    const materialSet = payload.materials?.[0];
    const diffuse = [materialSet?.normalPng, materialSet?.slot0Png]
      .filter((bytes): bytes is Uint8Array<ArrayBuffer> => !!bytes)
      .sort((a, b) => b.length - a.length)[0] ?? null;
    if (diffuse) {
      const texture = await new Promise<ThreeNamespace.Texture | null>((resolve) => {
        new ctx.THREE.TextureLoader().load(
          pngBlobUrl(diffuse),
          (loaded) => {
            loaded.colorSpace = ctx.THREE.SRGBColorSpace;
            loaded.flipY = true;
            loaded.anisotropy = 4;
            resolve(loaded);
          },
          undefined,
          () => resolve(null),
        );
      });
      if (texture) {
        propModel.traverse((child) => {
          const mesh = child as ThreeNamespace.Mesh;
          if (!mesh.isMesh) return;
          const materials = Array.isArray(mesh.material)
            ? mesh.material
            : [mesh.material];
          for (const material of materials) {
            const standard = material as ThreeNamespace.MeshStandardMaterial;
            standard.map = texture;
            standard.alphaTest = 0.5;
            standard.needsUpdate = true;
          }
        });
      }
    }
    const bounds = new ctx.THREE.Box3().setFromObject(propModel);
    const height = Math.max(bounds.max.z - bounds.min.z, 0.01);
    propModel.scale.multiplyScalar(7.5 / height);
  }
  return propModel;
};
