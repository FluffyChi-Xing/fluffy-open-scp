from __future__ import annotations

import hashlib
import json
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUNTIME = ROOT / "tools/sc5_asset_packager/runtime"
sys.path.insert(0, str(RUNTIME / "engine"))
from inspect_park_route import parse
from package_io import index
from rw4_bridge import RW4

BASE = ROOT / "tools/sc5_asset_packager/projects/tall_test/build/run-da10abeb4796"
SPLICE = ROOT / "tools/sc5_asset_packager/projects/tall_test/build/run-c0814b62eacb"
S31 = ROOT / "tmp/s31-project/.build/6cb4f7c487047df6241d3ff73a8cc30b87573900ac5a8f3c8f2d751dc032cc2e/packages/000_OpenSCP_University_s31.package"


def resources(package: Path):
    return {entry.key: entry.read() for entry in index(package)}


def report(run: Path):
    return json.loads((run / "candidate/候选包验证报告.json").read_text(encoding="utf-8"))


def key(text: str):
    return tuple(int(part, 16) for part in text.split(":"))


def tgi(value):
    return ":".join(f"{part:08X}" for part in value)


def sha(data: bytes):
    return hashlib.sha256(data).hexdigest()


def role_fields(run: Path):
    rep = report(run)
    res = resources(run / "candidate" / rep["package"])
    roles = {role: key(rep["identities"][role]) for role in ("menu", "unit", "asset")}
    return rep, res, roles, {role: parse(res[identity], True) for role, identity in roles.items()}


base_report, base_res, base_roles, base_props = role_fields(BASE)
splice_report, splice_res, splice_roles, splice_props = role_fields(SPLICE)
s31_res = resources(S31)
s31_props = {}
for identity, data in s31_res.items():
    if identity[0] != 0xB1B104:
        continue
    fields = parse(data, True)
    if "0975695F" in fields:
        s31_props["menu"] = fields
    elif "08E17ED7" in fields:
        s31_props["unit"] = fields
    elif "00F9EFBB" in fields:
        s31_props["asset"] = fields

print("== package identity ==")
for label, rep in (("base", base_report), ("splice", splice_report)):
    print(label, rep["sha256"], rep["resourceCount"], rep["identities"])

print("\n== model payload comparison ==")
base_lods = {sha(base_res[key(identity)]) for identity in base_report["identities"]["lods"]}
splice_lods = {sha(splice_res[key(identity)]) for identity in splice_report["identities"]["lods"]}
print("base LOD hashes", sorted(base_lods))
print("splice LOD hashes", sorted(splice_lods))
print("identical LOD payloads", base_lods == splice_lods)
for label, rep, res in (("base", base_report, base_res), ("splice", splice_report, splice_res)):
    identity = key(rep["identities"]["lods"][0])
    rw = RW4(res[identity])
    points, triangles = rw.decode()
    print(label, "LOD0", "sections", len(rw.sections), "meshes", len(rw.of_type(0x20005)), "vertices", len(points), "triangles", len(triangles), "bounds", rep and parse(res[key(rep["identities"]["asset"])], True)["00F9EFBA"]["value"])

print("\n== property differences: splice versus base ==")
for role in ("menu", "unit", "asset"):
    a = base_props[role]
    b = splice_props[role]
    print(role, "baseFields", len(a), "spliceFields", len(b), "onlyBase", sorted(set(a)-set(b)), "onlySplice", sorted(set(b)-set(a)))

print("\n== property differences: splice versus s31 ==")
for role in ("menu", "unit", "asset"):
    a = s31_props[role]
    b = splice_props[role]
    print(role, "s31Fields", len(a), "spliceFields", len(b))
    for field in sorted(set(a) | set(b)):
        if field not in a:
            print(" +", field, b[field]["type"], b[field]["value"])
        elif field not in b:
            print(" -", field, a[field]["type"], a[field]["value"])
        elif a[field]["recordHex"] != b[field]["recordHex"]:
            print(" *", field, "s31=", a[field]["value"], "splice=", b[field]["value"])

print("\n== scene resource families ==")
for label, res in (("s31", s31_res), ("base", base_res), ("splice", splice_res)):
    counts = {}
    for identity in res:
        counts[f"{identity[0]:08X}"] = counts.get(f"{identity[0]:08X}", 0) + 1
    print(label, counts)
    for identity in sorted(res):
        if identity[0] == 0xB1B104 and identity not in splice_roles.values() and identity not in base_roles.values():
            print(" extra property", tgi(identity), len(res[identity]))

print("\n== locale ==")
for label, rep, res in (("base", base_report, base_res), ("splice", splice_report, splice_res)):
    locale = res[key(rep["identities"]["locale"])].decode("utf-8-sig")
    print(label, rep["identities"]["locale"], locale)

print("\n== material slots ==")
for label, rep in (("base", base_report), ("splice", splice_report)):
    print(label, json.dumps(rep.get("authoredMaterial"), ensure_ascii=False))
