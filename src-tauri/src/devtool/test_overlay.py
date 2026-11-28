"""Exercise the real WinForms helper without starting or controlling a game."""
import json
import os
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import time


def run():
    with tempfile.TemporaryDirectory() as folder:
        root = Path(folder)
        requests = root / "requests"
        requests.mkdir()
        feed = root / "overlay.json"
        child = subprocess.Popen([
            "powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass",
            "-File", str(Path(__file__).with_name("overlay.ps1")),
            "-GamePid", str(os.getpid()), "-Feed", str(feed), "-Requests", str(requests),
        ], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf8",
            creationflags=0x08000000)
        events = queue.Queue()
        def read():
            for line in child.stdout:
                events.put(json.loads(line))
        threading.Thread(target=read, daemon=True).start()
        def expect(kind):
            event = events.get(timeout=15)
            assert event["kind"] == kind, event
            return event
        def publish(text):
            pending = root / "frame.pending"
            pending.write_text(text, encoding="utf8")
            os.replace(pending, feed)
        try:
            expect("ready")  # Missing first capture is a normal waiting state.
            publish("{invalid")
            failure = expect("read-error")
            assert failure["message"]
            publish(json.dumps(dict(pid=os.getpid(), time=int(time.time()*1000)+1000,
                                    sequence=1, viewport=[100, 100], boxes=[])))
            expect("snapshot")
            print("PASS overlay: missing feed waits, malformed feed reports detail, valid feed recovers")
        finally:
            child.terminate()
            child.communicate(timeout=10)


if __name__ == "__main__":
    run()
