"""CPU, mip-0 building4 plane experiments. Requires numpy and Pillow.

Run the Rust building_material_probe first; then:
  python tools/building_plane_probe.py tmp/building_probe_10daf7bf

No game files are changed. The historical Rust compose_*.png are NOT inputs.
"""
import argparse
import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw


def linear(x):
    return np.where(x <= .04045, x / 12.92, ((x + .055) / 1.055) ** 2.4)


def srgb(x):
    x = np.maximum(x, 0)
    return np.where(x <= .0031308, x * 12.92, 1.055 * x ** (1 / 2.4) - .055)


def sample(tex, uv, region=None):
    """GPU linear clamp, pixel centres at (n+.5)/size; signed atlas regions."""
    h, w = tex.shape[:2]
    if region is not None:
        lo = np.minimum(region[..., 2:], region[..., 2:] + region[..., :2])
        hi = np.maximum(region[..., 2:], region[..., 2:] + region[..., :2])
        inset = np.minimum(np.array([.5 / w, .5 / h]), (hi - lo) / 2)
        uv = np.clip(uv, lo + inset, hi - inset)
    xy = np.clip(uv * [w, h] - .5, [0, 0], [w - 1, h - 1])
    a = np.floor(xy).astype(int)
    b = np.minimum(a + 1, [w - 1, h - 1])
    f = xy - a
    x, y = f[..., 0, None], f[..., 1, None]
    return ((tex[a[..., 1], a[..., 0]] * (1-x) + tex[a[..., 1], b[..., 0]] * x) * (1-y)
            + (tex[b[..., 1], a[..., 0]] * (1-x) + tex[b[..., 1], b[..., 0]] * x) * y)


def grid(size):
    y, x = np.mgrid[:size, :size]
    return np.stack([(x + .5) / size, (y + .5) / size], -1)


def save(path, rgb):
    Image.fromarray(np.uint8(np.clip(rgb, 0, 1) * 255 + .5)).save(path)


class Probe:
    def __init__(self, root):
        self.root = root
        self.meta = json.loads((root / 'params.json').read_text())
        self.params = np.array(self.meta['values']).reshape(4, self.meta['width'], 4)
        self.maps = {}
        for slot in range(1, 6):
            tex = np.array(Image.open(root / f'slot{slot}.png').convert('RGBA'), dtype=float) / 255
            if slot in (4, 5):
                tex[..., :3] = linear(tex[..., :3])
            self.maps[slot] = tex

    def shade(self, column, base_uv, top_uv, *, base_only=False, variation=0):
        pal, base, top, room = (self.params[r, column] for r in range(4))
        buv = np.mod(base_uv, 1) * base[..., :2] + base[..., 2:]
        relief = np.mod(top_uv, 1) * (1 + room[..., :2]) - room[..., :2] / 2
        inside = np.all((relief >= 0) & (relief <= 1), axis=-1)
        inside &= np.all(np.abs(top[..., :2]) > 1e-6, axis=-1)
        tuv = np.clip(relief, 0, 1) * top[..., :2] + top[..., 2:]
        tb = sample(self.maps[1], buv, base)
        tt = sample(self.maps[1], tuv, top)
        mask = (tt[..., 3] * inside)[..., None]
        if base_only:
            mask = np.zeros_like(mask)
        def blend(a, b):
            return a * (1-mask) + b * mask
        def palette(tint, origin, row):
            uv = tint[..., :2] * [1/512, 1/16] + [1/1024, 1/32]
            uv = uv + np.stack([origin + np.zeros(uv.shape[:-1]), np.full(uv.shape[:-1], row)], -1)
            return sample(self.maps[4], uv)
        colour = blend(palette(tb, pal[..., 0], variation/8), palette(tt, pal[..., 1], variation/8))
        brightness = blend(tb[..., 2, None], tt[..., 2, None]) * 2
        normal = blend(sample(self.maps[2], buv, base), sample(self.maps[2], tuv, top))
        shader = blend(sample(self.maps[3], buv, base), sample(self.maps[3], tuv, top))
        albedo = colour[..., :3] * brightness * normal[..., 3, None]
        # Deliberately separate the true material albedo from an illustrative
        # daylight view. Full ray-box interior, cone-step and game lighting are
        # outside this orthographic probe; never label this game-exact output.
        room_uv = np.mod(base_uv * base[..., :2] * room[..., 2:], 1)
        interior_uv = room_uv * [.5, .5] + [.25, .25]
        interior_uv[..., 1] = interior_uv[..., 1] * pal[..., 2] + pal[..., 3]
        interior = sample(self.maps[5], interior_uv)
        interior_rgb = interior[..., :3] * (.25 + interior[..., 3, None] * 16)
        opacity = shader[..., 3, None]
        surface = blend(palette(tb, pal[..., 0], .875), palette(tt, pal[..., 1], .875))
        # Fixed sky proxy, intentionally not a claim about the game's cubemap.
        glass = np.array([.27, .34, .40]) * surface[..., 3, None] * brightness
        display = albedo * opacity + (interior_rgb + glass) * (1-opacity)
        cutout = tb[..., 3] < .5
        for arr in (albedo, display):
            arr[cutout] = [.12, .14, .17]
        return {'albedo': srgb(albedo), 'display': srgb(display),
                'overlay': mask[..., 0], 'ao': normal[..., 3],
                'opacity': shader[..., 3], 'height': shader[..., 0],
                'normal': normal[..., :3], 'shader': shader[..., :3],
                'surface': surface[..., :3], 'reflectance': surface[..., 3]}

    def tile(self, col, size=192, mode='padding', repeat=(1, 1)):
        uv = grid(size) * repeat
        top = uv
        if mode == 'motif':
            # Invert padding to expose the whole motif. This proves that even
            # huge padding is NOT equivalent to a disabled overlay.
            pad = self.params[3, col, :2]
            top = (grid(size) + pad/2) / (1+pad)
        return self.shade(col, uv, top, base_only=mode == 'base')


def sheet(probe, mode, size=160, columns=None, name=None):
    columns = list(range(probe.meta['width'])) if columns is None else list(columns)
    count = len(columns)
    cols = 8
    rows = (count + cols - 1) // cols
    canvas = Image.new('RGB', (cols * size, rows * (size+24)), '#20252b')
    draw = ImageDraw.Draw(canvas)
    for index, col in enumerate(columns):
        rgb = probe.tile(col, size, mode)['display']
        tile = Image.fromarray(np.uint8(np.clip(rgb, 0, 1) * 255 + .5))
        x, y = index % cols * size, index // cols * (size+24)
        canvas.paste(tile, (x, y+24))
        draw.text((x+6, y+5), f'{col:02d}', fill='white')
    canvas.save(probe.root / (name or f'columns_{mode}.png'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--size', type=int, default=1024)
    parser.add_argument('--columns', default='6,7,27', help='Material columns for square planes')
    parser.add_argument('--base-repeat', type=float, nargs=2, default=[8, 8])
    parser.add_argument('--top-repeat', type=float, nargs=2, default=[3, 3])
    args = parser.parse_args()
    if not 64 <= args.size <= 4096:
        parser.error('--size must be in [64, 4096]')
    p = Probe(args.directory)
    for mode in ('base', 'padding', 'motif'):
        sheet(p, mode)
    # Generic controlled experiment: one square, with independently authored
    # wall repeat and window repeat. Columns are explicit asset-specific choices.
    uv = grid(args.size)
    for col in map(int, args.columns.split(',')):
        if not 0 <= col < p.meta['width']:
            parser.error(f'column {col} out of range')
        result = p.shade(col, uv * args.base_repeat, uv * args.top_repeat)
        for channel, pixels in result.items():
            save(p.root / f'plane_{col:02d}_{channel}.png', pixels)
    print(json.dumps({'directory': str(p.root), 'columns': p.meta['width'],
                      'status': 'ok'}, ensure_ascii=False))


if __name__ == '__main__':
    main()
