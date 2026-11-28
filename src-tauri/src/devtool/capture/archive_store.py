"""Each delta refers directly to an immutable baseline, never to another delta."""
import gzip,json,time
from pathlib import Path

def atomic(path,raw):
    temp=path.with_suffix(path.suffix+".tmp")
    temp.write_bytes(raw)
    temp.replace(path)

class Archive:
    def __init__(self,root):
        self.root=Path(root)
        self.bases={}
    def store(self,data):
        key=(data["observedPeer"]["pid"],data["page"])
        base=self.bases.get(key)
        name="snapshot-"+str(time.time_ns())
        identical=("nodes","scruiControls","viewport","truncated","controlTraversalLimited")
        if base and all(data[k]==base[1][k] for k in identical):
            a=base[1]["documentTree"]["rows"]
            b=data["documentTree"]["rows"]
            if len(a)==len(b):
                changes=[[i,n] for i,n in enumerate(b) if a[i]!=n]
                if len(changes)<max(1,len(b)//3):
                    packet=dict(base=base[0],meta={k:v for k,v in data.items() if k not in ("nodes","scruiControls","documentTree")},
                                tree={k:v for k,v in data["documentTree"].items() if k!="rows"},changes=changes)
                    path=self.root/(name+".delta.gz")
                    atomic(path,gzip.compress(json.dumps(packet,ensure_ascii=False,separators=(",",":")).encode(),compresslevel=3))
                    return path
        path=self.root/(name+".json")
        atomic(path,json.dumps(data,ensure_ascii=False,separators=(",",":")).encode())
        self.bases[key]=(path.name,data)
        return path

def read(path):
    path=Path(path)
    if path.suffix==".json":return json.loads(path.read_bytes())
    record=json.loads(gzip.decompress(path.read_bytes()))
    base=json.loads(path.with_name(record["base"]).read_bytes())
    rows=base["documentTree"]["rows"]
    for i,row in record["changes"]:rows[i]=row
    return dict(record["meta"],nodes=base["nodes"],scruiControls=base["scruiControls"],documentTree=dict(record["tree"],rows=rows))

if __name__=="__main__":
    import argparse
    p=argparse.ArgumentParser()
    p.add_argument("snapshot",type=Path);p.add_argument("output",type=Path)
    args=p.parse_args()
    with args.output.open("x",encoding="utf8") as f:json.dump(read(args.snapshot),f,ensure_ascii=False)
