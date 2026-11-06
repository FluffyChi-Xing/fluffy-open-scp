#!/usr/bin/env python3
"""测试包归档/部署 + 台账登记（AGENTS.md 固化规则的工具化，2026-11-06）。

用法：
  归档:  python scripts/archive_test_package.py archive <包文件名> [--golden "备注"]
         （从 SimCityData 移到 test-archive，自动登记 MANIFEST.md）
  部署:  python scripts/archive_test_package.py deploy <包文件路径>
         （复制到 SimCityData，自动登记 MANIFEST.md 部署记录）
  状态:  python scripts/archive_test_package.py status <包文件名或sha前缀>
         （按文件名或 sha 前缀查台账，回答"哪个包在工作/哪个是坏的"）

关键约束（教训固化）：
- 对拍参照必须先验 SHA-256：部署过的包 ≠ staging 构建产物，永远先对哈希。
- known-working（GOLDEN）包是验收参照，移入 test-archive 时必须标 GOLDEN，
  且禁止同 TGI 重写/覆盖。
"""
import hashlib
import os
import sys
import datetime

GAME_DATA = r"D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData"
ARCHIVE = r"D:/ea-games/simcity_offline/test-archive"
MANIFEST = os.path.join(ARCHIVE, "MANIFEST.md")


def sha256(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def append_manifest(lines):
    with open(MANIFEST, "a", encoding="utf-8") as handle:
        handle.write("\n".join(lines) + "\n")


def cmd_archive(name, golden_note=None):
    src = os.path.join(GAME_DATA, name)
    dst = os.path.join(ARCHIVE, name)
    digest = sha256(src)
    os.replace(src, dst)
    today = datetime.date.today().isoformat()
    status = f"**GOLDEN-known-working**（{golden_note}）" if golden_note else "archived"
    append_manifest([
        "",
        f"| {name} | `{digest[:16]}` | {os.path.getsize(dst):,} | {status} | {today} |",
    ])
    print(f"archived {name} sha256={digest[:16]} status={status}")


def cmd_deploy(path):
    name = os.path.basename(path)
    digest = sha256(path)
    with open(path, "rb") as handle:
        payload = handle.read()
    with open(os.path.join(GAME_DATA, name), "wb") as handle:
        handle.write(payload)
    today = datetime.date.today().isoformat()
    append_manifest([
        "",
        f"| {today} | {name} | `{digest[:16]}` | 部署至 SimCityData |",
    ])
    print(f"deployed {name} sha256={digest[:16]}")


def cmd_status(query):
    for line in open(MANIFEST, encoding="utf-8"):
        if query.lower() in line.lower():
            print(line.rstrip())


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    command = sys.argv[1]
    if command == "archive":
        cmd_archive(sys.argv[2], sys.argv[4] if len(sys.argv) > 4 and sys.argv[3] == "--golden" else None)
    elif command == "deploy":
        cmd_deploy(sys.argv[2])
    elif command == "status":
        for name in sys.argv[2:]:
            cmd_status(name)
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
