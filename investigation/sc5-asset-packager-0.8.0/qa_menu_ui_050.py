from pathlib import Path
import json
from sc5_packager_gui import App
from menu_locations import CATEGORIES
root=Path(__file__).parent
project=root/'Projects/MenuQA050'
original=(project/'project.json').read_text(encoding='utf-8')
app=App()
try:
 app.project=project;app._load_config();app.update()
 settings=json.loads(original)['settings']
 for key,label in CATEGORIES.items():
  app.category_display.set(label);app.choose_category();app._save_config()
  saved=json.loads((project/'project.json').read_text(encoding='utf-8'))
  assert saved['category']==key
  assert saved['settings']==settings
  app._load_config();assert app.category_display.get()==label
  assert ' / ' in app.category_hint.cget('text')
 assert len(app.category_combo.cget('values'))==22
 result={'ok':True,'choices':22,'allChineseSelectionsSavedAndReopened':True,'settingsUnchanged':True}
 (root/'release/0.5.0/menu-ui-qa.json').write_text(json.dumps(result,indent=2),encoding='utf-8');print(result)
finally:
 app.destroy();(project/'project.json').write_text(original,encoding='utf-8')
