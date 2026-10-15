/** @summary Read preview resources exclusively from an explicitly selected installed game. */
import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
/** @summary Diagnostics for optional preview resources, never silent replacements. */
export const localPreviewWarnings = ref<string[]>([]);
/** @summary Immutable installed-game attachment identity. */
export interface LocalAssetStatus { ready: boolean; gameDataPath?: string; generation?: string; assetCount: number; provenance?: string; availableLocales?: string[]; }
let active: LocalAssetStatus | null = null;
const images = new Map<string, Promise<string>>();
/** @summary Retain installed-media provenance across generation changes so display bytes never become authored exports. */
const installedMedia = new Set<string>();
/** @summary Identify image payloads obtained from the installed game, independently of the active generation. */
export function isInstalledPreviewImage(value: unknown): boolean { return typeof value === "string" && installedMedia.has(value); }
let inFlight = 0;
const readers: (() => void)[] = [];
/** @summary Bound concurrent native reads without releasing a slot before its next owner. */
async function boundedRead<T>(command: string, args: Record<string, unknown>): Promise<T> {
  if (inFlight >= 8) await new Promise<void>((resolve) => readers.push(resolve)); else inFlight++;
  try { return await invoke<T>(command, args); }
  finally { const next = readers.shift(); if (next) next(); else inFlight--; }
}
/** @summary Validate backend identity and invalidate resources when the installation changes. */
function attach(status: LocalAssetStatus): LocalAssetStatus {
  if (typeof status?.ready !== "boolean" || (status.ready && (!status.generation || !Number.isInteger(status.assetCount)))) throw new Error("Invalid installed preview identity");
  if (status.generation !== active?.generation) { images.clear(); localPreviewWarnings.value = []; }
  active = status; return status;
}
/** @summary Query the current installation without a pre-extracted cache fallback. */
export async function localAssetStatus(): Promise<LocalAssetStatus> { return attach(await invoke<LocalAssetStatus>("local_game_assets_status")); }
/** @summary Prepare directly from SimCityData; no extraction selector or source import is accepted. */
export async function prepareLocalAssets(gameDataPath: string): Promise<LocalAssetStatus> { return attach(await invoke<LocalAssetStatus>("local_game_assets_prepare", { request: { gameDataPath } })); }
/** @summary Require an immutable generation for every preview read. */
export function localAssetGeneration(): string {
  if (!active?.ready || !active.generation) throw new Error("Local game preview is not configured. Select the installed SimCityData directory first.");
  return active.generation;
}
/** @summary Normalize virtual game paths while rejecting external URLs and traversal. */
export function localResourcePath(reference: string, from = ""): string {
  if (/^(?:[a-z][a-z\d+.-]*:|\/\/)/i.test(reference) || reference.includes("\\") || /[?#\0%]/.test(reference)) throw new Error(`Forbidden preview resource reference: ${reference}`);
  const source = reference.startsWith("/game-ui/") ? reference.slice(9) : reference.startsWith("/") ? "" : from.slice(0, from.lastIndexOf("/") + 1) + reference;
  const parts: string[] = [];
  for (const part of source.split("/")) {
    if (part === "..") { if (!parts.length) throw new Error("Preview resource escapes its virtual root"); parts.pop(); }
    else if (part && part !== ".") parts.push(part);
  }
  if (!parts.length) throw new Error("Empty preview resource path"); return parts.join("/");
}
/** @summary Accept exact typed resource identifiers or bounded virtual image names. */
function imageReference(reference: string): string {
  if (/^installed-game:[\da-f]{8}:[\da-f]{8}:[\da-f]{8}$/i.test(reference)) return reference;
  const name = localResourcePath(reference).split("/").pop()!;
  if (!/^[\w .-]+\.(?:png|jpe?g|gif)$/i.test(name)) throw new Error("Unsupported installed image reference"); return name;
}
/** @summary Read generation-bound images from installed packages, validating MIME and encoding. */
export function localImage(reference: string, generation = localAssetGeneration()): Promise<string> {
  const normalized = imageReference(reference), key = `${generation}:${normalized}`;
  const cached = images.get(key); if (cached) return cached;
  const pending = boundedRead<{ mime: string; base64: string }>("local_game_assets_resource", { reference: normalized, generation }).then(({ mime, base64 }) => {
    if (!/^image\/(?:png|jpeg|gif)$/.test(mime) || !/^[A-Za-z\d+/]*={0,2}$/.test(base64)) throw new Error("Installed resource has invalid media encoding");
    const value = `data:${mime};base64,${base64}`;
    installedMedia.add(value);
    return value;
  }).catch((error) => { images.delete(key); throw error; });
  images.set(key, pending); return pending;
}
/** @summary Replace typed references in JSON without evaluating imported scripts. */
export async function resolveLocalImages<T>(value: T, generation = localAssetGeneration()): Promise<T> {
  if (typeof value === "string" && (value.startsWith("installed-game:") || value.startsWith("/game-ui/"))) return await localImage(value, generation) as T;
  if (Array.isArray(value)) return await Promise.all(value.map((item) => resolveLocalImages(item, generation))) as T;
  if (value && typeof value === "object") return Object.fromEntries(await Promise.all(Object.entries(value).map(async ([key, item]) => {
    if (["preview", "marquee"].includes(key) && typeof item === "string" && item && !item.startsWith("installed-game:") && !item.startsWith("/game-ui/")) throw new Error("Non-local preview media is forbidden");
    if (key === "sourceResource") return [key, item];
    return [key, await resolveLocalImages(item, generation)];
  }))) as T;
  return value;
}
/** @summary Sanitize styles and bind every image URL to installed packages. */
export async function resolveStylesheet(css: string, path: string, generation: string): Promise<string> {
  css = css.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^[ \t]*\\\\border\s*:[^;\r\n]*;[ \t]*$/gm, "");
  if (css.includes("\\") || css.includes("/*") || /@import\b|expression\s*\(|image-set\s*\(|-moz-binding|behavior\s*:/i.test(css)) throw new Error("Active, escaped or imported preview CSS is forbidden");
  const refs = [...css.matchAll(/url\(\s*['"]?([^)'"\s]+)['"]?\s*\)/gi)];
  if ((css.match(/url\s*\(/gi)?.length ?? 0) !== refs.length) throw new Error("Unsupported preview CSS URL syntax");
  const replacements = new Map<string, string>();
  for (const match of refs) {
    const reference = localResourcePath(match[1]!, path);
    let url = replacements.get(reference);
    if (url === undefined) { url = await localImage(reference, generation); replacements.set(reference, url); }
    css = css.replace(match[0], `url("${url}")`);
  }
  return css;
}
/** @summary Reproduce the established HUD generator's gradient conversion, including its keyword-direction convention. */
export function compatibilityHudGradients(css: string): string {
  const start = /-moz-linear-gradient\s*\(/gi;
  let result = "", cursor = 0, match: RegExpExecArray | null;
  while ((match = start.exec(css))) {
    let depth = 1, end = start.lastIndex, comma = -1;
    for (; end < css.length && depth; end++) {
      const char = css[end];
      if (char === "(") depth++;
      else if (char === ")") depth--;
      else if (char === "," && depth === 1 && comma < 0) comma = end;
    }
    if (depth || comma < 0) throw new Error("Malformed installed HUD gradient");
    const direction = css.slice(start.lastIndex, comma).trim().match(/(-?\d+(?:\.\d+)?deg)\s*$/i)?.[1] ?? "180deg";
    const stops = css.slice(comma + 1, end - 1).replace(/\s+/g, " ").trim();
    result += css.slice(cursor, match.index) + `linear-gradient(${direction}, ${stops})`;
    cursor = end; start.lastIndex = end;
  }
  return result + css.slice(cursor);
}
/** @summary Retain flat autogen rules only for explicit active state classes. */
export function activeAutogenStyles(css: string, classes: ReadonlySet<string>): string {
  const plain = css.replace(/\/\*[\s\S]*?\*\//g, "");
  if (plain.includes("@") || /[^{}]*\{[^{}]*\{/.test(plain)) throw new Error("Unsupported autogen CSS structure");
  return [...plain.matchAll(/([^{}]+)\{([^{}]*)\}/g)].filter(([, selector]) => {
    const names = selector!.match(/\.[\w-]+/g) ?? [];
    return names.some((name) => classes.has(name.slice(1)));
  }).map(([rule]) => rule).join("\n");
}
/** @summary Read original CSS by virtual name and filter dormant generated fragments before image reads. */
export async function localStylesheet(name: string, generation = localAssetGeneration(), activeClasses: ReadonlySet<string> = new Set(), displayName = name): Promise<string> {
  const resource = await boundedRead<{ mime: string; base64: string }>("local_game_assets_stylesheet", { name, generation });
  if (resource.mime !== "text/css") throw new Error("Preview stylesheet has an unexpected MIME type");
  let css = new TextDecoder("utf-8", { fatal: true }).decode(Uint8Array.from(atob(resource.base64), (char) => char.charCodeAt(0)));
  if (displayName.startsWith("autogen")) css = activeAutogenStyles(css, activeClasses);
  return resolveStylesheet(css, `replica/css/game/${displayName}`, generation);
}
