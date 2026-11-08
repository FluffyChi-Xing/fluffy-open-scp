import argparse
import contextlib
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import sc5_packager


class OutputPaths(unittest.TestCase):
    def test_reports_actual_fresh_package_and_rejects_stale_file(self):
        with tempfile.TemporaryDirectory() as raw,contextlib.redirect_stdout(io.StringIO()):
            project=Path(raw)
            sc5_packager.init_project(project,argparse.Namespace(name='Test',category='education'))
            config=sc5_packager.load_project(project)
            config['backend']={'command':['test-compiler']}
            (project/'project.json').write_text(json.dumps(config),encoding='utf-8')
            def compiler(*args,**kwargs):
                (project/'output'/'new.package').write_bytes(b'new-output')
                return subprocess.CompletedProcess([],0)
            with patch('sc5_packager.subprocess.run',side_effect=compiler):
                self.assertEqual(sc5_packager.build_project(project,False),0)
            result=json.loads((project/'reports'/'build-result.json').read_text(encoding='utf-8'))
            self.assertEqual(result['packages'][0]['path'],str((project/'output'/'new.package').resolve()))
            self.assertEqual(result['outputDirectory'],str((project/'output').resolve()))
            with patch('sc5_packager.subprocess.run',return_value=subprocess.CompletedProcess([],0)):
                self.assertEqual(sc5_packager.build_project(project,False),1)
            result=json.loads((project/'reports'/'build-result.json').read_text(encoding='utf-8'))
            self.assertEqual(result['packages'],[])
            self.assertIn('旧输出',result['error'])


if __name__=='__main__':unittest.main()
