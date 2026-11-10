import { tauriApi } from "@/api";
import { command } from "@/api/tauri";
import {
  AUTHORED_LOT_MASK,
  AUTHORED_LOT_META,
  maskMatchesMeta,
  parseAuthoredLotMeta,
  type AuthoredLot,
} from "@/lib/lot-ground/authored";

/** 读取项目内自绘 Lot 地面；缺文件或内容损坏返回 null。 */
export async function loadAuthoredLot(project: string): Promise<AuthoredLot | null> {
  let meta;
  try {
    meta = parseAuthoredLotMeta(
      (await tauriApi.workspace.codeReadText(project, AUTHORED_LOT_META)).content,
    );
  } catch {
    return null;
  }
  if (!meta) return null;
  try {
    const value = (await command("code_flow", {
      request: { project, action: "read-asset", payload: { asset: AUTHORED_LOT_MASK } },
    })) as { base64?: string };
    return value.base64 && maskMatchesMeta(meta, value.base64)
      ? { meta, mask: value.base64 }
      : null;
  } catch {
    return null;
  }
}

/** 写入自绘 Lot 地面：先写 mask（write-asset 会建目录），再按修订号写 ground.json。 */
export async function saveAuthoredLot(project: string, lot: AuthoredLot): Promise<void> {
  await command("code_flow", {
    request: {
      project,
      action: "write-asset",
      payload: { asset: AUTHORED_LOT_MASK, data: lot.mask },
    },
  });
  let revision: string | undefined;
  try {
    revision = (await tauriApi.workspace.codeReadText(project, AUTHORED_LOT_META)).revision;
  } catch {
    revision = undefined;
  }
  await tauriApi.workspace.codeWriteText(
    project,
    AUTHORED_LOT_META,
    JSON.stringify(lot.meta, null, 2),
    revision,
  );
}
