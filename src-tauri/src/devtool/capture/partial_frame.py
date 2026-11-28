"""Read only complete control/element records before an interrupted document-tree upload."""
import json

def recover(body, peer, allowed):
    # v4 writes the mapped controls and the element table before documentTree.
    # Decode top-level members sequentially so strings cannot masquerade as keys.
    text = body.decode("utf-8",errors="ignore")
    decoder = json.JSONDecoder()
    if not text.startswith("{"):
        return None
    fields = {}
    pos = 1
    try:
        while pos < len(text):
            key,pos = decoder.raw_decode(text,pos)
            if not isinstance(key,str) or text[pos:pos+1] != ":":
                return None
            pos += 1
            if key == "nodes":
                break
            fields[key],pos = decoder.raw_decode(text,pos)
            if text[pos:pos+1] != ",":
                return None
            pos += 1
        if key != "nodes" or text[pos:pos+1] != "[" or fields.get("page","").lower() not in allowed:
            return None
        pos += 1
        rows = []
        while pos < len(text) and len(rows)<16384:
            try:
                row,end = decoder.raw_decode(text,pos)
            except ValueError:
                break
            i=len(rows)
            if not isinstance(row,dict) or not (row["parent"]==-1 if i==0 else 0<=row["parent"]<i):
                return None
            rows.append(row)
            if text[end:end+1] != ",":
                break
            pos=end+1
        if not rows or rows[0]["tag"]!="html":
            return None
        controls=[c for c in fields["scruiControls"] if 0<=c["domIndex"]<len(rows)]
        if len(controls)<2:
            return None
        return dict(fields,nodes=rows,scruiControls=controls,truncated=True,observedPeer=peer,
                    sequence="partial",partial=True)
    except (ValueError, KeyError, TypeError, UnboundLocalError):
        return None
