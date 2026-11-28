"""Index a captured view using only its tree, viewport and native mappings."""
from pathlib import Path
import argparse,json,time,os
def index(s):
    nodes=s["nodes"];width,height=s["viewport"]
    mapped={}
    for c in s["scruiControls"]:
        i=c["domIndex"]
        if not 0<=i<len(nodes):raise ValueError("control outside tree")
        mapped.setdefault(i,[]).append(c)
    owners=[];names={};elements=[]
    for i,n in enumerate(nodes):
        p=n["parent"]
        if not (p==-1 if i==0 else 0<=p<i):raise ValueError("invalid parent")
        owner=i if i in mapped else owners[p] if p>=0 else None
        owners.append(owner)
        text=n["text"].strip()
        if text and owner is not None:
            labels=names.setdefault(owner,[])
            if text not in labels and len(labels)<6:labels.append(text)
        l,t,r,b=n["rect"];rect=[max(l,0),max(t,0),min(r,width),min(b,height)]
        if rect[0]<rect[2] and rect[1]<rect[3]:
            elements.append({"domIndex":i,"parent":p,"owner":owner,"clippedRect":rect})
    return dict(page=s["page"],pid=s["observedPeer"]["pid"],viewport=s["viewport"],truncated=s["truncated"],controlTraversalLimited=s["controlTraversalLimited"],domCount=len(nodes),controlMappings=len(s["scruiControls"]),viewportElements=elements,controls=[dict(domIndex=i,mappings=mapped[i],directLabels=names.get(i,[])) for i in sorted(mapped)])
def process(root):
    dest=root/"auto-index";dest.mkdir(exist_ok=True)
    for file in sorted(root.glob("snapshot-*.json")):
        out=dest/file.name
        if out.exists():continue
        data=index(json.loads(file.read_bytes()))
        data["source"]=file.name
        tmp=out.with_suffix(".%d.tmp"%os.getpid())
        tmp.write_text(json.dumps(data,ensure_ascii=False,separators=(",",":")),encoding="utf8")
        tmp.replace(out)
if __name__=="__main__":
    ap=argparse.ArgumentParser()
    ap.add_argument("--watch",action="store_true")
    args=ap.parse_args();live=Path(__file__).resolve().parent/"live"
    while True:
        process(live)
        if not args.watch:break
        time.sleep(1)
