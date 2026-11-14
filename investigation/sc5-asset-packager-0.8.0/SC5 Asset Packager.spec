# -*- mode: python ; coding: utf-8 -*-
from PyInstaller.utils.hooks import collect_all
from pathlib import Path

py_root = Path(r'C:\Users\thunderobot\.cache\codex-runtimes\codex-primary-runtime\dependencies\python')
tk_datas, tk_binaries, tk_hidden = collect_all('tkinter')
tk_binaries += [
    (str(py_root / 'DLLs' / '_tkinter.pyd'), '.'),
    (str(py_root / 'DLLs' / 'tcl86t.dll'), '.'),
    (str(py_root / 'DLLs' / 'tk86t.dll'), '.'),
]
tk_datas += [(str(py_root / 'tcl'), 'tcl')]


a = Analysis(
    ['sc5_packager_gui.py'],
    pathex=[],
    binaries=tk_binaries,
    datas=[('project_schema.json', '.')] + tk_datas,
    hiddenimports=['tkinter', 'tkinter.ttk', 'tkinter.filedialog', 'tkinter.messagebox'] + tk_hidden,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
    optimize=0,
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='SC5 Asset Packager',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=False,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
