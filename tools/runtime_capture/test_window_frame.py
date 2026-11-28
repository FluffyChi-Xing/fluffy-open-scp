import importlib.util
from pathlib import Path
from PIL import Image,ImageDraw
source=Path(__file__).with_name('window_frame.py')
spec=importlib.util.spec_from_file_location('window_frame',source);module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
black=Image.new('RGB',(400,240));ImageDraw.Draw(black).rectangle((0,0,399,18),fill='white')
assert module.is_black_content(black), 'Window chrome must not hide a black client'
real=Image.new('RGB',(400,240),'#447799')
assert not module.is_black_content(real)
print('PASS black client detection with painted window chrome; normal image retained')
