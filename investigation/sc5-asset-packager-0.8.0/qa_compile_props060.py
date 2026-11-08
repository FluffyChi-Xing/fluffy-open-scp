from pathlib import Path
import json,sys
r=Path.cwd();sys.path.insert(0,str(r/'SC5 Asset Packager'));import sc5_packager
p=r/'SC5 Asset Packager/Projects/PropsQA060';sc5_packager.open_asset_folder(p)
f=p/'project.json';d=json.loads(f.read_text(encoding='utf-8'));d['nameEn']='SC5 Props Test';d['category']='park';d['settings']['rotationDegrees']=90;f.write_text(json.dumps(d,ensure_ascii=False,indent=2),encoding='utf-8')
from preview_3d import load_scene
s=load_scene(p/'PropsHouse.lotm');assert s['propCount']==6;print('PREVIEW_SCENE_PASS',len(s['propTemplates']),s['propCount'])
from backend_compile import compile_project
print(compile_project(p))
