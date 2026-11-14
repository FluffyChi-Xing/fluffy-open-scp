"""Shared preview/compiler orientation and native interior-state rules."""
import numpy as np


def rotation_degrees(value):
    if isinstance(value, bool) or value not in (0, 90, 180, 270):
        raise ValueError('朝向仅支持 0、90、180、270 度')
    return int(value)


def rotate_xyz(points, degrees):
    degrees = rotation_degrees(degrees)
    matrices = {0: ((1,0,0),(0,1,0),(0,0,1)),
                90: ((0,-1,0),(1,0,0),(0,0,1)),
                180: ((-1,0,0),(0,-1,0),(0,0,1)),
                270: ((0,1,0),(-1,0,0),(0,0,1))}
    return np.asarray(points) @ np.asarray(matrices[degrees]).T


def rotate_lot(mask, size, offset, period, degrees):
    degrees = rotation_degrees(degrees)
    swapped = degrees in (90,270)
    size = list(reversed(size)) if swapped else list(size)
    period = list(reversed(period)) if swapped else list(period)
    offset = rotate_xyz([[*offset,0]], degrees)[0,:2].tolist()
    return np.rot90(mask, degrees//90).copy(), size, offset, period


def room_strip(room, scale, offset, night_only=True):
    """Exactly the existing native sampling; alter only non-night emission.

    The engine chooses variation*4 + state. State 1 is the night cell;
    RGB remains intact even when emission alpha is zero.
    """
    h,w = room.shape[:2]
    u = (np.arange(64)+.5)/64
    yy = np.floor(np.mod(u*scale+offset,1)*h).astype(int)
    tiles, evidence = [], []
    for state in range(16):
        xx = np.floor(np.mod((u+state)*scale,1)*w).astype(int)
        original = room[yy[:,None],xx[None,:]]
        tile = original.copy()
        if night_only and state%4 != 1:
            tile[:,:,3] = 0
        tiles.append(tile)
        evidence.append({'state':state, 'alphaBefore':int(original[:,:,3].sum()),
                         'alphaAfter':int(tile[:,:,3].sum()),
                         'rgbUnchanged':bool(np.array_equal(original[:,:,:3],tile[:,:,:3])),
                         'nightUnchanged':bool(np.array_equal(original,tile)) if state%4==1 else None})
    return np.concatenate(tiles,axis=1), evidence
