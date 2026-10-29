"""Read-only logical + original-byte fingerprint for an explicitly marked test library."""
import hashlib
import json
from pathlib import Path
import sqlite3
import sys

root = Path(sys.argv[1]).resolve()
if not (root / "stage-f-test-owned").is_file():
    raise SystemExit("Refusing unmarked library; test libraries only")
pointer_path = root / "active.json"
location = json.loads(pointer_path.read_text(encoding="utf-8"))["location"] if pointer_path.exists() else {"kind": "flat"}
active = root if location["kind"] == "flat" else root / "storage" / location["id"]
connection = sqlite3.connect((active / "library.db").as_uri() + "?mode=ro", uri=True)
tables = [row[0] for row in connection.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")]
rows = {}
counts = {}
for table in tables:
    values = list(connection.execute('SELECT * FROM "' + table.replace('"', '""') + '"'))
    counts[table] = len(values)
    rows[table] = sorted([json.dumps(row, ensure_ascii=False, default=lambda value: value.hex()) for row in values])
originals = {str(p.relative_to(active)).replace("\\", "/"): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((active / "assets").rglob("*")) if p.is_file()}
print(json.dumps({"schema": connection.execute("PRAGMA user_version").fetchone()[0], "integrity": connection.execute("PRAGMA integrity_check").fetchone()[0], "foreignKeys": list(connection.execute("PRAGMA foreign_key_check")), "counts": counts, "contentSha256": hashlib.sha256(json.dumps(rows, sort_keys=True, ensure_ascii=False).encode()).hexdigest(), "originals": originals}, ensure_ascii=False, indent=2))
