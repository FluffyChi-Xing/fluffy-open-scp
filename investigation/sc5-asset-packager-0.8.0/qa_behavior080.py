"""Prepare an isolated park functional candidate with previously tested inputs."""
from pathlib import Path
import json,shutil
ROOT=Path(__file__).resolve().parent
project=ROOT/'Projects/BehaviorQA080'
project.mkdir(exist_ok=True)
config=json.loads((ROOT/'Projects/LightsQA070/project.json').read_text(encoding='utf-8'))
inputs=project/'input';inputs.mkdir(exist_ok=True)
for source in config['sources']:
    path=Path(source);shutil.copyfile(path,inputs/path.name)
config['sources']=[]
config.update(name='基础公园功能测试',nameEn='SC5 Native Park Function Test',version='0.8.0',category='park',
              description='Native park behavior test. Custom house, ground, tree and decorative lights. City effects require in-game validation.')
config['settings'].update(behaviorProfile='native-park-w1-v1',provideSimulationService=True,cost=400,maintenancePerHour=40)
(project/'project.json').write_text(json.dumps(config,ensure_ascii=False,indent=2),encoding='utf-8')
if __name__=='__main__':
    from sc5_packager import build_project
    raise SystemExit(build_project(project,False))
