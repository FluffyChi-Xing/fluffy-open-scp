"""Native backend adapter for the current research compiler.

The adapter is deliberately configuration-driven. Existing research pipelines
can be used without changing the packager CLI; a future generic compiler can
replace the pipeline script while keeping this contract.
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import hashlib
from datetime import datetime, timezone
from pathlib import Path


def now() -> str:
    return datetime.now(timezone.utc).isoformat()


def input_manifest(project: Path) -> list[dict[str, object]]:
    """Capture the exact input set passed to a compiler for reproducibility."""
    root = project / "input"
    items = []
    if not root.is_dir():
        return items
    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        items.append({"path": str(path.relative_to(project)), "bytes": path.stat().st_size, "sha256": digest})
    return items


def main() -> int:
    parser = argparse.ArgumentParser(description="SC5 native research-pipeline adapter")
    parser.add_argument("--project", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--pipeline", required=True, type=Path)
    parser.add_argument("--package-glob", default="**/*.package")
    args = parser.parse_args()
    project = args.project.resolve()
    output = args.output.resolve()
    pipeline = args.pipeline.resolve()
    if not project.is_dir() or not pipeline.is_file():
        raise SystemExit("项目目录或编译流水线不存在")
    output.mkdir(parents=True, exist_ok=True)
    started = {p: p.stat().st_mtime_ns for p in pipeline.parent.glob(args.package_glob) if p.is_file()}
    completed = subprocess.run([sys.executable, str(pipeline)], cwd=pipeline.parent, check=False)
    if completed.returncode:
        return completed.returncode
    candidates = sorted(
        (p for p in pipeline.parent.glob(args.package_glob)
         if p.is_file() and (p not in started or p.stat().st_mtime_ns > started[p])),
        key=lambda p: p.stat().st_mtime_ns,
        reverse=True,
    )
    if not candidates:
        raise SystemExit("编译后端完成，但没有找到 Package 输出")
    package = candidates[0]
    project_config = json.loads((project / "project.json").read_text(encoding="utf-8-sig"))
    safe_name = "".join(ch if ch.isalnum() or ch in "-_" else "_" for ch in project_config.get("nameEn", project_config.get("name", "SC5_Asset")))
    target = output / f"{safe_name}_{project_config.get('version', '0.1.0')}.package"
    shutil.copy2(package, target)
    result = {
        "schema": "sc5.native-backend-result/1",
        "createdAt": now(),
        "pipeline": str(pipeline),
        "package": str(target),
        "bytes": target.stat().st_size,
        "gameWrites": False,
        "sourceProject": str(project),
        "inputs": input_manifest(project),
        "compilerContract": "sc5.native-backend-result/1",
    }
    (project / "reports" / "native-backend-result.json").write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
