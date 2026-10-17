import { validateSize } from "./schema";

/** Lossless PNG without canvas premultiplication (RGB under zero alpha is data). */
export function encodeRgbaPng(
  width: number,
  height: number,
  rgba: Uint8ClampedArray,
): Uint8Array {
  validateSize(width, height);
  if (rgba.length !== width * height * 4)
    throw new Error("RGBA length does not match dimensions");
  const scan = new Uint8Array((width * 4 + 1) * height);
  for (let y = 0; y < height; y++)
    scan.set(
      rgba.subarray(y * width * 4, (y + 1) * width * 4),
      y * (width * 4 + 1) + 1,
    );
  const zlib = new Uint8Array(
    2 + scan.length + Math.ceil(scan.length / 65535) * 5 + 4,
  );
  zlib.set([0x78, 0x01]);
  let offset = 2,
    a = 1,
    b = 0;
  for (let start = 0; start < scan.length; start += 65535) {
    const length = Math.min(65535, scan.length - start);
    zlib.set(
      [
        start + length === scan.length ? 1 : 0,
        length & 255,
        length >>> 8,
        ~length & 255,
        (~length >>> 8) & 255,
      ],
      offset,
    );
    offset += 5;
    zlib.set(scan.subarray(start, start + length), offset);
    offset += length;
  }
  for (const byte of scan) {
    a = (a + byte) % 65521;
    b = (b + a) % 65521;
  }
  new DataView(zlib.buffer).setUint32(offset, ((b << 16) | a) >>> 0);
  const header = new Uint8Array(13),
    view = new DataView(header.buffer);
  view.setUint32(0, width);
  view.setUint32(4, height);
  header.set([8, 6, 0, 0, 0], 8);
  const chunks = [
    new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk("IHDR", header),
    chunk("IDAT", zlib),
    chunk("IEND", new Uint8Array()),
  ];
  const png = new Uint8Array(chunks.reduce((n, c) => n + c.length, 0));
  offset = 0;
  for (const bytes of chunks) {
    png.set(bytes, offset);
    offset += bytes.length;
  }
  return png;
}
function chunk(type: string, data: Uint8Array) {
  const bytes = new Uint8Array(data.length + 12),
    view = new DataView(bytes.buffer);
  view.setUint32(0, data.length);
  bytes.set(new TextEncoder().encode(type), 4);
  bytes.set(data, 8);
  let crc = 0xffffffff;
  for (let i = 4; i < data.length + 8; i++) {
    crc ^= bytes[i];
    for (let bit = 0; bit < 8; bit++)
      crc = (crc >>> 1) ^ (crc & 1 ? 0xedb88320 : 0);
  }
  view.setUint32(data.length + 8, (crc ^ 0xffffffff) >>> 0);
  return bytes;
}
