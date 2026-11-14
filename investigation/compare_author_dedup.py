import json, sys
from pathlib import Path
import numpy as np
sys.path.insert(0, 'tools/sc5_asset_packager/runtime/engine')
from probe_tower import read
from compile_tower_asset import accessor
def rotate_xyz(points, degrees):
    assert degrees == 180
    return np.asarray(points) * np.asarray([-1, -1, 1])

chunks, mats, _, diag = read('tools/sc5_asset_packager/projects/tall_test/input/tall_test.lotm')
author = json.loads(Path('tools/sc5_asset_packager/projects/tall_test/build/run-c0814b62eacb/希望大厦.geometry.json').read_text('utf8'))
for i, ((g, b), obj) in enumerate(zip(chunks, author['objects'])):
    pr = g['meshes'][0]['primitives'][0]
    a = pr['attributes']
    pos = rotate_xyz(accessor(g,b,a['POSITION']), 180)
    norm = rotate_xyz(accessor(g,b,a['NORMAL']), 180)
    tan = accessor(g,b,a['TANGENT'])
    tan[:,:3] = rotate_xyz(tan[:,:3], 180)
    uv = accessor(g,b,a['TEXCOORD_0'])
    top = accessor(g,b,a['TEXCOORD_3'])
    seeds = accessor(g,b,a['TEXCOORD_1'])
    lookup = {}
    for j in range(len(pos)):
        seed = int(np.clip(round(float(seeds[j,1])*255),0,255))
        key = tuple(pos[j].tolist()+norm[j].tolist()+tan[j,:3].tolist()+uv[j].tolist()+top[j].tolist())+(seed,)
        if key not in lookup:
            lookup[key] = len(lookup)
    if len(lookup) != len(obj['vertices']):
        print(i, 'raw', len(lookup), 'author', len(obj['vertices']), 'source', len(pos))
print('raw_total', sum(len(o['vertices']) for o in author['objects']), 'author_total', sum(len(o['vertices']) for o in author['objects']))
