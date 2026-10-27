"""Pure-Python validation helpers for the Blender asset pipeline.

This module deliberately has no Blender dependency so an agent/CI job can validate
an export before opening Blender.
"""
from __future__ import annotations

import json
from pathlib import Path

SLOTS = tuple(f"slot{i}" for i in range(6))
REQUIRED_TEXTURES = tuple(f"slot{i}.png" for i in range(1, 6))


def load_json(path: str | Path) -> dict:
    return json.loads(Path(path).read_text(encoding="utf-8"))


def validate_asset(asset_dir: str | Path) -> dict:
    root = Path(asset_dir)
    errors: list[str] = []
    warnings: list[str] = []
    mesh_path = root / "mesh.json"
    params_path = root / "params.json"
    if not mesh_path.is_file():
        errors.append("missing mesh.json")
    if not params_path.is_file():
        errors.append("missing params.json")
    for name in REQUIRED_TEXTURES:
        if not (root / name).is_file():
            errors.append(f"missing {name}")
    if errors:
        return {"ok": False, "errors": errors, "warnings": warnings}

    mesh = load_json(mesh_path)
    params = load_json(params_path)
    vertices = mesh.get("vertices", [])
    triangles = mesh.get("triangles", [])
    width = int(params.get("width", 0))
    values = params.get("values", [])
    if not vertices or not triangles:
        errors.append("mesh.json has no vertices or triangles")
    if any(len(v.get("position", [])) != 3 or len(v.get("uv", [])) != 4 for v in vertices):
        errors.append("each vertex needs position[3] and uv[4]")
    if any(len(t) != 3 or any(i < 0 or i >= len(vertices) for i in t) for t in triangles):
        errors.append("triangle index is out of range")
    expected = width * 4
    if width <= 0 or len(values) not in (width, expected):
        errors.append(f"params.json needs {expected} float values or {width} RGBA rows ({width} vs {len(values)})")
    if width > 256:
        errors.append("material column count exceeds the byte based TEXCOORD_1 contract")
    columns = [int(v.get("column", -1)) for v in vertices]
    if columns and (min(columns) < 0 or max(columns) >= max(width, 1)):
        errors.append("vertex material column is outside params.json width")
    if values and isinstance(values[0], list) and any(len(row) != 4 for row in values):
        errors.append("each params row must contain RGBA values")
    if not (root / "slot0_f32.png").exists() and not (root / "slot0.exr").exists():
        warnings.append("slot0 float image is absent; Blender will build it from params.json")
    return {
        "ok": not errors,
        "errors": errors,
        "warnings": warnings,
        "vertices": len(vertices),
        "triangles": len(triangles),
        "param_columns": width,
        "used_columns": sorted(set(columns)),
    }


def write_manifest(asset_dir: str | Path, output: str | Path, result: dict) -> None:
    payload = {"asset_dir": str(Path(asset_dir).resolve()), "validation": result}
    Path(output).write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
