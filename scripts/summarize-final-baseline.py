"""Summarize existing instrumented response events for the explicit final test session."""
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1]
lab = root / "evidence/release-preparation"
session = json.loads((lab / "performance-session.json").read_text(encoding="utf-8-sig"))
data = Path(session["data"])
events = []
for log in data.rglob("logs/poc.jsonl"):
    for line in log.read_text(encoding="utf-8").splitlines():
        event = json.loads(line)
        if event["pid"] == session["pid"] and event["timestampMs"] >= session["startedAtMs"]:
            events.append(event)
groups = {
    "quickWakeHotkey": [e["data"]["elapsedMs"] for e in events if e["kind"] == "quick_interactive" and e["data"].get("source") == "hotkey"],
    "libraryInteractive": [e["data"]["elapsedMs"] for e in events if e["kind"] == "library_interactive"],
    "quickSearch": [e["data"]["elapsedMs"] for e in events if e["kind"] == "search_interactive" and e["data"].get("window") == "quick_panel"],
}
summary = {}
for name, values in groups.items():
    ordered = sorted(values)
    summary[name] = {"samples": len(values), "p50Ms": ordered[(len(values)-1)//2] if values else None, "p95Ms": ordered[max(0, (len(values)*95+99)//100-1)] if values else None, "maxMs": max(values) if values else None, "valuesMs": values}
result = {"responses": summary, "notInteractiveEvents": sum(e["kind"] == "ready_not_interactive" for e in events), "supersededSearches": sum(e["kind"] == "search_superseded" for e in events), "thresholds": "NONE", "limits": "User-operated states. Wake uses existing Rust start -> React focused ready instrumentation; search measures input -> React commit + RAF. Small samples are descriptive, not a performance pass. Library timing may include cold creation; no cold/hot inference."}
(lab / "response-baseline.json").write_text(json.dumps(result, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
print(json.dumps(result, ensure_ascii=False, indent=2))
