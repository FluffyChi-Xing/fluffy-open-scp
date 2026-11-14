"""SC5 Asset Packager MVP.

This layer intentionally never writes to the game directory. It owns project
metadata, input validation, reproducible reports, and the future compiler hook.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from menu_locations import MENU_LOCATIONS, destination
import behavior_profiles

SCHEMA = "sc5.asset-project/1"
INPUT_NAMES = {"glb", "lotm", "package", "json", "png", "jpg", "jpeg"}
REQUIRED_HINTS = {"glb": "GLB geometry", "lotm": "native LOTM", "package": "lot/package"}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def now() -> str:
    return datetime.now(timezone.utc).isoformat()


def load_project(project: Path) -> dict[str, Any]:
    path = project / "project.json"
    if not path.is_file():
        raise ValueError(f"缺少项目配置：{path}")
    data = json.loads(path.read_text(encoding="utf-8"))
    if data.get("schema") != SCHEMA:
        raise ValueError(f"不支持的项目 schema：{data.get('schema')!r}")
    for key in ("name", "category", "version", "settings"):
        if key not in data:
            raise ValueError(f"项目缺少字段：{key}")
    return data


def open_asset_folder(folder: Path) -> tuple[Path, bool]:
    """Open an existing project or attach a manifest to a Blender export folder.

    Original assets stay in place. Only top-level supported source files are
    enrolled, so build/output packages never become inputs on later opens.
    """
    folder = folder.expanduser().resolve()
    if not folder.is_dir():
        raise ValueError("请选择一个存在的项目或资产文件夹")
    manifest = folder / "project.json"
    if manifest.exists():
        load_project(folder)
        return folder, False
    sources = sorted(p for p in folder.iterdir()
                     if p.is_file() and p.suffix.lower().lstrip(".") in INPUT_NAMES)
    if not any(p.suffix.lower() in {".glb", ".lotm"} for p in sources):
        raise ValueError("此文件夹没有打包项目，也没有 Blender 导出的 GLB 或 LOTM。请选择包含建筑导出文件的文件夹，或点击“新建项目”。")
    config = {
        "schema": SCHEMA, "name": folder.name, "nameEn": "SC5_Asset",
        "category": "education", "version": "0.1.0", "description": "",
        "sources": [str(p) for p in sources],
        "settings": {"cost": 50000, "maintenancePerHour": 100,
                     "geometryStrategy": "stable_single_mesh",
                     "materialStrategy": "compact_2048",
                     "behaviorProfile": behavior_profiles.LEGACY,
                     "provideSimulationService": False},
        "backend": None,
    }
    try:
        # Exclusive creation avoids replacing a project created concurrently.
        with manifest.open("x", encoding="utf-8") as stream:
            stream.write(json.dumps(config, ensure_ascii=False, indent=2) + "\n")
    except FileExistsError:
        load_project(folder)
        return folder, False
    return folder, True


def inspect_inputs(project: Path) -> dict[str, Any]:
    input_dir = project / "input"
    files = []
    warnings: list[str] = []
    errors: list[str] = []
    if input_dir.is_dir():
        for path in sorted(input_dir.rglob("*")):
            if path.is_file() and path.suffix.lower().lstrip(".") in INPUT_NAMES:
                files.append({
                    "path": str(path.relative_to(project)),
                    "bytes": path.stat().st_size,
                    "sha256": sha256(path),
                    "extension": path.suffix.lower().lstrip("."),
                })
    for external in load_project(project).get("sources", []):
        path = Path(external).expanduser().resolve()
        if path.is_file():
            try:
                digest = sha256(path)
                size = path.stat().st_size
            except (OSError, PermissionError) as exc:
                warnings.append(f"无法读取外部源文件：{path} ({exc})")
                continue
            if not any((project / item["path"]).resolve() == path for item in files):
                files.append({"path": str(path), "bytes": size, "sha256": digest, "extension": path.suffix.lower().lstrip("."), "external": True})

    for item in list(files):
        if item['extension']=='lotm':
            model=(project/ item['path']).resolve()
            for suffix in ('.props.json','.lights.json'):
                side=model.with_suffix(suffix)
                if side.is_file() and not any((project/x['path']).resolve()==side for x in files):
                    files.append({'path':str(side),'bytes':side.stat().st_size,'sha256':sha256(side),'extension':'json','external':True})
    by_ext: dict[str, list[dict[str, Any]]] = {}
    for item in files:
        by_ext.setdefault(item["extension"], []).append(item)
    if not by_ext.get("glb"):
        warnings.append("未找到 GLB 几何文件")
    if not by_ext.get("lotm"):
        warnings.append("未找到 LOTM 原生材质文件")
    if not by_ext.get("package"):
        warnings.append("未找到 lot/package 地块来源文件")
    for item in files:
        if item["bytes"] == 0:
            errors.append(f"空文件：{item['path']}")

    native = []
    for item in by_ext.get("json", []):
        path = project / item["path"]
        try:
            value = json.loads(path.read_text(encoding="utf-8-sig"))
        except Exception as exc:
            warnings.append(f"JSON 无法解析：{item['path']} ({exc})")
            continue
        if "nativeControlProfile" in value or "nativePayload" in json.dumps(value):
            native.append({"path": item["path"], "keys": sorted(value)[:30]})

    return {
        "checkedAt": now(),
        "files": files,
        "countsByExtension": {k: len(v) for k, v in sorted(by_ext.items())},
        "nativeMetadata": native,
        "warnings": warnings,
        "errors": errors,
        "readOnly": True,
    }


def init_project(project: Path, args: argparse.Namespace) -> None:
    project.mkdir(parents=True, exist_ok=True)
    for name in ("input", "reports", "build", "output"):
        (project / name).mkdir(exist_ok=True)
    config = {
        "schema": SCHEMA,
        "name": args.name,
        "nameEn": args.name,
        "category": args.category,
        "version": "0.1.0",
        "description": "",
        "settings": {
            "cost": 50000,
            "maintenancePerHour": 100,
            "geometryStrategy": "stable_single_mesh",
            "materialStrategy": "compact_2048",
            "behaviorProfile": behavior_profiles.LEGACY,
            "provideSimulationService": False,
        },
        "backend": None,
    }
    (project / "project.json").write_text(json.dumps(config, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"created": str(project.resolve()), "project": config}, ensure_ascii=False, indent=2))


def inspect_project(project: Path) -> int:
    config = load_project(project)
    report = {"project": config, "inputs": inspect_inputs(project)}
    try:report['behavior']=behavior_profiles.preflight(config)
    except (ValueError,OSError) as exc:report['inputs']['errors'].append('功能检查：'+str(exc))
    target = project / "reports" / "asset-inspection.json"
    target.parent.mkdir(exist_ok=True)
    target.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 1 if report["inputs"]["errors"] else 0


def build_project(project: Path, dry_run: bool) -> int:
    config = load_project(project)
    location=destination(config["category"])
    inspection = inspect_inputs(project)
    try:behavior=behavior_profiles.preflight(config)
    except (ValueError,OSError) as exc:
        behavior={'valid':False,'error':str(exc)}
        inspection['errors'].append('功能检查：'+str(exc))
    plan = {
        "schema": "sc5.asset-build-plan/1",
        "createdAt": now(),
        "project": str(project.resolve()),
        "name": config["name"],
        "category": config["category"],
        "menuLocation": {"label":location["label"],"path":location["path"],"runtimeVerified":location["runtimeVerified"],"scope":"menu-location-only"},
        "settings": config["settings"],
        "behavior": behavior,
        "inputReport": inspection,
        "gameWrites": False,
        "steps": [
            "validate_inputs",
            "compile_geometry_rw4",
            "compile_six_slot_materials",
            "compile_lot_raster",
            "compile_property_menu_unit_asset",
            "write_dbpf_package",
            "run_offline_validation",
        ],
    }
    target = project / "reports" / "build-plan.json"
    target.parent.mkdir(exist_ok=True)
    target.write_text(json.dumps(plan, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    if dry_run:
        print(json.dumps(plan, ensure_ascii=False, indent=2))
        return 1 if inspection["errors"] else 0
    if inspection['errors']:
        raise ValueError('打包前检查未通过：'+'; '.join(inspection['errors']))
    backend = config.get("backend") or {}
    command = backend.get("command")
    if not command:
        if getattr(sys, 'frozen', False):
            command = [sys.executable, '--compile-project', '{project}']
        else:
            command = [sys.executable, str(Path(__file__).with_name('backend_compile.py')), '{project}']
    if not isinstance(command, list) or not all(isinstance(part, str) for part in command):
        raise ValueError("backend.command 必须是字符串数组")
    output = project / "output"
    output.mkdir(exist_ok=True)
    previous_packages = {p.resolve(): (p.stat().st_size, p.stat().st_mtime_ns) for p in output.glob('*.package')}
    substitutions = {
        "project": str(project.resolve()),
        "output": str(output.resolve()),
        "config": str((project / "project.json").resolve()),
    }
    expanded = [part.format(**substitutions) for part in command]
    log_path = project / "reports" / "backend.log"
    with log_path.open("w", encoding="utf-8") as log:
        log.write(f"startedAt={now()}\ncommand={json.dumps(expanded, ensure_ascii=False)}\n")
        completed = subprocess.run(expanded, cwd=project, stdout=log, stderr=subprocess.STDOUT, check=False,
                                   creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
        log.write(f"finishedAt={now()}\nreturnCode={completed.returncode}\n")
    packages = [{'path':str(p.resolve()),'bytes':p.stat().st_size} for p in output.glob('*.package')
                if previous_packages.get(p.resolve()) != (p.stat().st_size,p.stat().st_mtime_ns)]
    code=completed.returncode
    error=None
    if code==0 and not packages:
        code=1
        error='编译器没有生成本次 Package；旧输出不计为本次成功结果'
    result = {
        "menuLocation": plan["menuLocation"],
        "behavior": plan["behavior"],
        "schema": "sc5.asset-build-result/1",
        "createdAt": now(),
        "returnCode": code,
        "command": expanded,
        "log": str(log_path.relative_to(project)),
        "output": str(output.relative_to(project)),
        "outputDirectory": str(output.resolve()),
        "packages": packages if code==0 else [],
        "error": error,
        "gameWrites": False,
    }
    (project / "reports" / "build-result.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return code


def main() -> int:
    parser = argparse.ArgumentParser(description="SC5 Asset Packager MVP")
    sub = parser.add_subparsers(dest="command", required=True)
    p_init = sub.add_parser("init", help="创建项目目录")
    p_init.add_argument("project", type=Path)
    p_init.add_argument("--name", required=True)
    p_init.add_argument("--category", choices=list(MENU_LOCATIONS), default="education")
    p_inspect = sub.add_parser("inspect", help="只读检查输入资产")
    p_inspect.add_argument("project", type=Path)
    p_build = sub.add_parser("build", help="生成编译计划或调用后端")
    p_build.add_argument("project", type=Path)
    p_build.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    if args.command == "init":
        init_project(args.project, args)
        return 0
    if args.command == "inspect":
        return inspect_project(args.project)
    return build_project(args.project, args.dry_run)


if __name__ == "__main__":
    raise SystemExit(main())
