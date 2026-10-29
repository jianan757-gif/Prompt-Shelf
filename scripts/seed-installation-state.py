"""Add a draft/trash/favorite to the explicitly owned installation test fixture."""
from pathlib import Path
import sqlite3
import sys

root = Path(sys.argv[1]).resolve()
if not (root / "stage-f-test-owned").is_file():
    raise SystemExit("Test library marker required")
with sqlite3.connect(root / "library.db") as db:
    db.execute("PRAGMA foreign_keys=ON")
    ids = [r[0] for r in db.execute("SELECT id FROM cases ORDER BY id LIMIT 2")]
    db.execute("UPDATE cases SET deleted_at=1 WHERE id=?", (ids[0],))
    db.execute("UPDATE cases SET favorite=1,last_used_at=2 WHERE id=?", (ids[1],))
    db.execute("INSERT INTO drafts(id,case_id,base_revision,title,prompt,updated_at,save_seq) SELECT 'f-install-draft',id,edit_revision,'Installation draft','Unsaved draft content',3,1 FROM cases WHERE id=?", (ids[1],))
    db.execute("INSERT INTO draft_assets(id,draft_id,asset_id,role,position,original_name) SELECT 'draft-'||id,'f-install-draft',asset_id,role,position,original_name FROM case_assets WHERE case_id=?", (ids[1],))
    db.execute("INSERT INTO draft_tags SELECT 'f-install-draft',tag_id,position FROM case_tags WHERE case_id=?", (ids[1],))
print("Seeded isolated trash, favorite, recent use and shared-image draft")
