ALTER TABLE drafts ADD COLUMN pending_project_name TEXT;
ALTER TABLE drafts ADD COLUMN dirty INTEGER NOT NULL DEFAULT 1 CHECK(dirty IN(0,1));
CREATE TABLE draft_pending_tags (
 draft_id TEXT NOT NULL REFERENCES drafts(id) ON DELETE CASCADE,
 position INTEGER NOT NULL CHECK(position>=0), name TEXT NOT NULL,
 PRIMARY KEY(draft_id,position)
) STRICT;
