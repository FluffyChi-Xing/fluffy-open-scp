"""Offline coverage checks for historical map-mod questions."""
import argparse, json, hashlib
from collections import Counter
from pathlib import Path

EXPECTED = {
    "region_template": ["RegionTemplates", "LatestBox", "PlayingRegions", "UserData"],
    "city_slot": ["regiondata", "RegionPlayers", "boxes", "cities"],
    "localization": ["locale", "Locale", "GetCityStubDescription"],
    "resource": ["resourceAmount", "binAmount", "cachedUnits", "bins", "regionUnits"],
    "camera": ["205419630", "205419631", "screenPositionCenter", "origin"],
}

def run(source):
    rows = json.loads((source / "timeline.json").read_text(encoding="utf-8"))
    queries, commands, events = [], [], []
    for row in rows:
        args = row.get("arguments", [])
        text = args[0].get("text", "") if args else ""
        if row.get("channel") == "invoke":
            (queries if row.get("name") == "RequestGameData" else commands).append(text)
        elif row.get("channel") == "event":
            events.append(row)
    query_names = Counter(x.split("/", 1)[0] for x in queries)
    command_names = Counter(x.split("/", 1)[0] for x in commands)
    event_ids = Counter(str(x.get("eventID")) for x in events)
    observed = {"queries": dict(query_names), "commands": dict(command_names), "eventIDs": dict(event_ids)}
    checks = {}
    for category, needles in EXPECTED.items():
        hits = []
        for needle in needles:
            found = any(needle.lower() in x.lower() for x in queries + commands)
            if needle in event_ids: found = True
            if found: hits.append(needle)
        checks[category] = {"status": "observed" if hits else "not_observed",
                           "signals": hits,
                           "interpretation": {
                               "region_template": "现有样本未进入创建区域页；不能验证模板城市数与boxes。",
                               "city_slot": "regiondata/RegionPlayers只能证明相关命令出现，需区域页或区域数据返回。",
                               "localization": "通信日志不直接等于locale消费；需页面加载和显示值对拍。",
                               "resource": "已有城市资源查询可验证消费读取，不能证明地图初始化或存档持久化。",
                               "camera": "可验证移动通知和部分坐标查询，不能证明边界夹取已解除。",
                           }[category]}
    report = {"source": str(source.resolve()), "sourceSha256": hashlib.sha256((source / "timeline.json").read_bytes()).hexdigest(),
              "recordCount": len(rows), "observed": observed, "checks": checks,
              "nextScenarios": [
                  {"name": "create-region", "actions": ["创建区域页", "切换两张模板地图", "选中城市后返回"], "captures": ["RegionTemplates", "regiondata", "boxes", "cities", "locale"]},
                  {"name": "resource-reload", "actions": ["进入一个新城市", "切换资源图层", "保存并重载"], "captures": ["resourceAmount", "binAmount", "cachedUnits", "globalFlags", "事件时间线"]},
                  {"name": "camera-edge", "actions": ["连续移动到四个城市边缘", "缩放和旋转", "切换城市后重复"], "captures": ["camera start/stop", "screenPositionCenter", "origin", "坐标对象字段"]},
              ], "scope": "offline analysis; no process attach"}
    return report

def main():
    import sys
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")
    p = argparse.ArgumentParser(description="历史地图问题的离线日志覆盖检查 / Historical map log coverage")
    p.add_argument("source", type=Path)
    p.add_argument("--output", type=Path, required=True)
    a = p.parse_args(); report = run(a.source)
    a.output.mkdir(parents=True, exist_ok=False)
    (a.output / "coverage.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"output": str(a.output / "coverage.json"), "records": report["recordCount"], "checks": {k: v["status"] for k,v in report["checks"].items()}}, ensure_ascii=False))
if __name__ == "__main__": main()
