"""Publish compact, session-specific bounds for the WinForms inspector."""
import json
import os
import time
from pathlib import Path
from threading import Lock
from auto_index import index

lock = Lock()

def overlay(data):
    layout = index(data)
    controls = {c["domIndex"] for c in layout["controls"]}
    boxes = []
    for item in layout["viewportElements"]:
        i = item["domIndex"]
        n = data["nodes"][i]
        if i not in controls and n["tag"].lower() not in ("image","button","input","select","textarea","a"):
            continue
        boxes.append(dict(i=i, parent=n["parent"], tag=n["tag"], rect=item["clippedRect"], text=n["text"].strip()[:80]))
    return dict(pid=layout["pid"], page=data["page"], time=data["time"], sequence=data.get("sequence"),
                viewport=layout["viewport"], boxes=boxes)

def publish(folder, data):
    publish_view(folder,overlay(data))

def publish_view(folder,view):
    dest = Path(folder) / ("overlay-%d.json" % view["pid"])
    with lock:
        if dest.exists():
            old=json.loads(dest.read_bytes())
            if old["time"] > view["time"]:
                return
        temp=dest.with_suffix(".tmp")
        temp.write_text(json.dumps(view, ensure_ascii=False,separators=(",",":")), encoding="utf8")
        for attempt in range(5):
            try:
                os.replace(temp,dest)
                break
            except PermissionError:
                if attempt == 4: raise
                time.sleep(.01)
