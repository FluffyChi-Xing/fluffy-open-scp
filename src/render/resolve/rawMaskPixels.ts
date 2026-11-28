/** One source per viewport: repeated scene assembly must not decode the same
 * multi-megabyte base64 RGBA mask on the UI thread each time. */
export function createRawMaskDecoder() {
  let cached: { source: string; pixels: ImageData } | null = null;
  return (source: string | null, width: number, height: number): ImageData | null => {
    if (!source) return null;
    if (cached?.source === source && cached.pixels.width === width && cached.pixels.height === height)
      return cached.pixels;
    const binary = atob(source);
    const expected = width * height * 4;
    if (binary.length < expected) return null;
    const pixels = new Uint8ClampedArray(expected);
    for (let i = 0; i < expected; i++) pixels[i] = binary.charCodeAt(i);
    const image = new ImageData(pixels, width, height);
    cached = { source, pixels: image };
    return image;
  };
}
