import { signDisplayPixels } from "./signPaint";

export const graffitiPalette = ["#eec866", "#5ac4a6", "#e17c76", "#e4e3dd"];

/** Match the editor shader's A/B/G/R priority and preserve the chosen coverage. */
export function graffitiDisplayPixels(pixels: Uint8ClampedArray, colors: readonly string[]) {
  const output = signDisplayPixels(pixels, colors);
  for (let offset = 0; offset < pixels.length; offset += 4) {
    for (let channel = 3; channel >= 0; channel--) {
      const coverage = pixels[offset + channel];
      if (coverage < 128) continue;
      output[offset + 3] = coverage;
      break;
    }
  }
  return output;
}
