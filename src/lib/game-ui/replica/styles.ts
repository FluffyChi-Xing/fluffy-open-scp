/** @summary Load verified local CSS into Shadow DOM without network or arbitrary file access. */
import { compatibilityHudGradients, localAssetGeneration, localStylesheet, resolveStylesheet } from "../local-assets";
import replica from "./authored/replica.css?raw";
import tweaks from "./authored/tweaks.css?raw";
import compatibility from "./authored/compatibility.css?raw";

/** @summary Original authoring stylesheet order, using verified installed type/group/instance identities rather than guessed header names. */
export const COMPATIBILITY_BASE_STYLE_IDS = ["92319616", "40969046", "8145AF53", "D743498E", "0292AFE2", "14D97BD3", "06DEC864", "CD6DD755"] as const;
/** @summary Preserve the generator's concatenation order before its authored reconstruction rules. */
export const COMPATIBILITY_HUD_STYLE_IDS = ["92319616", "40969046", "24E582BA", "7F44F9EE", "8145AF53", "06DEC864", "CD6DD755", "0292AFE2", "D743498E", "14D97BD3"] as const;

/** @summary Replace a stage's complete style set only after every required stylesheet loads. */
export async function injectReplicaStyles(root: ShadowRoot, generation = localAssetGeneration(), data?: unknown): Promise<void> {
  const active = new Set<string>();
  /** @summary Gather only explicit native drawable state classes, including nine-slice states. */
  function visit(value: unknown): void {
    if (Array.isArray(value)) { for (const item of value) visit(item); return; }
    if (!value || typeof value !== "object") return;
    for (const [key, item] of Object.entries(value)) {
      if (["cssStyles", "styles", "nineSliceStyles", "requiredStyleClasses"].includes(key) && Array.isArray(item)) for (const name of item) if (typeof name === "string") active.add(name);
      visit(item);
    }
  }
  visit(data);
  const stylesheets = (data as { stylesheets?: { name: string; reference: string }[] } | undefined)?.stylesheets;
  if (!stylesheets?.length || stylesheets.some((sheet) => typeof sheet.name !== "string" || !/^installed-game:2c978db6:[\da-f]{8}:[\da-f]{8}$/i.test(sheet.reference))) throw new Error("Installed preview stylesheet identities are missing or invalid");
  const reads = new Map<string, Promise<string>>();
  /** @summary Fail explicitly when an original authoring dependency cannot be read from this installation. */
  function installed(id: string): Promise<string> {
    const reference = `installed-game:2C978DB6:00000000:${id}`;
    const sheet = stylesheets!.find((item) => item.reference.toUpperCase() === reference.toUpperCase());
    if (!sheet) throw new Error(`Required authoring stylesheet is unavailable: ${reference}`);
    let pending = reads.get(id);
    if (!pending) { pending = localStylesheet(sheet.reference, generation, active, sheet.name); reads.set(id, pending); }
    return pending;
  }
  const base = await Promise.all(COMPATIBILITY_BASE_STYLE_IDS.map(installed));
  const hud = compatibilityHudGradients((await Promise.all(COMPATIBILITY_HUD_STYLE_IDS.map(installed))).join("\n"));
  const sheets = [...base, hud, await resolveStylesheet(compatibility, "replica/css/compatibility.css", generation), await resolveStylesheet(replica, "replica/css/replica.css", generation), await resolveStylesheet(tweaks, "replica/css/tweaks.css", generation)];
  root.querySelectorAll("style[data-local-game-style]").forEach((node) => node.remove());
  for (const css of sheets) {
    const style = document.createElement("style");
    style.dataset.localGameStyle = generation;
    style.textContent = css;
    root.appendChild(style);
  }
}

