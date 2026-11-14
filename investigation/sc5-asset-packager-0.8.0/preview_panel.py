import queue
import threading
from pathlib import Path
import vendor_tk as tk
from vendor_tk import ttk


class PreviewPanel(ttk.Frame):
    def __init__(self,parent,rotation,save_callback):
        super().__init__(parent)
        self.rotation=rotation;self.save_callback=save_callback
        self.scene=None;self.generation=0;self.pending=None
        self.yaw=-.8;self.pitch=.35;self.zoom=1.;self.last=None
        self.messages=queue.Queue()
        tools=ttk.Frame(self);tools.pack(fill='x',padx=6,pady=5)
        ttk.Label(tools,text='导出旋转').pack(side='left')
        combo=ttk.Combobox(tools,textvariable=rotation,values=(0,90,180,270),state='readonly',width=5)
        combo.pack(side='left',padx=4);combo.bind('<<ComboboxSelected>>',lambda e:self.orient())
        ttk.Button(tools,text='左转90°',command=lambda:self.turn(90)).pack(side='left',padx=3)
        ttk.Button(tools,text='右转90°',command=lambda:self.turn(-90)).pack(side='left',padx=3)
        ttk.Button(tools,text='适配视角',command=self.reset).pack(side='left',padx=3)
        self.building=tk.BooleanVar(value=True);self.ground=tk.BooleanVar(value=True);self.props=tk.BooleanVar(value=True);self.lights=tk.BooleanVar(value=True)
        toggles=ttk.Frame(self);toggles.pack(fill='x',padx=6)
        for label,var in [('建筑',self.building),('地块',self.ground),('道具',self.props),('灯光',self.lights)]:
            ttk.Checkbutton(toggles,text=label,variable=var,command=self.schedule).pack(side='left',padx=3)
        ttk.Label(self,text='蓝色箭头 = 游戏临街 +Y；请把实际主入口转到蓝色道路一侧。',foreground='#116887').pack(fill='x',padx=8)
        self.canvas=tk.Canvas(self,bg='#243347',highlightthickness=0,height=380)
        self.canvas.pack(fill='both',expand=True,padx=6,pady=5)
        self.canvas.bind('<Configure>',lambda e:self.schedule())
        self.canvas.bind('<ButtonPress-1>',self.press)
        self.canvas.bind('<B1-Motion>',self.drag)
        self.canvas.bind('<MouseWheel>',self.wheel)
        self.info=ttk.Label(self,text='打开资产文件夹后自动载入三维形体。',wraplength=690)
        self.info.pack(fill='x',side='bottom',before=self.canvas,padx=8,pady=4)
        ttk.Label(self,text='左键拖动环绕 · 滚轮缩放 · 形体/颜色近似预览，非游戏视差和昼夜渲染；地块为材质权重示意；灯光线框显示方向和范围，非游戏光照效果。',wraplength=690).pack(fill='x',side='bottom',before=self.info,padx=8,pady=(0,6))
        self.image=None;self.gpu=None
        self.canvas.bind("<Expose>",lambda e:self.schedule())
        self.bind("<Destroy>",self.cleanup)
        self.rotation.trace_add('write',lambda *_:self.schedule())
        self.after(80,self.poll)

    def orient(self):
        try:self.save_callback()
        except Exception as exc:self.info.configure(text='朝向暂未保存：'+str(exc))
        self.schedule()

    def turn(self,angle):
        self.rotation.set((int(self.rotation.get())+angle)%360);self.orient()

    def reset(self):
        self.yaw=-.8;self.pitch=.35;self.zoom=1.;self.schedule()

    def press(self,event):self.last=(event.x,event.y)

    def drag(self,event):
        if self.last:
            self.yaw+=(event.x-self.last[0])*.008
            self.pitch=max(-.2,min(1.2,self.pitch+(event.y-self.last[1])*.006))
        self.last=(event.x,event.y);self.schedule()

    def wheel(self,event):
        self.zoom=max(.25,min(5.,self.zoom*(1.12 if event.delta>0 else 1/1.12)));self.schedule()

    def load_project(self,project):
        self.generation+=1;generation=self.generation
        self.scene=None;self.canvas.delete('all');self.info.configure(text='正在读取几何与地块，进行预检…')
        def worker():
            try:
                import sc5_packager
                from preview_3d import load_scene
                config=sc5_packager.load_project(project)
                paths=[Path(p) for p in config.get('sources',[])]
                if (project/'input').is_dir():paths+=list((project/'input').rglob('*.lotm'))
                models=list(dict.fromkeys(p.resolve() for p in paths if p.suffix.lower()=='.lotm' and p.is_file()))
                if len(models)!=1:raise ValueError('请选择包含唯一 LOTM 的资产项目')
                self.messages.put((generation,load_scene(models[0]),None))
            except Exception as exc:self.messages.put((generation,None,str(exc)))
        threading.Thread(target=worker,daemon=True).start()

    def poll(self):
        try:
            while True:
                generation,scene,error=self.messages.get_nowait()
                if generation!=self.generation:continue
                if error:self.info.configure(text='预检未通过：'+error)
                else:self.scene=scene;self.reset()
        except queue.Empty:pass
        self.after(80,self.poll)

    def schedule(self):
        if self.pending:return
        self.pending=self.after(16,self.render)

    def render(self):
        self.pending=None
        if self.scene is None:return
        try:
            import time
            from preview_gpu import GPUPreview
            start=time.perf_counter()
            if self.gpu is None:self.gpu=GPUPreview(self.canvas)
            if self.gpu.scene is not self.scene:self.gpu.upload(self.scene)
            self.gpu.render(int(self.rotation.get()),self.yaw,self.pitch,self.zoom,self.building.get(),self.ground.get(),self.props.get(),self.lights.get())
            self.last_frame_ms=(time.perf_counter()-start)*1000
            s=self.scene;dims=' × '.join(f'{d:.2f}' for d in s['high']-s['low'])
            self.info.configure(text=f"模型尺寸 {dims} 米 | {s['triangleCount']:,} 三角形 | {s.get('propCount',0)} 个道具 | {s.get('lightCount',0)} 个光源 | 旋转 {self.rotation.get()}°\nGPU：{self.gpu.renderer} | 完整贴图 / 深度检测")
        except Exception as exc:self.info.configure(text='显卡预览初始化失败：'+str(exc))

    def cleanup(self,event):
        if event.widget is self and self.gpu:self.gpu.close()
