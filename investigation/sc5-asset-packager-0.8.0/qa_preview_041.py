from pathlib import Path
import json,time,tempfile,io
from unittest.mock import patch
import numpy as np
from PIL import Image
from sc5_packager_gui import App
from menu_icons import save_project_icon,icon_png
root=Path(__file__).resolve().parent
out=root/'release/0.4.1';out.mkdir(parents=True,exist_ok=True)
config=json.loads((root/'Projects/OldTower040/project.json').read_text(encoding='utf-8'))
project=root/'Projects/OldTower041';project.mkdir(exist_ok=True)
config['version']='0.4.1';config['nameEn']='Old Tower 041'
source=project/'custom-icon-source.png'
im=Image.new('RGBA',(240,160),(30,60,100,255))
from PIL import ImageDraw
ImageDraw.Draw(im).rectangle((60,20,180,140),fill=(240,170,40,255));im.save(source)
config['menuIconImage']=save_project_icon(project,source)
(project/'project.json').write_text(json.dumps(config,ensure_ascii=False,indent=2),encoding='utf-8')
app=App()
try:
    with patch('sc5_packager_gui.filedialog.askdirectory',return_value=str(project)):app.open_project()
    end=time.time()+45
    while (not app.viewer.gpu or app.viewer.gpu.scene is None) and time.time()<end:app.update();time.sleep(.02)
    v=app.viewer
    assert v.gpu and v.gpu.scene is not None,v.info.cget('text')
    assert 'GDI Generic' not in v.gpu.renderer,'Hardware OpenGL unavailable'
    timings=[]
    for i in range(120):
        start=time.perf_counter();v.gpu.render(0,-.8+i*.01,.35,1);v.gpu.gl.glFinish();timings.append((time.perf_counter()-start)*1000)
    g=v.gpu.gl
    import ctypes as c
    g.glReadPixels.argtypes=[c.c_int,c.c_int,c.c_int,c.c_int,c.c_uint,c.c_uint,c.c_void_p]
    g.glReadBuffer.argtypes=[c.c_uint];g.glReadBuffer(0x404)
    w=v.canvas.winfo_width();h=v.canvas.winfo_height();pixels=np.zeros((h,w,3),np.uint8)
    g.glPixelStorei(0xd05,1);g.glReadPixels(0,0,w,h,0x1907,0x1401,pixels.ctypes.data)
    Image.fromarray(pixels[::-1]).save(out/'preview-gpu.png')
    assert g.glGetError()==0,'OpenGL error'
    assert pixels.max()>100,'Empty framebuffer'
    v.turn(90);app.update();v.turn(-90)
    app.clear_menu_icon();assert not json.loads((project/'project.json').read_text(encoding='utf-8'))['menuIconImage']
    with patch('sc5_packager_gui.filedialog.askopenfilename',return_value=str(source)):app.choose_menu_icon()
    assert Image.open(io.BytesIO(icon_png(config['menuIconImage']))).size==(128,128)
    report={'triangles':v.scene['triangleCount'],'materialGroups':v.scene['materialGroups'],'renderer':v.gpu.renderer,'frames':120,'medianFrameMs':float(np.median(timings)),'p95FrameMs':float(np.percentile(timings,95)),'iconSelectedSavedRestored':True,'GLerror':0,'framebuffer':[w,h]}
    (out/'preview-icon-qa.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report))
finally:app.destroy()

