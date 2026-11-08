"""Vendor the verified local compiler building blocks, without game installation writes."""
from pathlib import Path
import sys
import json
import shutil

HERE = Path(__file__).resolve().parent
RESEARCH = HERE.parent / '对接研究' / 'rw4_bridge'
DEST = HERE / 'engine'
if (DEST / 'prepare_asset.py').is_file():
    raise SystemExit('engine 已含后续修正；本脚本仅供首次提取历史基线，不允许覆盖现有编译器。')
DEST.mkdir(exist_ok=True)
for name in ['probe_tower.py', 'compile_tower_asset.py', 'compile_geometry_contract.py',
             'rw4_bridge.py', 'collision_proxy.py', 'authored_materials.py',
             'package_io.py', 'native_property_codec.py', 'material_internal_refs.py',
             'independent_park_profile.py', 'interactive_park_profile.py']:
    shutil.copy2(RESEARCH / name, DEST / name)
# Use the parser without the old relative plugin import path.
s = (RESEARCH / 'inspect_park_route.py').read_text(encoding='utf-8')
start = s.index('def parse(')
end = s.index('\ndef ', start + 1)
(DEST / 'inspect_park_route.py').write_text('import struct\n' + s[start:end] + '\n', encoding='utf-8')
s = (HERE.parent / '纽约立面插件' / 'extract_game_ground_1_22.py').read_text(encoding='utf-8')
s = s.replace(";OUT.mkdir(parents=True,exist_ok=True)", "")
(DEST / 'extract_game_ground_1_22.py').write_text(s, encoding='utf-8')
s = (RESEARCH / 'build_candidate.py').read_text(encoding='utf-8')
s = s.replace("    require(OUT.is_relative_to(ROOT) and not ROOT.is_relative_to(GAME),'Unsafe workspace location')", "    require(OUT.is_relative_to(OUTPUT_ROOT) and not OUT.is_relative_to(GAME),'Unsafe project output location')")
s = s.replace("OUT=(ROOT/'第三阶段候选包').resolve()", "OUT=(ROOT/'第三阶段候选包').resolve()\nOUTPUT_ROOT=ROOT")
(DEST / 'build_candidate.py').write_text(s, encoding='utf-8')
s = (RESEARCH / 'verify_candidate.py').read_text(encoding='utf-8')
s = s[:s.index("resources=validate(package);results=")]
s = s.replace('candidate_root.is_relative_to(build_candidate.ROOT)', 'candidate_root.is_relative_to(build_candidate.OUTPUT_ROOT)')
s += '\nvalidate(package)\nprint("PACKAGE_VALIDATED")\n'
(DEST / 'verify_package.py').write_text(s, encoding='utf-8')
# The verified single mesh material transformation, with source/output passed by caller.
s = (RESEARCH / 'compile_tower_appearance_fixed.py').read_text(encoding='utf-8')
start = s.index('_spec=')
end = s.index('\nOUT=', start)
s = s[:start] + 'import native_controls as ctl' + s[end:]
s = s.replace("ROOT.parents[1]/'纽约立面插件/ny_facade/assets/packs/Office/slot5.png'", "ROOT/'office-room.png'")
(DEST / 'prepare_asset.py').write_text(s, encoding='utf-8')
shutil.copy2(HERE.parent / '纽约立面插件/ny_facade/native_controls.py', DEST / 'native_controls.py')
shutil.copy2(HERE.parent / '纽约立面插件/ny_facade/assets/packs/Office/slot5.png', DEST / 'office-room.png')
for relative in ['第四阶段研究/175EFB4E.research-template.rw4',
                 '第二阶段/CD6F8697.collision-source.rw4',
                 '材质测试0.2.0/DCBEA56D.texture-template.rw4',
                 '材质测试0.2.0/2EF7D20A.texture-template.rw4']:
    target = DEST / relative
    target.parent.mkdir(exist_ok=True)
    shutil.copy2(RESEARCH / relative, target)
sys.path.insert(0, str(RESEARCH))
from package_io import index
baseline = RESEARCH / '交互朝向研究0.7.0/构建核验'
r = json.loads((baseline / '候选包验证报告.json').read_text(encoding='utf-8'))
unit = tuple(int(x, 16) for x in r['identities']['unit'].split(':'))
(DEST / 'tested-unit.bin').write_bytes(next(e.read() for e in index(baseline / r['package']) if e.key == unit))
print('Engine prepared:', DEST)
