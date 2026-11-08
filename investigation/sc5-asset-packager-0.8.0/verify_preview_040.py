"""Actual-asset preview and GUI QA entry; never opens or modifies the game."""
from pathlib import Path
import json
import time
from preview_3d import load_scene,render_scene

root=Path(__file__).resolve().parent
target=root/'Projects'/'PreviewQA040'
target.mkdir(parents=True,exist_ok=True)
asset=root.parent/'资产打包'/'老式大楼'/'Box001.002_原生.lotm'
started=time.perf_counter()
scene=load_scene(asset)
info=[]
for angle in (0,90,180,270):
    t=time.perf_counter()
    png,metadata=render_scene(scene,900,640,rotation=angle)
    (target/f'preview-{angle}.png').write_bytes(png)
    info.append({'angle':angle,'renderSeconds':time.perf_counter()-t,**metadata})
(target/'preview-check.json').write_text(json.dumps({'triangles':scene['triangleCount'],'groups':scene['materialGroups'],
    'vertices':scene['vertices'],'warnings':scene['warnings'],'views':info,
    'elapsedSeconds':time.perf_counter()-started},ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps({'triangles':scene['triangleCount'],'groups':scene['materialGroups'],'views':info},ensure_ascii=False))
