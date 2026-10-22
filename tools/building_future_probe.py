"""Metal / glass plane experiments. Run after the two Rust extractions.

python tools/building_future_probe.py tmp/future_planes
Layouts are authored probe inputs, not an original-mesh reconstruction.
"""
import argparse
import json
from pathlib import Path
from collections import Counter

import numpy as np
from PIL import Image, ImageDraw
from building_plane_probe import Probe, grid, save, sheet


def run(root):
    size = 768
    uv = grid(size)
    report = []
    for asset in ['5a6ec67a', '1d261fdd']:
        p = Probe(root / asset)
        mesh = json.loads((p.root / 'mesh.json').read_text())
        used = Counter(v['column'] for v in mesh['vertices'])
        assert max(used) < p.meta['width']
        sheet(p, 'padding', columns=sorted(used), name='used_columns_padding.png')
        if asset == '5a6ec67a':
            # Authored metal panel with windows, and a weathered lower band.
            col = np.full((size, size), 30)
            buv, tuv = uv * [4, 4], uv * [3, 3]
            band = uv[..., 1] > .85
            col[band] = 18
            buv[band, 0] = uv[band, 0] * 3
            buv[band, 1] = (uv[band, 1] - .85) / .15
            # Explicit plain panel cells expose the difference from window cells.
            plain = (np.floor(uv[..., 0]*3) + np.floor(uv[..., 1]*3)) % 3 == 0
            col[plain & ~band] = 39
            recipe = {'window': 30, 'plain': 39, 'lower_band': 18,
                      'base_repeat': [4, 4], 'top_repeat': [3, 3], 'band_start': .85}
        else:
            # The glass is already in Base. Do NOT force a window overlay:
            # this asset demonstrates that curtain walls need not be Top-driven.
            col = np.full((size, size), 7)
            buv, tuv = uv * [8, 5], uv * [8, 5]
            recipe = {'glass': 7, 'base_repeat': [8, 5], 'top_repeat': [8, 5]}
        full = p.shade(col, buv, tuv)
        base = p.shade(col, buv, tuv, base_only=True)
        for key, pixels in full.items():
            assert np.isfinite(pixels).all(), (asset, key)
            save(p.root / f'square_{key}.png', pixels)
        save(p.root / 'square_base_only.png', base['display'])
        save(p.root / 'square_top_difference.png', np.abs(full['display'] - base['display']))
        report.append({'property': p.meta['property_tgi'], 'model': p.meta['model_tgi'],
                       'package': p.meta['package'], 'slots': p.meta['slots'],
                       'parameter_columns': p.meta['width'], 'used_columns': sorted(used),
                       'mesh_section': mesh['mesh_section'], 'recipe': recipe,
                       'window_fraction': float(np.mean(full['opacity'] < .5)),
                       'top_changed_fraction': float(np.mean(np.max(np.abs(full['albedo']-base['albedo']), axis=-1) > 1/255)),
                       'channels': {key: {'min': float(a.min()), 'max': float(a.max()), 'mean': float(a.mean())}
                                    for key, a in full.items()}})
    (root / 'results.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    canvas = Image.new('RGB', (size*2, size+28), '#20252b')
    draw = ImageDraw.Draw(canvas)
    for i, asset in enumerate(['5a6ec67a', '1d261fdd']):
        draw.text((i*size+8, 8), asset, fill='white')
        canvas.paste(Image.open(root / asset / 'square_display.png'), (i*size, 28))
    canvas.save(root / 'comparison.png')
    print(json.dumps([{'property': r['property'], 'window_fraction': r['window_fraction'],
                       'top_changed_fraction': r['top_changed_fraction']} for r in report]))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    run(parser.parse_args().directory)
