import type { Channel, RasterProfile } from "./schema";
import type { Rgba } from "./tools";

interface ProfileBehavior {
  dataChannels: boolean;
  layers: "lot" | "decal" | null;
  palette: boolean;
  defaultChannel: Channel;
}
// Exhaustive contract shared by the canvas, toolbar, inspector and previews.
export const PROFILE_BEHAVIOR: Record<RasterProfile, ProfileBehavior> = {
  slot0: {
    dataChannels: true,
    layers: null,
    palette: false,
    defaultChannel: "rgba",
  },
  slot1: {
    dataChannels: true,
    layers: null,
    palette: false,
    defaultChannel: "rgba",
  },
  slot2: {
    dataChannels: true,
    layers: null,
    palette: false,
    defaultChannel: "rgba",
  },
  slot3: {
    dataChannels: true,
    layers: null,
    palette: false,
    defaultChannel: "rgba",
  },
  slot4: {
    dataChannels: true,
    layers: null,
    palette: false,
    defaultChannel: "rgba",
  },
  slot5: {
    dataChannels: true,
    layers: null,
    palette: false,
    defaultChannel: "rgba",
  },
  "lot-mask": {
    dataChannels: true,
    layers: "lot",
    palette: false,
    defaultChannel: "rgba",
  },
  "lot-texture": {
    dataChannels: true,
    layers: null,
    palette: false,
    defaultChannel: "rgba",
  },
  "decal-sign": {
    dataChannels: true,
    layers: "decal",
    palette: true,
    defaultChannel: "rgba",
  },
  "decal-graffiti": {
    dataChannels: true,
    layers: "decal",
    palette: true,
    defaultChannel: "rgba",
  },
  "decal-sdf": {
    dataChannels: true,
    layers: null,
    palette: true,
    defaultChannel: "r",
  },
  "decal-interior": {
    dataChannels: true,
    layers: null,
    palette: false,
    defaultChannel: "rgba",
  },
  "decal-rgba": {
    dataChannels: false,
    layers: null,
    palette: false,
    defaultChannel: "rgba",
  },
  rgba: {
    dataChannels: false,
    layers: null,
    palette: false,
    defaultChannel: "rgba",
  },
};

export function layerPaint(
  profile: RasterProfile,
  layer: number,
  strength = 255,
): Rgba {
  const pixel: [number, number, number, number] = [0, 0, 0, 0];
  // Lot menu LC4/3/2/1 maps A/R/G/B; Decal Color1/2/3/4 maps R/G/B/A.
  const index =
    PROFILE_BEHAVIOR[profile].layers === "lot" ? [3, 0, 1, 2][layer] : layer;
  pixel[index] = Math.max(0, Math.min(255, strength));
  return pixel;
}

export function dataChannelPixels(pixels: Uint8ClampedArray) {
  const view = new Uint8ClampedArray(pixels);
  for (let index = 3; index < view.length; index += 4) view[index] = 255;
  return view;
}

export function pickedLayer(
  profile: RasterProfile,
  pixel: Rgba,
): number | null {
  const order =
    PROFILE_BEHAVIOR[profile].layers === "lot" ? [3, 0, 1, 2] : [3, 2, 1, 0];
  const channel = order.find((index) => pixel[index] >= 128);
  if (channel == null) return null;
  return PROFILE_BEHAVIOR[profile].layers === "lot"
    ? [3, 0, 1, 2].indexOf(channel)
    : channel;
}
