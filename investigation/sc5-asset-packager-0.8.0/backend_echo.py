"""Development backend used to verify the packager contract."""
from __future__ import annotations
import argparse
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--project", required=True, type=Path)
parser.add_argument("--output", required=True, type=Path)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
files = sorted(str(p.relative_to(args.project)) for p in (args.project / "input").rglob("*") if p.is_file())
(args.output / "backend-echo.json").write_text(json.dumps({"project": str(args.project), "inputFiles": files, "gameWrites": False}, ensure_ascii=False, indent=2), encoding="utf-8")
print("BACKEND_ECHO_OK", len(files))
