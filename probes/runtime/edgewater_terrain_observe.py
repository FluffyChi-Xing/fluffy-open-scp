"""Read terrain dimensions at the retail physics update during a user-started session."""
import argparse
import hashlib
import json
from pathlib import Path
import threading

import frida

from edgewater_native_sizing import EXE_SHA


AGENT = """
const exe = Process.mainModule;
if (Process.arch !== "ia32") throw new Error("expected x86 game");
const entry = exe.base.add(0x7e5880);
const footprint = Array.from(new Uint8Array(entry.readByteArray(7)));
if (footprint.join(",") !== "131,236,48,83,85,86,87")
    throw new Error("physics update code differs");
const observed = new Set();
const hook = Interceptor.attach(entry, {
    onEnter(args) {
        try {
            const face = args[0].add(0x14).readPointer().add(0xc).readU32();
            const eco = args[2].toUInt32();
            const raster = args[4].readPointer().readPointer();
            const width = raster.add(0xc).readU16();
            const height = raster.add(0xe).readU16();
            const half = exe.base.add(0xc3d43c).readFloat();
            if (face < 1 || eco < 1 || width < 1 || height < 1) return;
            const sample = {
                physicsFace: face, ecoSide: eco, controlWidth: width,
                controlHeight: height, flatScale: half,
                proposed512Compatible: width === height && width >= 512 && width % 512 === 0
            };
            const key = JSON.stringify(sample);
            if (!observed.has(key) && observed.size < 8) {
                observed.add(key);
                send(sample);
            }
        } catch (_) {
            // The game can release its terrain objects during a city transition.
        }
    }
});
rpc.exports = { stop() { hook.detach(); } };
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--seconds", type=int, default=30, choices=range(5, 61), metavar="5–60")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    device = frida.get_local_device()
    process = next((p for p in device.enumerate_processes(scope="full") if p.pid == args.pid), None)
    if process is None or process.name.lower() != "simcity.exe":
        parser.error("expected the PID of a running SimCity.exe")
    exe = Path(process.parameters["path"])
    if hashlib.sha256(exe.read_bytes()).hexdigest() != EXE_SHA:
        parser.error("stock retail executable SHA-256 mismatch")
    stop = threading.Event()
    rows = []
    errors = []

    def receive(message, _data):
        if message["type"] == "send":
            rows.append(message["payload"])
        elif message["type"] == "error":
            errors.append(message["description"])
            stop.set()

    with args.output.open("x", encoding="utf-8") as output:
        session = device.attach(args.pid)
        session.on("detached", lambda *_: stop.set())
        try:
            script = session.create_script(AGENT, runtime="qjs")
            script.on("message", receive)
            script.load()
            print("地形采集已就绪，请重新进入嘉納谷。", flush=True)
            try:
                stop.wait(args.seconds)
                if not session.is_detached and not errors:
                    script.exports_sync.stop()
            finally:
                if not session.is_detached:
                    script.unload()
        finally:
            if not session.is_detached:
                session.detach()
            json.dump({"pid": args.pid, "samples": rows, "errors": errors}, output, indent=2)
            output.write("\n")
    return 0 if rows and not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
