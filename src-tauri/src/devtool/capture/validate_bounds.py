"""Validate captured hierarchy before exposing it to the desktop tree."""
import math

def validate(data):
    viewport=data["viewport"]
    if len(viewport)!=2 or not all(isinstance(v,(int,float)) and math.isfinite(v) and v>0 for v in viewport):
        raise ValueError("invalid viewport")
    nodes=data.get("nodes")
    if nodes is not None:
        if not 0<len(nodes)<=32768:raise ValueError("invalid node count")
        for i,n in enumerate(nodes):
            if n["i"]!=i or not isinstance(n["tag"],str):raise ValueError("invalid node identity")
            if not (n["parent"]==-1 if i==0 else isinstance(n["parent"],int) and 0<=n["parent"]<i):
                raise ValueError("invalid ancestry")
    boxes=data["boxes"]
    if not isinstance(boxes,list) or len(boxes)>32768:raise ValueError("invalid boxes")
    seen=set()
    for box in boxes:
        i=box["i"]
        if not isinstance(i,int) or i<0 or i in seen:raise ValueError("duplicate bounds")
        seen.add(i)
        if nodes is not None and (i>=len(nodes) or box["parent"]!=nodes[i]["parent"] or box["tag"]!=nodes[i]["tag"]):
            raise ValueError("bounds mismatch")
        rect=box["rect"]
        if len(rect)!=4 or not all(isinstance(v,(int,float)) and math.isfinite(v) for v in rect):
            raise ValueError("invalid rectangle")
        if not (0<=rect[0]<rect[2]<=viewport[0] and 0<=rect[1]<rect[3]<=viewport[1]):
            raise ValueError("rectangle outside viewport")
