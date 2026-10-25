"""Summarize per-agent traffic snapshots without treating them as engine logs."""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import json
from pathlib import Path
from typing import Any


def load_rows(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def analyze(rows: list[dict[str, Any]]) -> dict[str, Any]:
    current = Counter()
    candidates = Counter()
    transitions = Counter()
    sequences: dict[str, list[dict[str, Any]]] = defaultdict(list)
    observations = 0
    previous: dict[str, tuple[int, int | None]] = {}
    previous_slot_agent: dict[str, str | None] = {}
    key_reuses = 0

    for row in rows:
        tick = int(row.get("simTick", -1))
        for agent in row.get("agents", []):
            transaction = (agent.get("detail") or {}).get("transaction") or {}
            sink = transaction.get("sinkEntryIndex")
            sink_candidates = tuple(transaction.get("sinkEntryIndices") or [])
            slot = f"{agent.get('handler')}:{agent.get('key')}"
            key = f"{slot}:{agent.get('agent')}"
            observations += 1
            current[str(sink)] += 1
            candidates["[" + ", ".join(str(value) for value in sink_candidates) + "]"] += 1
            if slot in previous_slot_agent and previous_slot_agent[slot] != agent.get("agent"):
                key_reuses += 1
            previous_slot_agent[slot] = agent.get("agent")
            if key in previous and previous[key][1] != sink:
                transitions[f"{previous[key][1]}->{sink}"] += 1
            previous[key] = (tick, sink)
            if len(sequences[key]) < 64:
                if not sequences[key] or sequences[key][-1]["sinkEntryIndex"] != sink:
                    sequences[key].append({
                        "simTick": tick,
                        "sinkEntryIndex": sink,
                        "sinkEntryIndices": list(sink_candidates),
                    })

    changed = {key: value for key, value in sequences.items() if len(value) > 1}
    return {
        "schema": "openscp.traffic-agent-analysis.v1",
        "rows": len(rows),
        "observations": observations,
        "currentSinkCounts": dict(current),
        "candidateListCounts": dict(candidates),
        "currentSinkTransitions": dict(transitions),
        "keysWithSinkTransitions": len(changed),
        "transactionKeyReuseObservations": key_reuses,
        "sinkTransitionSequences": changed,
        "interpretation": (
            "Current sink 323 is direct evidence that the transaction selected the "
            "CrimeInProgress sink only when observed; a candidate list containing 323 "
            "is not evidence of selection. Sequences are non-atomic external snapshots."
        ),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="Analyze traffic per-agent snapshots")
    parser.add_argument("samples", type=Path, help="agent-samples.jsonl")
    parser.add_argument("--out", type=Path, help="write JSON summary")
    args = parser.parse_args()
    result = analyze(load_rows(args.samples))
    encoded = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if args.out:
        args.out.write_text(encoded, encoding="utf-8")
    print(encoded, end="")


if __name__ == "__main__":
    main()
