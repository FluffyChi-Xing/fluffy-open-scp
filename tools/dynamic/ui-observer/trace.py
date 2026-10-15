"""Local UI trace collector and reversible, fingerprinted scrui overlay workflow."""
import argparse
from collections import Counter
import hashlib
import hmac
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import sys
import threading
import time
import urllib.request
from http.server import BaseHTTPRequestHandler, HTTPServer

ROOT = Path(__file__).resolve().parents[3]
EXE_HASH = "2b0aff527baa3aa536aca6c68332c208d1dbd904695f0f985d17d56b128d48ab"
SCRUI_HASH = "c36316891708c6bf89c9530714704446dd20104f9e40420c461c6972257e776a"
OVERLAY = "000_openscp_ui_observer.package"
SCHEMA = "openscp.ui-trace.v1"
TEXT = {
    "zh-CN": {
        "description": "SimCity 只读 UI 观察器：准备覆盖包、安装、接收日志、检查、撤回。",
        "prepare": "构建覆盖包；不修改游戏", "install": "安装已验证覆盖包（游戏须关闭）",
        "serve": "在 127.0.0.1 接收日志", "status": "读取本地捕获摘要", "uninstall": "按哈希撤回覆盖包",
        "directory": "本次观察会话目录", "exe": "受支持的 SimCity.exe 完整路径",
        "port": "本机接收端口", "lang": "输出语言", "ready": "覆盖包已生成并回读验证",
        "installed": "覆盖包已安装；下次启动游戏加载", "removed": "覆盖包已撤回",
        "listening": "接收器已启动，等待游戏页面连接", "error": "操作失败",
        "start": "在后台启动接收器", "stop": "停止接收器", "stopped": "接收器已停止",
        "report": "汇总命令、事件、模板与城市选择证据",
        "probe": "只读检查进程私有内存中的脚本标记", "pid": "当前 SimCity 进程 ID",
        "stage": "实验阶段；observe 已发现黑屏，禁止安装",
    },
    "en-US": {
        "description": "SimCity read-only UI observer: prepare, install, collect, inspect and remove.",
        "prepare": "Build an overlay without changing the game", "install": "Install verified overlay (game must be closed)",
        "serve": "Collect traces on 127.0.0.1", "status": "Read local capture summary", "uninstall": "Remove overlay with hash verification",
        "directory": "Capture session directory", "exe": "Full path to supported SimCity.exe",
        "port": "Local collector port", "lang": "Output language", "ready": "Overlay built and roundtrip verified",
        "installed": "Overlay installed; it loads on next game start", "removed": "Overlay removed",
        "listening": "Collector started; waiting for the game page", "error": "Operation failed",
        "start": "Start collector in background", "stop": "Stop collector", "stopped": "Collector stopped",
        "report": "Summarize commands, events, templates and city selection evidence",
        "probe": "Read-only scan for script markers in private process memory", "pid": "Current SimCity process ID",
        "stage": "Experiment stage; observe has a known black screen regression and cannot be installed",
    },
}


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, value):
    path = Path(path)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, ensure_ascii=False), encoding="utf-8")
    os.replace(temporary, path)


def read_manifest(directory):
    return json.loads((directory / "manifest.json").read_text(encoding="utf-8"))


def ensure_game_closed():
    if os.name == "nt":
        result = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command",
                                 "if (Get-Process -Name SimCity -ErrorAction SilentlyContinue) { exit 1 }"],
                                capture_output=True, check=False)
        if result.returncode:
            raise ValueError("game_running: close SimCity before changing its package set")


def package_tool(*args):
    binary = ROOT / "target" / "debug" / "examples" / ("ui_observer_package.exe" if os.name == "nt" else "ui_observer_package")
    result = subprocess.run([str(binary), *map(str, args)], capture_output=True, check=True, encoding="utf-8")
    return result.stdout.splitlines()


def prepare(directory, exe, port, stage="baseline"):
    exe = exe.resolve(strict=True)
    if sha(exe) != EXE_HASH:
        raise ValueError("unsupported_exe_hash")
    app = exe.parent.parent / "SimCityData" / "SimCity_App.package"
    if not app.is_file() or not 1024 <= port <= 65535:
        raise ValueError("invalid_app_or_port")
    directory.mkdir(parents=True, exist_ok=False)
    baseline = directory / "scrui.baseline.js"
    package_tool(app, baseline)
    if sha(baseline) != SCRUI_HASH:
        raise ValueError("unsupported_scrui_hash")
    token = secrets.token_hex(32)
    build = secrets.token_hex(8)
    config = {"build": build, "endpoint": f"http://127.0.0.1:{port}/capture/{token}"}
    suffix = directory / "observer.configured.js"
    script = ""
    if stage == "marker":
        script = "var OpenSCP_UI_Probe_V1=" + json.dumps({"build": build, "stage": stage}) + ";\n"
    elif stage == "observe":
        script = "this.__OpenSCPObserverConfig=" + json.dumps(config) + ";\n" + Path(__file__).with_name("observer.js").read_text(encoding="utf-8")
    suffix.write_text(script, encoding="utf-8")
    overlay = directory / OVERLAY
    package_tool(app, baseline, suffix, overlay)
    manifest = {"schema": SCHEMA, "build": build, "stage": stage, "exe": str(exe), "exeSha256": EXE_HASH,
                "app": str(app), "scruiSha256": SCRUI_HASH, "observerSha256": sha(suffix),
                "overlaySha256": sha(overlay), "port": port, "token": token,
                "target": str(app.parent / OVERLAY), "installed": False}
    save(directory / "manifest.json", manifest)
    return {"build": build, "stage": stage, "overlay": str(overlay), "sha256": manifest["overlaySha256"]}


def install(directory):
    m = read_manifest(directory)
    # Actual retail regression: 000_ + full observer caused a black screen with
    # music; removal restored the menu. No override until the cause is resolved.
    if m.get("stage", "observe") not in ("baseline", "marker"):
        raise ValueError("known_black_screen_regression: full observer installation disabled")
    ensure_game_closed()
    target = Path(m["target"])
    expected_dir = Path(m["exe"]).parent.parent / "SimCityData"
    if target.parent.resolve() != expected_dir.resolve() or target.name not in (OVERLAY, "zzz_openscp_ui_observer.package"):
        raise ValueError("unexpected_target")
    source = directory / target.name
    if sha(m["exe"]) != m["exeSha256"] or sha(source) != m["overlaySha256"]:
        raise ValueError("fingerprint_mismatch")
    # Scan exact TGI in all packages instead of assuming filename sort establishes precedence.
    roots = [target.parent, expected_dir.parent / "SimCityUserData" / "Packages"]
    conflicts = [p for root in roots if root.is_dir() for p in package_tool("scan", root)
                 if Path(p).resolve() != Path(m["app"]).resolve()]
    if conflicts:
        raise ValueError("scrui_conflict: " + json.dumps(conflicts, ensure_ascii=False))
    # Re-extract to ensure the source package has not changed since preparation.
    check = directory / "scrui.install-check.js"
    if check.exists():
        check.unlink()
    package_tool(m["app"], check)
    if sha(check) != m["scruiSha256"]:
        raise ValueError("source_changed")
    with target.open("xb") as out, source.open("rb") as incoming:
        shutil.copyfileobj(incoming, out)
    if sha(target) != m["overlaySha256"]:
        raise ValueError("installed_hash_mismatch")
    m["installed"] = True
    save(directory / "manifest.json", m)
    return {"target": str(target), "build": m["build"]}


def uninstall(directory):
    ensure_game_closed()
    m = read_manifest(directory)
    target = Path(m["target"])
    expected_dir = Path(m["exe"]).parent.parent / "SimCityData"
    if target.parent.resolve() != expected_dir.resolve() or target.name not in (OVERLAY, "zzz_openscp_ui_observer.package"):
        raise ValueError("unexpected_target")
    if target.exists():
        if sha(target) != m["overlaySha256"]:
            raise ValueError("installed_file_changed: preserving file")
        target.unlink()
    m["installed"] = False
    save(directory / "manifest.json", m)
    return {"target": str(target)}


def health(manifest):
    try:
        with urllib.request.urlopen(f'http://127.0.0.1:{manifest["port"]}/health', timeout=1) as response:
            result = json.load(response)
        if result.get("build") == manifest["build"] and result.get("schema") == SCHEMA:
            return result
    except (OSError, ValueError):
        pass
    return None


def start(directory, lang):
    manifest = read_manifest(directory)
    if health(manifest):
        return {"port": manifest["port"], "alreadyRunning": True}
    with (directory / "collector.log").open("ab") as log:
        process = subprocess.Popen([sys.executable, "-u", str(Path(__file__).resolve()), "--lang", lang, "serve", str(directory)],
                                   stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                   creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
                                   start_new_session=os.name != "nt")
    for _ in range(30):
        if process.poll() is not None:
            raise ValueError("collector_start_failed: inspect collector.log")
        if health(manifest):
            return {"pid": process.pid, "port": manifest["port"]}
        time.sleep(0.1)
    raise ValueError("collector_start_timeout: inspect collector.log")


def decode_snapshot(value):
    """Convenience view only; raw JSONL remains the source, including all $cut markers."""
    if isinstance(value, dict):
        if "$array" in value and "$cut" not in value and value.get("length", 129) <= 128:
            items = value["$array"]
            if len(items) == value["length"] and all(item.get("key") == str(i) for i, item in enumerate(items)):
                return [decode_snapshot(item["value"]) for item in items]
        return {key: decode_snapshot(item) for key, item in value.items()}
    if isinstance(value, list):
        return [decode_snapshot(item) for item in value]
    return value


def contains_cut(value):
    if isinstance(value, dict):
        return "$cut" in value or any(contains_cut(v) for v in value.values())
    if isinstance(value, list):
        return any(contains_cut(v) for v in value)
    return False


def report(directory):
    commands, events, queries = Counter(), Counter(), Counter()
    result = {"schema": SCHEMA, "records": 0, "recordsWithTruncation": 0,
              "templateResponses": [], "citySelections": [], "modes": []}
    trace_path = directory / "trace.jsonl"
    if trace_path.exists():
        with trace_path.open(encoding="utf-8") as stream:
            for line in stream:
                entry = json.loads(line)
                raw = entry["record"]
                row = decode_snapshot(raw)
                result["records"] += 1
                result["recordsWithTruncation"] += int(contains_cut(raw))
                channel, phase = row["channel"], row["phase"]
                name, payload = row.get("name"), row.get("payload")
                if channel == "command" and phase == "call":
                    commands[str(name)] += 1
                elif channel == "query" and phase == "call" and isinstance(name, list):
                    queries.update(str(q) for q in name)
                elif channel == "query" and phase == "result" and isinstance(name, list) and "RegionTemplates" in name:
                    position = name.index("RegionTemplates")
                    if isinstance(payload, list) and payload and isinstance(payload[0], list) and position < len(payload[0]):
                        response = payload[0][position]
                        templates = response.get("regionTemplates") if isinstance(response, dict) else response
                        finding = {"page": entry["page"], "sequence": row["sequence"],
                                   "truncated": contains_cut(raw), "templates": []}
                        if isinstance(templates, list):
                            for template in templates:
                                if not isinstance(template, dict):
                                    continue
                                map_data = template.get("map") or {}
                                cities = map_data.get("cities") if isinstance(map_data, dict) else None
                                boxes = template.get("boxes")
                                finding["templates"].append({"name": template.get("name"), "key": template.get("key"),
                                    "cityCount": len(cities) if isinstance(cities, list) else None,
                                    "boxCount": len(boxes) if isinstance(boxes, list) else None,
                                    "cityUIDs": [c.get("uid") for c in cities if isinstance(c, dict)] if isinstance(cities, list) else None})
                        result["templateResponses"] = (result["templateResponses"] + [finding])[-10:]
                elif channel == "event" and phase == "receive":
                    events[str(name)] += 1
                    if name == 246368015:
                        result["citySelections"] = (result["citySelections"] + [{"page": entry["page"],
                            "sequence": row["sequence"], "payload": payload}])[-50:]
                elif channel == "observer":
                    result["modes"] = (result["modes"] + [{"page": entry["page"], "phase": phase, "payload": payload}])[-10:]
    result.update(commands=commands.most_common(30), queries=queries.most_common(30), events=events.most_common(30))
    save(directory / "analysis.json", result)
    return result


class Collector:
    def __init__(self, directory, manifest, limit=64 * 1024 * 1024):
        self.directory, self.manifest, self.limit = directory, manifest, limit
        self.path = directory / "trace.jsonl"
        self.lock = threading.Lock()
        self.last = {}
        self.summary = {"schema": SCHEMA, "build": manifest["build"], "records": 0,
                        "pages": {}, "channels": {}, "lastReceived": None, "bytes": 0}
        if self.path.exists():
            if self.path.stat().st_size > limit:
                raise ValueError("trace_size_limit")
            with self.path.open(encoding="utf-8") as stream:
                for line in stream:
                    entry = json.loads(line)
                    if entry["build"] != manifest["build"]:
                        raise ValueError("trace_build_mismatch")
                    self.account(entry)
            self.summary["bytes"] = self.path.stat().st_size

    def account(self, entry):
        page, record = entry["page"], entry["record"]
        self.last[page] = record["sequence"]
        self.summary["records"] += 1
        self.summary["lastReceived"] = entry["receivedAt"]
        self.summary["pages"][page] = {"lastSequence": record["sequence"], "dropped": entry["dropped"],
                                       "observerErrors": entry["observerErrors"]}
        channel = record["channel"]
        self.summary["channels"][channel] = self.summary["channels"].get(channel, 0) + 1

    def accept(self, body):
        if not isinstance(body, dict) or body.get("schema") != SCHEMA or body.get("build") != self.manifest["build"]:
            raise ValueError("invalid_build_or_schema")
        page, records = body.get("page"), body.get("records")
        if not isinstance(page, str) or not 1 <= len(page) <= 80 or not isinstance(records, list) or not 1 <= len(records) <= 32:
            raise ValueError("invalid_page_or_records")
        for key in ("dropped", "observerErrors"):
            if type(body.get(key)) is not int or body[key] < 0:
                raise ValueError("invalid_counters")
        previous = 0
        for record in records:
            if not isinstance(record, dict) or type(record.get("sequence")) is not int or record["sequence"] <= previous:
                raise ValueError("invalid_sequence")
            if record.get("channel") not in ("observer", "command", "query", "event"):
                raise ValueError("invalid_channel")
            if record.get("phase") not in ("ready", "heartbeat", "call", "receive", "result", "error", "throw"):
                raise ValueError("invalid_phase")
            if type(record.get("time")) not in (int, float):
                raise ValueError("invalid_time")
            previous = record["sequence"]
        with self.lock:
            if page not in self.last and len(self.last) >= 256:
                raise ValueError("page_limit")
            received = time.time()
            entries = [{"schema": SCHEMA, "build": body["build"], "page": page, "receivedAt": received,
                        "dropped": body["dropped"], "observerErrors": body["observerErrors"], "record": r}
                       for r in records if r["sequence"] > self.last.get(page, 0)]
            data = "".join(json.dumps(e, ensure_ascii=False, separators=(",", ":")) + "\n" for e in entries).encode("utf-8")
            if self.summary["bytes"] + len(data) > self.limit:
                raise OverflowError("trace_size_limit")
            if data:
                with self.path.open("ab") as stream:
                    stream.write(data)
                    stream.flush()
                    os.fsync(stream.fileno())
                for entry in entries:
                    self.account(entry)
                self.summary["bytes"] += len(data)
                save(self.directory / "status.json", self.summary)


def server_for(collector, port):
    class Handler(BaseHTTPRequestHandler):
        def setup(self):
            super().setup()
            self.connection.settimeout(3)

        def log_message(self, *_):
            pass  # The URL contains a local capability token; never print it.

        def reply(self, status, body=None):
            data = json.dumps(body).encode("utf-8") if body is not None else b""
            self.send_response(status)
            self.send_header("Access-Control-Allow-Origin", "*")
            self.send_header("Access-Control-Allow-Methods", "POST, OPTIONS")
            self.send_header("Access-Control-Allow-Headers", "Content-Type")
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            if data:
                self.wfile.write(data)

        def authorized(self):
            return hmac.compare_digest(self.path, "/capture/" + collector.manifest["token"])

        def do_GET(self):
            if self.path == "/health":
                self.reply(200, {"schema": SCHEMA, "build": collector.manifest["build"],
                                 "records": collector.summary["records"]})
            else:
                self.reply(404)

        def do_OPTIONS(self):
            self.reply(204 if self.authorized() else 404)

        def do_POST(self):
            if hmac.compare_digest(self.path, "/stop/" + collector.manifest["token"]):
                self.reply(204)
                threading.Thread(target=self.server.shutdown, daemon=True).start()
                return
            if not self.authorized():
                self.reply(404)
                return
            try:
                length = int(self.headers.get("Content-Length", "0"))
                if not 0 < length <= 1048576 or self.headers.get("Transfer-Encoding"):
                    self.reply(413)
                    return
                raw = self.rfile.read(length)
                if len(raw) != length:
                    raise ValueError("incomplete_body")
                collector.accept(json.loads(raw))
            except OverflowError:
                self.reply(507)
                return
            except (ValueError, TypeError, RecursionError):
                self.reply(400)
                return
            except OSError:
                self.reply(503)
                return
            self.reply(204)

    # Serial server bounds connection/thread count. Only small local POSTs are accepted.
    return HTTPServer(("127.0.0.1", port), Handler)


def main():
    lang = "en-US" if "en-US" in sys.argv else "zh-CN"
    text = TEXT[lang]
    parser = argparse.ArgumentParser(description=text["description"])
    parser.add_argument("--lang", choices=TEXT, default=lang, help=text["lang"])
    commands = parser.add_subparsers(dest="command", required=True)
    for command in ("prepare", "install", "start", "serve", "status", "report", "probe", "stop", "uninstall"):
        p = commands.add_parser(command, help=text[command], description=text[command])
        p.add_argument("directory", type=Path, help=text["directory"])
        if command == "prepare":
            p.add_argument("--exe", type=Path, required=True, help=text["exe"])
            p.add_argument("--port", type=int, default=18765, help=text["port"])
            p.add_argument("--stage", choices=("baseline", "marker", "observe"), default="baseline", help=text["stage"])
        if command == "probe":
            p.add_argument("--pid", type=int, required=True, help=text["pid"])
    args = parser.parse_args()
    directory = args.directory.resolve()
    try:
        if args.command == "prepare":
            result, message = prepare(directory, args.exe, args.port, args.stage), "ready"
        elif args.command == "install":
            result, message = install(directory), "installed"
        elif args.command == "uninstall":
            result, message = uninstall(directory), "removed"
        elif args.command == "start":
            result, message = start(directory, lang), "listening"
        elif args.command == "report":
            result, message = report(directory), "report"
        elif args.command == "probe":
            from probe import inspect
            result = inspect(args.pid, directory / "scrui.baseline.js", read_manifest(directory)["build"])
            save(directory / "memory-probe.json", result)
            message = "probe"
        elif args.command == "stop":
            manifest = read_manifest(directory)
            request = urllib.request.Request(f'http://127.0.0.1:{manifest["port"]}/stop/{manifest["token"]}', data=b"", method="POST")
            with urllib.request.urlopen(request, timeout=3) as response:
                if response.status != 204:
                    raise ValueError("stop_failed")
            result, message = {}, "stopped"
        elif args.command == "status":
            status = directory / "status.json"
            result, message = (json.loads(status.read_text(encoding="utf-8")) if status.exists() else {"records": 0}), "status"
            result["collectorRunning"] = health(read_manifest(directory)) is not None
        else:
            manifest = read_manifest(directory)
            collector = Collector(directory, manifest)
            server = server_for(collector, manifest["port"])
            print(json.dumps({"message": text["listening"], "port": manifest["port"], "build": manifest["build"]}, ensure_ascii=False), flush=True)
            try:
                server.serve_forever()
            finally:
                server.server_close()
            return
        print(json.dumps({"message": text[message], **result}, ensure_ascii=False))
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(json.dumps({"message": text["error"], "detail": str(error)}, ensure_ascii=False), file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8")
    main()
