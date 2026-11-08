import sys,time,json,ctypes as c
from pathlib import Path
from PIL import Image
r=Path.cwd();sys.path.insert(0,str(r/'SC5 Asset Packager'))
from sc5_packager_gui import App
p=r/'SC5 Asset Packager/Projects/LightsQA070';app=App();app.project=p;app._load_config();app.viewer.load_project(p)
limit=time.monotonic()+30
while (app.viewer.gpu is None or app.viewer.gpu.scene is None) and time.monotonic()<limit:app.update();time.sleep(.02)
v=app.viewer;assert v.gpu,v.info.cget('text');g=v.gpu;times=[]
for i in range(40):
 start=time.perf_counter();g.render(90,-.8+i*.01,.5,1);g.gl.glFinish();times.append((time.perf_counter()-start)*1000)
g.render(90,-.8,.6,1);g.gl.glFinish()
w,h=v.canvas.winfo_width(),v.canvas.winfo_height();buf=(c.c_ubyte*(w*h*4))();g.gl.glReadBuffer.argtypes=[c.c_uint];g.gl.glReadPixels.argtypes=[c.c_int,c.c_int,c.c_int,c.c_int,c.c_uint,c.c_uint,c.c_void_p];g.gl.glReadBuffer(0x404);g.gl.glReadPixels(0,0,w,h,0x1908,0x1401,buf)
Image.frombytes('RGBA',(w,h),bytes(buf)).transpose(Image.Transpose.FLIP_TOP_BOTTOM).save(p/'preview-lights.png')
from PIL import ImageGrab
app.update();ImageGrab.grab(bbox=(app.winfo_rootx(),app.winfo_rooty(),app.winfo_rootx()+app.winfo_width(),app.winfo_rooty()+app.winfo_height())).save(p/'application-logo.png')
result={'props':v.scene['propCount'],'lights':v.scene['lightCount'],'logoWidth':app.brand_logo.width(),'logoTransparent':bool(app.brand_logo_source.transparency_get(0,0)),'templates':len(g.prop_meshes),'bodyTriangles':v.scene['triangleCount'],'renderer':g.renderer,'meanFrameMs':sum(times)/len(times),'glError':g.gl.glGetError()};assert result['props']==1 and result['lights']==4 and result['logoTransparent'] and result['glError']==0,result
(p/'gpu-qa.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8');app.destroy();print(result)
