from pathlib import Path
import json,sys
r=Path.cwd();sys.path.insert(0,str(r/'SC5 Asset Packager'));import sc5_packager
p=r/'SC5 Asset Packager/Projects/LightsQA070';sc5_packager.open_asset_folder(p);f=p/'project.json';d=json.loads(f.read_text(encoding='utf-8'));d['sources']=[str(x) for x in p.iterdir() if x.is_file() and x.name.startswith('LightsHouse.')];d['nameEn']='SC5 Lighting Test';d['category']='park';d['settings']['rotationDegrees']=90;f.write_text(json.dumps(d,ensure_ascii=False,indent=2),encoding='utf-8')
from preview_3d import load_scene
s=load_scene(p/'LightsHouse.lotm');assert s['lightCount']==4 and s['propCount']==1
from backend_compile import compile_project
result=compile_project(p);assert result['lights']['count']==4;assert not result['lights']['powerControlVerified'];print('LIGHTS_PACKAGE_PASS',result['bytes'])
