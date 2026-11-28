from http.server import BaseHTTPRequestHandler,ThreadingHTTPServer
from pathlib import Path
import json,secrets,time,sys
d=Path(__file__).resolve().parent
def credential(folder):
    file=folder/"session-key"
    if not file.exists():file.write_text(secrets.token_hex(24),encoding="ascii")
    return file.read_text(encoding="ascii").strip()
MENU_URI="game://localhost/resource/0x40464200!0x82c29d9b.0xdd6233d6"
def server(folder,port,expected_game=None,allowed_pages=None,quiet_legacy=False,exclusive=False):
    allowed=frozenset(allowed_pages if allowed_pages is not None else (MENU_URI,))
    from archive_store import Archive
    archive=Archive(folder)
    key=credential(folder)
    class Receiver(BaseHTTPRequestHandler):
        def log_message(self,*args):pass
        def do_POST(self):
            if self.path not in ("/"+key,"/"+key+"/demand") or self.headers.get("Host")!="127.0.0.1:"+str(self.server.server_port):
                self.send_error(404);return
            peer=None
            if expected_game is not None:
                from peer import client_identity
                peer=client_identity(self.client_address[1],self.server.server_port)
                if not peer or Path(peer["executable"])!=expected_game:
                    self.send_error(403);return
            try:
                length=int(self.headers.get("Content-Length",0))
                if not 0<length<=8*1024*1024:
                    if peer:
                        with (folder/"upload-failures.jsonl").open("a",encoding="utf8") as log:
                            print(json.dumps(dict(pid=peer["pid"],declared=length,phase="length",time=time.time())),file=log)
                    self.send_error(413);return
                parts=bytearray()
                while len(parts)<length:
                    try:
                        block=self.rfile.read1(min(65536,length-len(parts)))
                    except ConnectionError:
                        break
                    if not block:break
                    parts.extend(block)
                body=bytes(parts)
                if quiet_legacy and self.path=="/"+key:
                    # v4 retries on failure or after 5s. Acknowledge before its timeout.
                    time.sleep(3.4)
                    self.send_response(204);self.send_header("Access-Control-Allow-Origin","*");self.end_headers()
                    return
                try:
                    data=json.loads(body)
                except (ValueError,UnicodeError):
                    if peer and self.path.endswith("/demand"):
                        from partial_frame import recover
                        from overlay_feed import publish
                        partial=recover(body,peer,allowed)
                        diagnostic=dict(pid=peer["pid"],declared=length,received=len(body),time=time.time())
                        if partial:
                            name="partial-"+str(time.time_ns())+".json"
                            (folder/name).write_text(json.dumps(partial,ensure_ascii=False),encoding="utf8")
                            publish(folder,partial)
                            diagnostic.update(file=name,page=partial["page"],elements=len(partial["nodes"]))
                        with (folder/"upload-failures.jsonl").open("a",encoding="utf8") as log:
                            print(json.dumps(diagnostic),file=log)
                    self.close_connection=True
                    return
                if data.get("op")=="bounds":
                    if not peer or data.get("page","").lower() not in allowed:
                        self.send_error(403);return
                    from overlay_feed import publish_view
                    if not isinstance(data["boxes"],list) or len(data["boxes"])>32768:
                        self.send_error(400);return
                    from validate_bounds import validate
                    validate(data)
                    data["pid"]=peer["pid"]
                    publish_view(folder,data)
                    (folder/("bounds-"+str(time.time_ns())+".json")).write_bytes(body)
                    self.send_response(204);self.send_header("Access-Control-Allow-Origin","*");self.end_headers()
                    return
                if data.get("op")=="ready":
                    page=data.get("page","").lower()
                    if page not in allowed:
                        self.send_error(422);return
                    import hashlib
                    request=folder/("request-"+hashlib.sha256(page.encode()).hexdigest())
                    try:
                        request.unlink()
                        status=200
                    except FileNotFoundError:
                        status=204
                    self.send_response(status);self.send_header("Access-Control-Allow-Origin","*");self.end_headers()
                    return
                rows=data["nodes"]
                if not 0<len(rows)<=16384 or rows[0]["tag"]!="html":raise ValueError()
                for i,n in enumerate(rows):
                    if not (n["parent"]==-1 if i==0 else 0<=n["parent"]<i):raise ValueError()
                if peer is not None:
                    if data.get("page","").lower() not in allowed:
                        self.send_error(422);return
                    data["observedPeer"]=peer
                if peer is not None and data.get("version")==4 and "documentTree" in data:
                    archive.store(data)
                    if data.get("watchMode") == "event":
                        from overlay_feed import publish
                        publish(folder,data)
                else:
                    name="snapshot-"+str(time.time_ns())+".json"
                    tmp=folder/(name+".tmp")
                    tmp.write_text(json.dumps(data,ensure_ascii=False),encoding="utf8")
                    tmp.replace(folder/name)
            except (KeyError,TypeError,ValueError):
                self.send_error(400);return
            self.send_response(204);self.send_header("Access-Control-Allow-Origin","*");self.end_headers()
    import socket
    class CaptureServer(ThreadingHTTPServer):
        allow_reuse_address = not exclusive
        def server_bind(self):
            if exclusive and hasattr(socket,"SO_EXCLUSIVEADDRUSE"):
                self.socket.setsockopt(socket.SOL_SOCKET,socket.SO_EXCLUSIVEADDRUSE,1)
            self.socket.setsockopt(socket.SOL_SOCKET,socket.SO_RCVBUF,2*1024*1024)
            super().server_bind()
    return CaptureServer(("127.0.0.1",port),Receiver)
if __name__=="__main__":server(d,47839).serve_forever()
