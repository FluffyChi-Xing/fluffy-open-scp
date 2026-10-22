"""Reproducible four-asset experiment after exporting both material bindings.

python tools/building_ep1_probe.py tmp/ep1_planes
The recipes are authored square-plane layouts, NOT reconstructed game geometry.
"""
import argparse
import json
from collections import Counter
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw
from building_plane_probe import Probe, grid, save, sheet


def run(root, size=768):
    ids = ['35dd7d33', '1e89c4e3', 'd1f40247', '4cc44d42']
    uv = grid(size)
    # Square test domain: material-column selection and the two UVs are the
    # only authored inputs. Every sampled pixel comes from the extracted maps.
    recipes = [
        # Two windows per bay, stone pilasters and a horizontal string course.
        dict(column=11, base=[6, 6], top=[6, 3], pillar=5, belt=10),
        # Factory: large glazed upper wall, brick plinth and diamond accents.
        dict(column=6, base=[8, 4], top=[4, 2], wall=0, accent=32, belt=2),
        # Stone walls with narrow windows and a central bank of larger windows.
        dict(column=11, base=[6, 6], top=[5, 4], center=4, ornament=17),
        # Continuous glass panels, including the blue horizontal spandrel.
        dict(column=3, base=[8, 6], top=[6, 4]),
    ]
    report = []
    for asset, recipe in zip(ids, recipes):
        p = Probe(root / asset / 'binding1')
        columns = np.full((size, size), recipe['column'])
        buv, tuv = uv * recipe['base'], uv * recipe['top']
        if asset == ids[0]:
            columns[np.mod(uv[..., 0] * 3, 1) < .12] = recipe['pillar']
            columns[np.mod(uv[..., 1] * 3, 1) < .045] = recipe['belt']
        elif asset == ids[1]:
            columns[uv[..., 1] > .57] = recipe['wall']
            band = (uv[..., 1] > .59) & (uv[..., 1] < .77)
            columns[band] = recipe['accent']
            # Give each diamond its full atlas rectangle, with the original
            # padding controlling its coverage in the horizontal repeat cell.
            tuv[band, 0] = uv[band, 0] * 4
            tuv[band, 1] = (uv[band, 1] - .59) / .18
            columns[(uv[..., 1] > .55) & (uv[..., 1] < .59)] = recipe['belt']
        elif asset == ids[2]:
            center = (uv[..., 0] > .32) & (uv[..., 0] < .68)
            columns[center] = recipe['center']
            # Each material panel gets its own local UV domain. Cutting a
            # globally repeated window at the panel boundary produces half windows.
            tuv[..., 0] = np.where(uv[..., 0] <= .32, uv[..., 0] / .32 * 2,
                                  (uv[..., 0] - .68) / .32 * 2)
            tuv[center, 0] = (uv[center, 0] - .32) / .36 * 2
            columns[center & (np.mod(uv[..., 1] * 4, 1) < .10)] = recipe['ornament']
        result = p.shade(columns, buv, tuv)
        stats = {}
        for channel, pixels in result.items():
            assert np.isfinite(pixels).all(), (asset, channel)
            save(root / asset / f'square_{channel}.png', pixels)
            stats[channel] = {'min': float(pixels.min()), 'max': float(pixels.max()),
                              'mean': float(pixels.mean())}
        bindings = []
        for directory in (root / asset, root / asset / 'binding1'):
            probe = Probe(directory)
            mesh = json.loads((directory / 'mesh.json').read_text())
            used = Counter(v['column'] for v in mesh['vertices'])
            assert max(used) < probe.meta['width']
            sheet(probe, 'motif', columns=sorted(used), name='used_columns_motif.png')
            bindings.append({'mesh_section': mesh['mesh_section'], 'slots': probe.meta['slots'],
                             'used_columns': dict(sorted(used.items())),
                             'vertex_count': len(mesh['vertices']),
                             'triangle_count': len(mesh['triangles'])})
        # Deliberate ablation: preserve UVs and disable only the Top blend.
        base = p.shade(columns, buv, tuv, base_only=True)
        save(root / asset / 'square_base_only.png', base['display'])
        save(root / asset / 'square_top_difference.png', np.abs(result['display'] - base['display']))
        report.append({'property': p.meta['property_tgi'], 'model': p.meta['model_tgi'],
                       'recipe': recipe, 'bindings': bindings, 'channels': stats,
                       'top_changed_fraction': float(np.mean(np.max(np.abs(result['albedo'] - base['albedo']), axis=-1) > 1/255)),
                       'window_fraction': float(np.mean(result['opacity'] < .5))})
    (root / 'results.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    # IDs are language-independent labels; explanations live in the report.
    canvas = Image.new('RGB', (size*2, (size+32)*2), '#20252b')
    draw = ImageDraw.Draw(canvas)
    for i, asset in enumerate(ids):
        x, y = i % 2 * size, i // 2 * (size+32)
        canvas.paste(Image.open(root / asset / 'square_display.png'), (x, y+32))
        draw.text((x+10, y+8), asset, fill='white')
    canvas.save(root / 'comparison.png')
    print(json.dumps({'assets': len(report), 'status': 'ok', 'output': str(root)}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    run(parser.parse_args().directory)
