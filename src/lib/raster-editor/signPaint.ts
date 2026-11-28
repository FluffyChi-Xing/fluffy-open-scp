import type { RasterDocument } from "./document";
import { layoutMultilineLettering } from "./multilineLettering";

export interface SignSettings {
  effect: "steady" | "marquee";
  speed: number;
  axis: "horizontal" | "vertical";
}

export const defaultSignSettings: Readonly<SignSettings> = {
  effect: "steady",
  speed: 1,
  axis: "horizontal",
};

export const signPalette = ["#ffe7a6", "#ff737d", "#74e6dc", "#a9b9ff"];

export function isSignSettings(value: SignSettings) {
  return (
    (value?.effect === "steady" || value?.effect === "marquee") &&
    (value.axis === "horizontal" || value.axis === "vertical") &&
    Number.isFinite(value.speed) && value.speed >= 0.1 && value.speed <= 2
  );
}

// decalLightBackground: sign chooses the UV axis, integer part counts
// chunks, and fractional part gives each light layer its phase.
export function signAnimationParameter(axis: SignSettings["axis"], layer: number) {
  return (axis === "horizontal" ? 1 : -1) * (60 + layer * 0.3);
}

export function signDisplayPixels(pixels: Uint8ClampedArray, colors: readonly string[]) {
  const palette = colors.map((hex) => [
    parseInt(hex.slice(1, 3), 16),
    parseInt(hex.slice(3, 5), 16),
    parseInt(hex.slice(5, 7), 16),
  ]);
  const output = new Uint8ClampedArray(pixels.length);
  for (let offset = 0; offset < pixels.length; offset += 4) {
    // Both sign shaders resolve overlap in A, B, G, R order.
    for (let layer = 3; layer >= 0; layer--) {
      if (pixels[offset + layer] < 128) continue;
      output.set(palette[layer], offset);
      output[offset + 3] = 255;
      break;
    }
  }
  return output;
}

export function createSignPaintPreview(
  colors: readonly string[],
  displayPixels = signDisplayPixels,
) {
  const scratch = document.createElement("canvas");
  const bitmap = scratch.getContext("2d");
  return (context: CanvasRenderingContext2D, doc: RasterDocument) => {
    if (!bitmap) return;
    if (scratch.width !== doc.width) scratch.width = doc.width;
    if (scratch.height !== doc.height) scratch.height = doc.height;
    bitmap.putImageData(
      new ImageData(displayPixels(doc.pixels, colors), doc.width, doc.height),
      0,
      0,
    );
    context.imageSmoothingEnabled = false;
    context.drawImage(scratch, 0, 0);
  };
}

export function createSignLettering(width: number, height: number, text: string) {
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) return null;
  const letteringFont = (size: number) => `700 ${size}px system-ui, sans-serif`;
  context.textAlign = "center";
  context.textBaseline = "middle";
  const multiline = layoutMultilineLettering(text, width, height, (line, fontSize) => {
    context.font = letteringFont(fontSize);
    return context.measureText(line).width;
  });
  if (multiline) {
    context.font = letteringFont(multiline.fontSize);
    for (const line of multiline.lines)
      context.fillText(line.text, width / 2, line.y);
  } else {
    context.font = letteringFont(Math.max(1, Math.floor(height * 0.45)));
    context.fillText(text, width / 2, height / 2, width * 0.86);
  }
  return context.getImageData(0, 0, width, height).data;
}

export function stampSignLettering(
  pixels: Uint8ClampedArray,
  glyphs: Uint8ClampedArray,
  layer: number,
  strength = 255,
) {
  let changed = false;
  for (let offset = 0; offset < pixels.length; offset += 4) {
    if (glyphs[offset + 3] < 128) continue;
    for (let channel = 0; channel < 4; channel++) {
      const value = channel === layer ? strength : 0;
      if (pixels[offset + channel] !== value) changed = true;
      pixels[offset + channel] = value;
    }
  }
  return changed;
}
