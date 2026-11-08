from __future__ import annotations

import json
import shutil
import threading
import sys
import os
import vendor_tk as tk
from pathlib import Path
from vendor_tk import filedialog, messagebox, ttk

import sc5_packager
import behavior_profiles
from preview_panel import PreviewPanel


from menu_locations import CATEGORIES, MENU_LOCATIONS



class App(tk.Tk):
    """Window front end for the offline packager; it never writes to the game."""

    def __init__(self):
        super().__init__()
        self.title("SC5 Asset Packager 0.8.0")
        self.geometry("1320x920")
        self.minsize(1100, 790)
        self.project: Path | None = None
        self.preview_image: tk.PhotoImage | None = None
        self.name = tk.StringVar(value="New SC5 Asset")
        self.name_en = tk.StringVar(value="New_SC5_Asset")
        self.category = tk.StringVar(value="education")
        self.category_display=tk.StringVar(value=CATEGORIES["education"])
        self.version = tk.StringVar(value="0.1.0")
        self.cost = tk.StringVar(value="50000")
        self.maintenance = tk.StringVar(value="100")
        self.behavior_profile=tk.StringVar(value=behavior_profiles.LEGACY)
        self.behavior_display=tk.StringVar(value=behavior_profiles.PROFILES[behavior_profiles.LEGACY])
        self.geometry_strategy = tk.StringVar(value="stable_single_mesh")
        self.material_strategy = tk.StringVar(value="compact_2048")
        self.game_directory = tk.StringVar(value="D:/E/gameecg/SimCity：Cites of Tomorrow")
        self.menu_icon_path = ""
        self.busy = False
        self.rotation = tk.IntVar(value=180)
        self.output_directory = tk.StringVar(value='打开项目后显示 Package 成品目录')
        self.status = tk.StringVar(value="请打开或新建一个项目")
        self._build_ui()

    def _build_ui(self):
        style = ttk.Style(self)
        try:
            style.theme_use("vista")
        except tk.TclError:
            pass
        top = ttk.Frame(self, padding=12)
        top.pack(fill="x")
        logo_path=Path(__file__).resolve().parent/'brand_assets/logo.png'
        self.brand_logo_source=tk.PhotoImage(file=str(logo_path))
        self.brand_logo=self.brand_logo_source.subsample(6,6)
        ttk.Label(top,image=self.brand_logo).pack(side='left',padx=(0,12))
        self.iconphoto(True,self.brand_logo_source)
        ttk.Label(top, text="SC5 Asset Packager", font=("Segoe UI", 19, "bold")).pack(side="left")
        ttk.Label(top, text="离线资产打包工具", foreground="#4c6a85").pack(side="left", padx=14, pady=(6, 0))
        ttk.Button(top, text="打开项目 / 资产文件夹", command=self.open_project).pack(side="right", padx=4)
        ttk.Button(top, text="新建项目", command=self.new_project).pack(side="right", padx=4)

        form = ttk.LabelFrame(self, text="资产与游戏属性", padding=10)
        form.pack(fill="x", padx=12, pady=(0, 8))
        fields = [("中文名称", self.name, 28), ("英文名称", self.name_en, 22), ("版本", self.version, 10), ("造价", self.cost, 10), ("维护费/小时", self.maintenance, 10)]
        for col, (label, var, width) in enumerate(fields):
            ttk.Label(form, text=label).grid(row=0, column=col * 2, sticky="w", padx=(0, 5))
            ttk.Entry(form, textvariable=var, width=width).grid(row=0, column=col * 2 + 1, sticky="ew", padx=(0, 12))
        ttk.Label(form, text="菜单位置").grid(row=1, column=0, sticky="w", pady=(8, 0))
        self.category_combo=ttk.Combobox(form,textvariable=self.category_display,values=list(CATEGORIES.values()),state='readonly',width=24,height=15)
        self.category_combo.grid(row=1,column=1,sticky='w',pady=(8,0))
        self.category_combo.bind('<<ComboboxSelected>>',self.choose_category)
        ttk.Label(form, text="游戏菜单：").grid(row=1, column=2, sticky="e", pady=(8, 0))
        self.category_hint = ttk.Label(form, text="教育", foreground="#1e6aa8")
        self.category_hint.grid(row=1, column=3, columnspan=7, sticky="w", pady=(8, 0))
        ttk.Label(form, text="几何策略").grid(row=2, column=0, sticky="w", pady=(8, 0))
        ttk.Combobox(form, textvariable=self.geometry_strategy, state="readonly", width=22,
                     values=("stable_single_mesh",)).grid(row=2, column=1, sticky="w", pady=(8, 0))
        ttk.Label(form, text="材质策略").grid(row=2, column=2, sticky="e", pady=(8, 0))
        ttk.Combobox(form, textvariable=self.material_strategy, state="readonly", width=22,
                     values=("compact_2048", "stable_baseline")).grid(row=2, column=3, sticky="w", pady=(8, 0))
        self.category.trace_add('write',lambda *_:self.update_category_hint())
        self.update_category_hint()
        ttk.Label(form,text='建筑功能').grid(row=5,column=0,sticky='w',pady=(8,0))
        self.behavior_combo=ttk.Combobox(form,textvariable=self.behavior_display,values=list(behavior_profiles.PROFILES.values()),state='readonly',width=47)
        self.behavior_combo.grid(row=5,column=1,columnspan=4,sticky='w',pady=(8,0))
        self.behavior_combo.bind('<<ComboboxSelected>>',self.choose_behavior)
        ttk.Label(form,text='功能与菜单位置分别保存',foreground='#596878').grid(row=5,column=5,columnspan=5,sticky='w',pady=(8,0))
        self.behavior_hint=ttk.Label(form,text='',foreground='#896324',wraplength=1140)
        self.behavior_hint.grid(row=6,column=0,columnspan=10,sticky='w',pady=(5,0))
        self.behavior_profile.trace_add('write',lambda *_:self.update_behavior_hint())
        self.update_behavior_hint()
        ttk.Label(form, text="游戏目录").grid(row=3, column=0, sticky="w", pady=(8, 0))
        ttk.Entry(form, textvariable=self.game_directory).grid(row=3, column=1, columnspan=7, sticky="ew", pady=(8, 0))
        ttk.Button(form, text="选择目录", command=self.choose_game).grid(row=3, column=8, columnspan=2, pady=(8, 0))
        ttk.Label(form,text='成品目录').grid(row=4,column=0,sticky='w',pady=(8,0))
        ttk.Entry(form,textvariable=self.output_directory,state='readonly').grid(row=4,column=1,columnspan=7,sticky='ew',pady=(8,0))
        ttk.Button(form,text='打开输出文件夹',command=self.open_output_folder).grid(row=4,column=8,columnspan=2,pady=(8,0))
        for i in range(1, 10, 2):
            form.columnconfigure(i, weight=1)

        body = ttk.PanedWindow(self, orient="horizontal")
        body.pack(fill="both", expand=True, padx=12, pady=(0, 8))
        left = ttk.Frame(body, padding=8)
        right = ttk.Frame(body, padding=8)
        body.add(left, weight=1)
        body.add(right, weight=3)
        actions = ttk.Frame(left)
        actions.pack(fill="x", pady=(0, 8))
        ttk.Button(actions, text="导入资产文件", command=self.import_assets).pack(side="left", padx=(0, 6))
        ttk.Button(actions, text="刷新浏览", command=self.refresh_assets).pack(side="left", padx=6)
        ttk.Button(actions, text="检查资产", command=self.inspect).pack(side="left", padx=6)
        ttk.Button(actions, text="生成计划", command=lambda: self.build(True)).pack(side="left", padx=6)
        ttk.Button(actions, text="导出 Package", command=lambda: self.build(False)).pack(side="right")

        listing = ttk.LabelFrame(left, text="资产浏览")
        listing.pack(fill="both", expand=True)
        self.asset_tree = ttk.Treeview(listing, columns=("type", "size", "path"), show="headings")
        for key, title, width in (("type", "类型", 90), ("size", "大小", 110), ("path", "文件", 520)):
            self.asset_tree.heading(key, text=title)
            self.asset_tree.column(key, width=width, anchor="center" if key == "type" else "w")
        self.asset_tree.pack(side="left", fill="both", expand=True)
        scroll = ttk.Scrollbar(listing, orient="vertical", command=self.asset_tree.yview)
        scroll.pack(side="right", fill="y")
        self.asset_tree.configure(yscrollcommand=scroll.set)
        self.asset_tree.bind("<<TreeviewSelect>>", self._show_selected_preview)
        self.asset_tree.column('size',width=70)
        self.asset_tree.column('path',width=250)
        def place_divider(_event=None):
            if body.winfo_width()>800:
                body.after_idle(lambda:body.sashpos(0,490))
                body.unbind('<Configure>')
        body.bind('<Configure>',place_divider)

        preview_tabs = ttk.Notebook(right)
        preview_tabs.pack(fill='both',expand=True)
        self.viewer = PreviewPanel(preview_tabs,self.rotation,self._save_config)
        preview_tabs.add(self.viewer,text='三维朝向与预检')
        preview_box = ttk.Frame(preview_tabs)
        preview_tabs.add(preview_box,text='图片预览')
        self.preview_label = ttk.Label(preview_box, text="导入 PNG 预览图后将在这里显示", anchor="center", justify="center")
        self.preview_label.pack(fill="both", expand=True, padx=10, pady=10)
        self.preview_info = ttk.Label(preview_box, text="", wraplength=250, justify="left")
        self.preview_info.pack(fill="x", padx=10, pady=(0, 10))
        icon_tab=ttk.Frame(preview_tabs,padding=12)
        preview_tabs.add(icon_tab,text='菜单图标')
        ttk.Button(icon_tab,text='选择图标图片（PNG / JPG）',command=self.choose_menu_icon).pack(pady=8)
        ttk.Button(icon_tab,text='恢复自动图标',command=self.clear_menu_icon).pack(pady=4)
        self.icon_preview=ttk.Label(icon_tab,text='自动生成菜单图标',anchor='center')
        self.icon_preview.pack(pady=12)
        self.icon_info=ttk.Label(icon_tab,text='自定义图片居中裁成正方形，导出为 128 × 128 PNG。',wraplength=550)
        self.icon_info.pack(fill='x')
        bottom = ttk.LabelFrame(self, text="运行记录")
        bottom.pack(fill="x",side='bottom',before=body,padx=12,pady=(0,12))
        self.progress = ttk.Progressbar(self, mode='indeterminate')
        self.progress.pack(fill='x',side='bottom',before=body,padx=12,pady=(0,4))
        self.output = tk.Text(bottom, height=5, wrap="none", font=("Consolas", 9), state="disabled")
        self.output.pack(fill="both", expand=True, padx=6, pady=6)
        ttk.Label(self, textvariable=self.status, anchor="w", relief="sunken").pack(fill="x",side="bottom",before=top)

    def log(self, value: str):
        self.output.configure(state="normal")
        self.output.insert("end", value.rstrip() + "\n")
        self.output.see("end")
        self.output.configure(state="disabled")

    def open_project(self):
        if self.busy:
            return
        path = filedialog.askdirectory(title="选择打包项目或 Blender 导出资产文件夹")
        if not path:
            return
        try:
            target, created = sc5_packager.open_asset_folder(Path(path))
            config = sc5_packager.load_project(target)
            self._load_config(config)
            self.project = target
            self.output_directory.set(str((target/'output').resolve()))
            self.refresh_assets()
            self.viewer.load_project(target)
            self.status.set(f"已打开：{self.project}")
            self.log(f"已打开项目：{self.project}")
            if created:
                self.log("已识别 Blender 导出文件，自动创建 project.json；原资产保留在原位置。")
            if not config.get("backend"):
                self.log("已启用内置 LOTM9 编译器，支持 22 个菜单位置；新增位置需游戏内验收。")
        except Exception as exc:
            messagebox.showerror("打开失败", str(exc))

    def new_project(self):
        if self.busy:
            return
        parent = filedialog.askdirectory(title="选择项目父目录")
        if not parent:
            return
        safe = self.name_en.get().strip().replace(" ", "_") or "SC5_Asset"
        target = Path(parent) / safe
        try:
            args = type("Args", (), {"name": self.name.get().strip() or safe, "category": self.category.get()})()
            sc5_packager.init_project(target, args)
            self.project = target
            self.output_directory.set(str((target/'output').resolve()))
            self.name_en.set(safe)
            self.menu_icon_path=""
            self.show_menu_icon()
            self._save_config()
            self.refresh_assets()
            self.status.set(f"已创建：{target}")
            self.log(f"已创建项目：{target}")
        except Exception as exc:
            messagebox.showerror("创建失败", str(exc))

    def _load_config(self, config=None):
        if config is None:
            config = sc5_packager.load_project(self.project)
        settings = config.get("settings", {})
        self.name.set(config.get("name", ""))
        self.name_en.set(config.get("nameEn", config.get("name", "")))
        self.category.set(config.get("category", "education"))
        self.version.set(config.get("version", "0.1.0"))
        self.cost.set(str(settings.get("cost", 50000)))
        self.maintenance.set(str(settings.get("maintenancePerHour", 100)))
        self.behavior_profile.set(settings.get('behaviorProfile',behavior_profiles.LEGACY))
        self.geometry_strategy.set(settings.get("geometryStrategy", "stable_single_mesh"))
        self.material_strategy.set(settings.get("materialStrategy", "stable_baseline"))
        self.game_directory.set(config.get('gameDirectory', 'D:/E/gameecg/SimCity：Cites of Tomorrow'))
        self.menu_icon_path=config.get("menuIconImage", "")
        self.show_menu_icon()
        self.rotation.set(settings.get('rotationDegrees',180))

    def choose_category(self,_event=None):
        selected=self.category_display.get()
        for key,label in CATEGORIES.items():
            if label==selected:
                self.category.set(key);break

    def choose_behavior(self,_event=None):
        for key,label in behavior_profiles.PROFILES.items():
            if label==self.behavior_display.get():self.behavior_profile.set(key);break

    def update_behavior_hint(self):
        key=self.behavior_profile.get()
        self.behavior_display.set(behavior_profiles.PROFILES.get(key,key))
        if key==behavior_profiles.PARK:
            text='基础公园：接入原版公园规则及幸福度相关参数，保留自制外观。造价、维护费使用上方设置；实际城市效果待游戏验证。建议在测试城市新建。'
        elif key==behavior_profiles.LEGACY:
            text='旧版兼容：保留既有模板行为（仍含公园规则），不自动改成新功能。菜单位置不会赋予学位、供水等作用。'
        else:text='当前项目功能预设不受支持，请重新选择。'
        self.behavior_hint.configure(text=text)

    def update_category_hint(self):
        key=self.category.get();d=MENU_LOCATIONS.get(key)
        self.category_display.set(CATEGORIES.get(key,key))
        if d:
            status='已通过此前游戏测试' if d['runtimeVerified'] else '新增位置，待游戏测试'
            self.category_hint.configure(text=d['path']+' · '+status,wraplength=410)
        else:self.category_hint.configure(text='此旧配置暂不支持，请重新选择菜单位置')

    def choose_menu_icon(self):
        if not self.project:
            messagebox.showinfo('提示','请先打开或创建项目');return
        raw=filedialog.askopenfilename(title='选择游戏菜单图标',filetypes=[('图片','*.png *.jpg *.jpeg')])
        if not raw:return
        try:
            from menu_icons import save_project_icon
            self.menu_icon_path=save_project_icon(self.project,Path(raw))
            self._save_config();self.show_menu_icon()
        except Exception as exc:messagebox.showerror('图标读取失败',str(exc))

    def clear_menu_icon(self):
        self.menu_icon_path='';self._save_config();self.show_menu_icon()

    def show_menu_icon(self):
        if not self.menu_icon_path:
            self.icon_preview.configure(image='',text='自动生成菜单图标')
            self.icon_info.configure(text='自定义图片居中裁成正方形，导出为 128 × 128 PNG。');return
        try:
            from menu_icons import icon_png
            self.icon_image=tk.PhotoImage(data=icon_png(Path(self.menu_icon_path)))
            self.icon_preview.configure(image=self.icon_image,text='')
            self.icon_info.configure(text='已选择：'+self.menu_icon_path+'\n导出尺寸：128 × 128；居中裁切，不拉伸。')
        except Exception as exc:self.icon_preview.configure(image='',text='图标不可用：'+str(exc))

    def choose_game(self):
        path = filedialog.askdirectory(title="选择包含 SimCityData 的游戏根目录（仅供读取）")
        if path:
            self.game_directory.set(path)

    def open_output_folder(self):
        if not self.project:
            messagebox.showinfo('提示','请先打开项目')
            return
        folder=self.project/'output'
        folder.mkdir(exist_ok=True)
        os.startfile(str(folder.resolve()))

    def _save_config(self):
        if not self.project:
            return
        config = sc5_packager.load_project(self.project)
        config.update({"name": self.name.get().strip(), "nameEn": self.name_en.get().strip(), "category": self.category.get(), "version": self.version.get().strip(), "gameDirectory": self.game_directory.get().strip()})
        config.setdefault("settings", {}).update({"cost": int(float(self.cost.get() or 0)), "maintenancePerHour": int(float(self.maintenance.get() or 0)), "geometryStrategy": self.geometry_strategy.get(), "materialStrategy": self.material_strategy.get()})
        config['menuIconImage']=self.menu_icon_path
        config['settings']['rotationDegrees'] = int(self.rotation.get())
        config['settings']['nightEmissionPolicy'] = 'night-only'
        config['settings']['behaviorProfile']=self.behavior_profile.get()
        config['settings']['provideSimulationService']=self.behavior_profile.get()==behavior_profiles.PARK
        behavior_profiles.selection(config['settings'])
        (self.project / "project.json").write_text(json.dumps(config, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    def import_assets(self):
        if self.busy:
            return
        if not self.project:
            messagebox.showinfo("提示", "请先打开或创建一个项目")
            return
        paths = filedialog.askopenfilenames(title="选择资产文件（GLB、LOTM、Package、PNG 等）")
        if not paths:
            return
        input_dir = self.project / "input"
        input_dir.mkdir(parents=True, exist_ok=True)
        copied, failed = [], []
        for raw in paths:
            source = Path(raw)
            try:
                shutil.copy2(source, input_dir / source.name)
                copied.append(source.name)
            except Exception as exc:
                failed.append(f"{source.name}: {exc}")
        self._save_config()
        self.refresh_assets()
        self.viewer.load_project(self.project)
        self.log(f"导入完成：{len(copied)} 个文件")
        if failed:
            self.log("导入失败：" + "；".join(failed))
            messagebox.showwarning("部分文件未导入", "\n".join(failed))

    def refresh_assets(self):
        for item in self.asset_tree.get_children():
            self.asset_tree.delete(item)
        if not self.project:
            return
        input_dir = self.project / "input"
        if input_dir.is_dir():
            for path in sorted(input_dir.rglob("*")):
                if path.is_file():
                    self.asset_tree.insert("", "end", values=(path.suffix.lower().lstrip("."), self._size(path.stat().st_size), str(path.relative_to(self.project))))
        try:
            sources = sc5_packager.load_project(self.project).get("sources", [])
            for source in sources:
                path = Path(source)
                if path.is_file():
                    self.asset_tree.insert("", "end", values=(path.suffix.lower().lstrip("."), self._size(path.stat().st_size), str(path)))
        except Exception:
            pass

    @staticmethod
    def _size(value: int) -> str:
        for unit in ("B", "KB", "MB", "GB"):
            if value < 1024 or unit == "GB":
                return f"{value:.1f} {unit}" if unit != "B" else f"{value} B"
            value /= 1024
        return "?"

    def _show_selected_preview(self, _event=None):
        selection = self.asset_tree.selection()
        if not selection:
            return
        path = Path(self.asset_tree.set(selection[0], "path"))
        if not path.is_absolute() and self.project:
            path = self.project / path
        self.preview_image = None
        if path.suffix.lower() == ".png" and path.is_file():
            try:
                self.preview_image = tk.PhotoImage(file=str(path))
                while self.preview_image.width() > 420 or self.preview_image.height() > 300:
                    self.preview_image = self.preview_image.subsample(2, 2)
                self.preview_label.configure(image=self.preview_image, text="")
            except tk.TclError:
                self.preview_label.configure(image="", text="PNG 无法预览")
        else:
            self.preview_label.configure(image="", text="该文件无内置图像预览")
        self.preview_info.configure(text=f"文件：{path.name}\n类型：{path.suffix.lower().lstrip('.') or '未知'}\n位置：{path}")

    def inspect(self):
        if self.busy:
            return
        if not self.project:
            messagebox.showinfo("提示", "请先打开或创建项目")
            return
        try:
            self._save_config()
            report = sc5_packager.inspect_inputs(self.project)
            self.log(json.dumps(report, ensure_ascii=False, indent=2))
            self.status.set("检查完成")
        except Exception as exc:
            messagebox.showerror("检查失败", str(exc))

    def build(self, dry_run: bool):
        if self.busy:
            return
        if not self.project:
            messagebox.showinfo("提示", "请先打开或创建项目")
            return
        try:
            self._save_config()
        except Exception as exc:
            messagebox.showerror("保存配置失败", str(exc))
            return
        self.status.set("正在生成计划…" if dry_run else "正在导出 Package…")
        self.busy = True
        previous=self.project/'reports'/'compile-progress.json'
        if previous.is_file():previous.unlink()
        self.progress.start(15)
        self.after(400,self._poll_compile_progress)
        threading.Thread(target=self._build_worker, args=(dry_run,), daemon=True).start()

    def _poll_compile_progress(self):
        if not self.busy:return
        try:
            data=json.loads((self.project/'reports'/'compile-progress.json').read_text(encoding='utf-8'))
            self.status.set(f"{data['step']}/{data['total']} · {data['label']}")
        except (OSError,ValueError,KeyError):pass
        self.after(400,self._poll_compile_progress)

    def _build_worker(self, dry_run: bool):
        try:
            code = sc5_packager.build_project(self.project, dry_run)
            result_path = self.project / "reports" / ("build-plan.json" if dry_run else "build-result.json")
            self.after(0, lambda: self._build_done(code, result_path, dry_run))
        except Exception as exc:
            self.after(0, lambda error=str(exc): messagebox.showerror("导出失败", error))
            self.after(0, lambda: self.status.set("导出失败"))
            self.after(0, lambda: setattr(self, 'busy', False))
            self.after(0, self.progress.stop)

    def _build_done(self, code: int, result_path: Path, dry_run: bool):
        self.busy = False
        self.progress.stop()
        self.status.set("计划已生成" if dry_run and code == 0 else ("导出完成" if code == 0 else f"返回码 {code}"))
        self.log(f"{'计划' if dry_run else '导出'}完成，报告：{result_path}")
        if code == 0 and not dry_run:
            result=json.loads(result_path.read_text(encoding='utf-8'))
            packages=result.get('packages',[])
            paths='\n'.join(p['path'] for p in packages)
            location=result.get('menuLocation',{})
            if location.get('path'):self.log('菜单位置：'+location['path']+'（仅菜单入口）')
            behavior_path=self.project/'reports'/'native-backend-result.json'
            if behavior_path.exists():
                behavior=json.loads(behavior_path.read_text(encoding='utf-8')).get('behavior',{})
                if behavior:self.log('建筑功能：'+behavior.get('label','')+'；功能效果待实机验证')
            self.log('本次 Package 文件：\n'+paths)
            self.log('成品目录：'+str((self.project/'output').resolve()))
            messagebox.showinfo("导出完成", f"本次生成的 Package：\n{paths}\n\n可点击“打开输出文件夹”找到成品。\n验证报告：{result_path}")
        elif code != 0:
            path = self.project / 'reports' / 'compiler.log'
            if not path.exists():
                path = self.project / 'reports' / 'backend.log'
            detail = path.read_text(encoding='utf-8', errors='replace')[-6000:] if path.exists() else '没有生成日志'
            self.log(detail)
            messagebox.showerror('编译失败', f'未完成导出，请查看运行记录。\n日志：{path}\n{detail[-1200:]}')


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == '--compile-project':
        project = Path(sys.argv[2]).resolve()
        (project / 'reports').mkdir(exist_ok=True)
        with (project / 'reports' / 'compiler.log').open('w', encoding='utf-8') as log:
            sys.stdout = sys.stderr = log
            try:
                from backend_compile import compile_project
                compile_project(project)
            except Exception:
                import traceback
                traceback.print_exc()
                raise SystemExit(1)
    elif len(sys.argv)==4 and sys.argv[1]=='--verify-preview':
        # Explicit diagnostic mode validates the frozen EXE's real Tk/WGL surface.
        import time
        app=App();app.project=Path(sys.argv[2]).resolve();report_path=Path(sys.argv[3]).resolve()
        try:
            app._load_config();app.viewer.load_project(app.project)
            deadline=time.monotonic()+60
            while (not app.viewer.gpu or app.viewer.gpu.scene is None) and time.monotonic()<deadline:
                app.update();time.sleep(.02)
            viewer=app.viewer
            if not viewer.gpu or viewer.gpu.scene is None:raise RuntimeError(viewer.info.cget('text'))
            times=[]
            for i in range(30):
                start=time.perf_counter();viewer.gpu.render(int(app.rotation.get()),-.8+i*.02,.35,1)
                viewer.gpu.gl.glFinish();times.append((time.perf_counter()-start)*1000)
            result={'ok':True,'renderer':viewer.gpu.renderer,'triangles':viewer.scene['triangleCount'],'props':viewer.scene.get('propCount',0),'lights':viewer.scene.get('lightCount',0),
                    'meanFrameMs':sum(times)/len(times),'menuIconLoaded':bool(app.menu_icon_path),
                    'menuLocation':app.category.get(),'menuLocationLabel':app.category_display.get(),'menuLocationChoices':len(app.category_combo.cget('values')),
                    'behaviorProfile':app.behavior_profile.get(),'behaviorChoices':len(app.behavior_combo.cget('values')),
                    'glError':viewer.gpu.gl.glGetError(),'frozen':bool(getattr(sys,'frozen',False))}
        except Exception as exc:result={'ok':False,'error':str(exc)}
        finally:app.destroy()
        report_path.write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
        if not result['ok']:raise SystemExit(1)
    else:
        App().mainloop()

