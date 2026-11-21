import { readFileSync, mkdirSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { pathToFileURL } from "node:url";
import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { build } = createRequire(require.resolve("vite/package.json"))(
  "esbuild",
);
const root = process.argv[2] ?? "D:/openscp/mods/泡芙顾问有限公司";
function png(path) {
  const bytes = readFileSync(path),
    width = bytes.readUInt32BE(16),
    height = bytes.readUInt32BE(20);
  if (bytes[24] !== 8 || bytes[25] !== 6 || bytes[28] !== 0)
    throw Error("Expected non-interlaced RGBA8 PNG");
  const chunks = [];
  for (let at = 8; at < bytes.length;) {
    const length = bytes.readUInt32BE(at),
      kind = bytes.toString("ascii", at + 4, at + 8);
    if (kind === "IDAT") chunks.push(bytes.subarray(at + 8, at + 8 + length));
    at += length + 12;
  }
  const raw = inflateSync(Buffer.concat(chunks)),
    data = new Uint8ClampedArray(width * height * 4),
    stride = width * 4;
  function paeth(a, b, c) {
    const p = a + b - c,
      pa = Math.abs(p - a),
      pb = Math.abs(p - b),
      pc = Math.abs(p - c);
    return pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
  }
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    for (let x = 0; x < stride; x++) {
      const at = y * stride + x,
        a = x >= 4 ? data[at - 4] : 0,
        b = y ? data[at - stride] : 0,
        c = y && x >= 4 ? data[at - stride - 4] : 0;
      const predictor = [0, a, b, Math.floor((a + b) / 2), paeth(a, b, c)][
        filter
      ];
      if (predictor === undefined) throw Error("Unexpected PNG filter");
      data[at] = (raw[y * (stride + 1) + x + 1] + predictor) & 255;
    }
  }
  return { width, height, data };
}
mkdirSync("tmp/pe-ground", { recursive: true });
await build({
  entryPoints: ["src/render/resolve/groundCompose.ts"],
  bundle: true,
  platform: "node",
  format: "esm",
  outfile: "tmp/pe-ground/compose.mjs",
});
const core = await import(
  pathToFileURL(process.cwd() + "/tmp/pe-ground/compose.mjs")
);
const lot = JSON.parse(readFileSync(root + "/asset.lot.json", "utf8")).lot;
const mask = png(root + "/assets/lot/mask.png"),
  surface = png(root + "/assets/lot/surface.png"),
  normalAtlas = png(root + "/assets/lot/normal.png");
const input = {
  mask,
  rawMask: {
    ...mask,
    data: new Uint8ClampedArray(readFileSync(root + "/assets/lot/mask.rgba")),
  },
  baseTile: core.copyAtlasRegion(
    surface,
    lot.baseTile,
    surface.width / 4,
    surface.height / 4,
  ),
  lotColors: lot.colors.map((c) => c.rgba),
  lotBorderColors: lot.borderColors,
  lotBorderPatternIndices: lot.borderPatterns,
  lotBorderWidths: lot.borderWidths,
  lotColorHeights: lot.colorHeights,
  lotBorderHeights: lot.borderHeights,
  normalAtlas,
  baseTileIndex: lot.baseTile,
  tilesX: lot.size[0] / lot.tilePeriod[0],
  tilesY: lot.size[1] / lot.tilePeriod[1],
  enhanceTintDetail: true,
};
input.outSize = core.groundOutputSize(
  mask.width,
  mask.height,
  input.tilesX,
  input.tilesY,
  normalAtlas.width / 4,
);
const start = performance.now(),
  baseline = core.composeGroundPreview(input),
  elapsed = performance.now() - start;
console.log(JSON.stringify({ root, size: input.outSize, baselineMs: elapsed }));
if (core.composeGroundPreviewFast) {
  const start = performance.now(),
    result = core.composeGroundPreviewFast(input),
    elapsed = performance.now() - start;
  console.log(
    JSON.stringify({
      fastMs: elapsed,
      albedoIdentical: Buffer.from(result.albedo).equals(
        Buffer.from(baseline.albedo),
      ),
      normalIdentical: Buffer.from(result.normal).equals(
        Buffer.from(baseline.normal),
      ),
    }),
  );
  if (!Buffer.from(result.albedo).equals(Buffer.from(baseline.albedo)) ||
      !Buffer.from(result.normal).equals(Buffer.from(baseline.normal))) {
    throw new Error("Accelerated composition differs from the reference pixels");
  }
  const stripeStart = performance.now();
  const parts = Array.from({ length: 4 }, (_, i) =>
    core.composeGroundPreviewFast(input, [
      Math.floor(i * input.outSize.height / 4),
      Math.floor((i + 1) * input.outSize.height / 4),
    ]),
  );
  const joinedAlbedo = Buffer.concat(parts.map(p => Buffer.from(p.albedo)));
  const joinedNormal = Buffer.concat(parts.map(p => Buffer.from(p.normal)));
  const albedoIdentical = joinedAlbedo.equals(Buffer.from(baseline.albedo));
  const normalIdentical = joinedNormal.equals(Buffer.from(baseline.normal));
  console.log(JSON.stringify({ stripedSequentialMs: performance.now() - stripeStart, albedoIdentical, normalIdentical }));
  if (!albedoIdentical || !normalIdentical)
    throw new Error("Striped composition differs from the reference pixels");
}
