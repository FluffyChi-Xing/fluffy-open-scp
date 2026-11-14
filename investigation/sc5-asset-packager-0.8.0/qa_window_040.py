"""Exercise the actual preview-loading and orientation-saving UI code."""
from pathlib import Path
import json
import tempfile
from unittest.mock import patch
from sc5_packager_gui import App

root=Path(__file__).resolve().parent
config=json.loads((root/'Projects/OldTower040/project.json').read_text(encoding='utf-8'))
with tempfile.TemporaryDirectory() as raw:
    project=Path(raw)
    config['settings']['rotationDegrees']=0
    (project/'project.json').write_text(json.dumps(config,ensure_ascii=False),encoding='utf-8')
    app=App()
    try:
        with patch('sc5_packager_gui.filedialog.askdirectory',return_value=str(project)):
            app.open_project()
        app.after(4000,app.quit)
        app.mainloop()
        assert app.viewer.scene is not None,app.viewer.info.cget('text')
        assert app.viewer.scene['triangleCount']==57430
        assert app.viewer.info.winfo_height()>20,'Preview metrics clipped'
        assert app.asset_tree.winfo_width()>350,'File pane collapsed'
        assert app.output_directory.get()==str((project/'output').resolve())
        app.viewer.turn(90)
        saved=json.loads((project/'project.json').read_text(encoding='utf-8'))
        assert saved['settings']['rotationDegrees']==90
        app.viewer.turn(-90)
        assert app.rotation.get()==0
        result={'loadedTriangles':57430,'rotationSaved':90,'outputPathVisible':True,
                'filePaneWidth':app.asset_tree.winfo_width(),'previewInfoHeight':app.viewer.info.winfo_height()}
        (root/'Projects/PreviewQA040/window-check.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
        print(json.dumps(result))
    finally:app.destroy()
