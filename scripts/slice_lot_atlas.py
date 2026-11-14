"""Split a SimCity 4x4 Lot ground atlas into sixteen labeled PNG tiles.

Usage: python scripts/slice_lot_atlas.py atlas.png output-directory
"""
from pathlib import Path
import sys

from PIL import Image


def split_atlas(source: Path, destination: Path) -> None:
    with Image.open(source).convert("RGBA") as atlas:
        if atlas.width % 4 or atlas.height % 4:
            raise ValueError("Lot atlas dimensions must be divisible by 4")
        destination.mkdir(parents=True, exist_ok=True)
        width, height = atlas.width // 4, atlas.height // 4
        for index in range(16):
            x, y = index % 4, index // 4
            atlas.crop((x * width, y * height, (x + 1) * width, (y + 1) * height)).save(
                destination / f"{index:02d}.png"
            )


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit("usage: slice_lot_atlas.py atlas.png output-directory")
    split_atlas(Path(sys.argv[1]), Path(sys.argv[2]))
