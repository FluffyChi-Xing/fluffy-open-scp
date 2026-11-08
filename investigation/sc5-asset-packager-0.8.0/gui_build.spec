# -*- mode: python ; coding: utf-8 -*-
from pathlib import Path
from PyInstaller.building.build_main import Analysis, PYZ, EXE

root = Path(SPECPATH)
runtime = Path(r"C:/Users/thunderobot/.cache/codex-runtimes/codex-primary-runtime/dependencies/python")
tk_dir = runtime / "tcl"
binaries = [
    (str(runtime / "DLLs" / "_tkinter.pyd"), "."),
    (str(runtime / "DLLs" / "tcl86t.dll"), "."),
    (str(runtime / "DLLs" / "tk86t.dll"), "."),
]
datas = [
    (str(root / "behavior_native_profile.json"), "."),
    (str(root / "brand_assets"), "brand_assets"),
    (str(root / "light_native_profile.json"), "."),
    (str(root / "prop_assets"), "prop_assets"),
    (str(root / "engine"), "engine"),
    (str(root / "project_schema.json"), "."),
    (str(tk_dir / "tcl8.6"), "tcl/tcl8.6"),
    (str(tk_dir / "tk8.6"), "tcl/tk8.6"),
]
a = Analysis(
    [str(root / "sc5_packager_gui.py")],
    pathex=[str(root)], binaries=binaries, datas=datas,
    hiddenimports=["numpy", "PIL.Image", "PIL.ImageDraw", "PIL.ImageFont", "PIL.ImageOps", "PIL.PngImagePlugin", "PIL.JpegImagePlugin", "sqlite3", "vendor_tk", "vendor_tk.ttk", "vendor_tk.filedialog", "vendor_tk.messagebox", "vendor_tk.simpledialog", "vendor_tk.constants", "vendor_tk.font", "vendor_tk.commondialog"],
    hookspath=[], hooksconfig={}, runtime_hooks=[str(root / "tk_runtime_hook.py")], excludes=["tkinter"], noarchive=False,
)
pyz = PYZ(a.pure)
exe = EXE(pyz, a.scripts, a.binaries, a.datas, [], name="SC5 Asset Packager", debug=False, bootloader_ignore_signals=False, strip=False, upx=False, console=False)
