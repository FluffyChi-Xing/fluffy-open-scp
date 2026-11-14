import json
from pathlib import Path
import tempfile
import unittest

from backend_compile import compile_project


class CompileGuards(unittest.TestCase):
    def test_unsupported_profiles_do_not_start_build(self):
        cases = [({'category':'office'}, {}, '菜单位置'),
                 ({}, {'provideSimulationService':True}, '模拟效果'),
                 ({}, {'cost':-1}, '无效费用'),
                 ({}, {'geometryStrategy':'experimental_hybrid_lod'}, '几何策略'),
                 ({}, {'materialStrategy':'experimental'}, '材质策略')]
        for overrides, settings, message in cases:
            with self.subTest(message=message), tempfile.TemporaryDirectory() as raw:
                folder = Path(raw)
                config = {'schema':'sc5.asset-project/1','name':'Test','nameEn':'Test',
                          'version':'0.3.0','category':'education',
                          'settings':{'cost':100,'maintenancePerHour':10,
                                      'geometryStrategy':'stable_single_mesh',
                                      'materialStrategy':'compact_2048',
                                      'provideSimulationService':False}}
                config.update(overrides)
                config['settings'].update(settings)
                (folder/'project.json').write_text(json.dumps(config), encoding='utf-8')
                with self.assertRaisesRegex(ValueError, message):
                    compile_project(folder)
                self.assertFalse((folder/'build').exists())
                self.assertFalse((folder/'output').exists())


if __name__ == '__main__':
    unittest.main()
