"""Normalize user menu icons once; preserve alpha and avoid stretching."""
from pathlib import Path
import io
from PIL import Image, ImageOps

def icon_png(path):
    with Image.open(path) as im:
        im=ImageOps.exif_transpose(im).convert('RGBA')
        im=ImageOps.fit(im,(128,128),method=Image.Resampling.LANCZOS)
        buf=io.BytesIO();im.save(buf,format='PNG');return buf.getvalue()

def save_project_icon(project,source):
    data=icon_png(source)
    folder=Path(project)/'ui';folder.mkdir(exist_ok=True)
    target=folder/'menu-icon.png';target.write_bytes(data)
    return str(target.resolve())
