/** @summary Load generation-bound local preview data without executing imported game scripts. */
import type { ReplicaData } from "./types";
import { invoke } from "@tauri-apps/api/core";
import { localAssetStatus, resolveLocalImages } from "../local-assets";

export type GameTextLocale = "en-US" | "pl-PL" | "zh-CN" | "zh-TW";
const localizedData = new Map<string, Promise<ReplicaData>>();

/** @summary Preserve the original authoring corpus language while keeping installed locale identities distinct. */
export function gameTextLocale(language = "zh-TW"): GameTextLocale {
  const normalized = language.toLowerCase();
  if (normalized.startsWith("pl")) return "pl-PL";
  if (normalized.startsWith("zh-tw") || normalized.startsWith("zh-hant")) return "zh-TW";
  if (normalized.startsWith("zh")) return "zh-CN";
  return "en-US";
}

/** @summary Choose an installed language explicitly; callers must display a notice when it differs. */
export function installedGameLanguage(preferred: GameTextLocale, available: readonly string[]): GameTextLocale {
  const supported = (["en-US", "pl-PL", "zh-CN", "zh-TW"] as const).filter((locale) => available.includes(locale));
  if (supported.includes(preferred)) return preferred;
  if (!supported.length) throw new Error("No supported game-text language is installed");
  return supported[0]!;
}

/** @summary Read a complete replica from one validated attachment, with retry after errors. */
export async function loadReplicaData(language = "zh-TW"): Promise<ReplicaData> {
  const status = await localAssetStatus();
  if (!status.ready || !status.generation) throw new Error("Local game preview is not configured. Select the installed SimCityData directory first.");
  const generation = status.generation;
  const locale = gameTextLocale(language);
  const key = `${generation}:${locale}`;
  const cached = localizedData.get(key);
  if (cached) return cached;
  const pending = (async () => {
    const data = await invoke<ReplicaData>("local_game_assets_model", { locale, generation });
    if (data.previewGeneration !== generation) throw new Error("Installed model generation changed during preview loading");
    const { layout, assets, locale: text, model, tools, palette } = data;
    if (!layout || !model?.categories || !palette?.byCategory || !tools || !assets) throw new Error("Incomplete local replica data");
    if (Object.values(assets).some((path) => typeof path !== "string" || !/^installed-game:[\da-f]{8}:[\da-f]{8}:[\da-f]{8}$/i.test(path))) throw new Error("Non-local asset map is forbidden");
    for (const category of model.categories) {
      const icon = category.icon;
      if (icon != null && (typeof icon !== "string" || !/^installed-game:[\da-f]{8}:[\da-f]{8}:[\da-f]{8}$/i.test(icon))) throw new Error("Non-local installed category icon is forbidden");
    }
    const resolved = await resolveLocalImages({ layout, assets, locale: text, model, tools, palette, previewGeneration: generation }, generation);
    const finalStatus = await localAssetStatus();
    if (!finalStatus.ready || finalStatus.generation !== generation) throw new Error("Installed model generation changed during preview loading");
    return Object.assign(resolved, { stylesheets: data.stylesheets, requiredStyleClasses: data.requiredStyleClasses, originalDrawableTypeCounts: data.originalDrawableTypeCounts, diagnostics: data.diagnostics, resolvedLocale: data.resolvedLocale });
  })().catch((cause) => { localizedData.delete(key); throw cause; });
  localizedData.set(key, pending);
  return pending;
}

