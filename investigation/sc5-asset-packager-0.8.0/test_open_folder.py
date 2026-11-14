"""Regression checks for export-folder import and GUI project switching."""
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import sc5_packager as core


class OpenFolderTests(unittest.TestCase):
    def test_exports_open_without_copy_or_manifest_overwrite(self):
        with tempfile.TemporaryDirectory() as raw:
            folder = Path(raw) / "老式大楼"
            folder.mkdir()
            names = ["Box001.002_原生.glb", "Box001.002_原生.lotm",
                     "Box001.002_原生.lot.package", "Box001.002_原生.lotmask.png",
                     "Box001.002_原生.lotpaint.json", "Box001.002_原生.native.json"]
            originals = {}
            for name in names:
                p = folder / name
                originals[p] = b'{}' if name.endswith('.json') else b'source-preserved'
                p.write_bytes(originals[p])
            project, created = core.open_asset_folder(folder)
            self.assertTrue(created)
            config = core.load_project(project)
            self.assertEqual(len(config['sources']), 6)
            self.assertIsNone(config['backend'])
            self.assertEqual(len(core.inspect_inputs(project)['files']), 6)
            self.assertEqual(core.build_project(project, True), 0)
            manifest = (project / 'project.json').read_bytes()
            (project / 'output').mkdir(exist_ok=True)
            (project / 'output' / 'generated.package').write_bytes(b'output')
            self.assertFalse(core.open_asset_folder(folder)[1])
            self.assertEqual((project / 'project.json').read_bytes(), manifest)
            for p, value in originals.items():
                self.assertEqual(p.read_bytes(), value)

    def test_empty_and_invalid_project_not_replaced(self):
        with tempfile.TemporaryDirectory() as raw:
            folder = Path(raw)
            with self.assertRaises(ValueError):
                core.open_asset_folder(folder)
            self.assertFalse((folder / 'project.json').exists())
            (folder / 'model.glb').write_bytes(b'geometry')
            manifest = folder / 'project.json'
            manifest.write_text('{broken', encoding='utf-8')
            with self.assertRaises(json.JSONDecodeError):
                core.open_asset_folder(folder)
            self.assertEqual(manifest.read_text(encoding='utf-8'), '{broken')

    def test_gui_open_and_failed_switch(self):
        import os
        import sys
        runtime = Path(sys.executable).parent
        os.environ['TCL_LIBRARY'] = str(runtime / 'tcl' / 'tcl8.6')
        os.environ['TK_LIBRARY'] = str(runtime / 'tcl' / 'tk8.6')
        from sc5_packager_gui import App
        with tempfile.TemporaryDirectory() as raw:
            folder = Path(raw) / '老式大楼'
            folder.mkdir()
            (folder / 'model.glb').write_bytes(b'geometry')
            bad = Path(raw) / 'empty'
            bad.mkdir()
            app = App()
            app.withdraw()
            try:
                with patch('sc5_packager_gui.filedialog.askdirectory', return_value=str(folder)):
                    app.open_project()
                app.update()
                self.assertEqual(app.project, folder.resolve())
                self.assertEqual(app.name.get(), '老式大楼')
                self.assertEqual(len(app.asset_tree.get_children()), 1)
                with patch('sc5_packager_gui.filedialog.askdirectory', return_value=str(bad)), patch('sc5_packager_gui.messagebox.showerror') as error:
                    app.open_project()
                    error.assert_called_once()
                self.assertEqual(app.project, folder.resolve())
                self.assertEqual(app.name.get(), '老式大楼')
            finally:
                app.destroy()


if __name__ == '__main__':
    unittest.main()
