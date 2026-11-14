"""Build the archived GUI with the Python chosen by the developer."""
from pathlib import Path
import sys,re,subprocess
root=Path(__file__).resolve().parent
if sys.platform!='win32':raise SystemExit('Please build this Windows application on Windows.')
import _tkinter
runtime=Path(sys.base_prefix)
required=[runtime/'tcl/tcl8.6/init.tcl',runtime/'tcl/tk8.6/tk.tcl',runtime/'DLLs/_tkinter.pyd',runtime/'DLLs/tcl86t.dll',runtime/'DLLs/tk86t.dll']
missing=[str(p) for p in required if not p.is_file()]
if missing:raise SystemExit('Use full Windows Python 3.12 x64 with Tcl/Tk installed. Missing:\n'+'\n'.join(missing))
spec=(root/'gui_build.spec').read_text(encoding='utf-8')
spec='import sys\n'+re.sub(r'^runtime = .*$', 'runtime = Path(sys.base_prefix)',spec,flags=re.M)
portable=root/'gui_source_build.spec';portable.write_text(spec,encoding='utf-8')
raise SystemExit(subprocess.call([sys.executable,'-m','PyInstaller','--noconfirm','--distpath',str(root/'release/0.8.0'),str(portable)],cwd=root))
