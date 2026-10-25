CREATE TABLE projects (
 id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL CHECK(length(trim(name))>0),
 name_key TEXT NOT NULL UNIQUE, sort_order INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL
) STRICT;
CREATE TABLE tags (
 id TEXT PRIMARY KEY NOT NULL, name TEXT NOT NULL CHECK(length(trim(name))>0), name_key TEXT NOT NULL UNIQUE,
 is_quick INTEGER NOT NULL DEFAULT 0 CHECK(is_quick IN(0,1)), quick_order INTEGER NOT NULL DEFAULT 0,
 sort_order INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL
) STRICT;
CREATE TABLE cases (
 id TEXT PRIMARY KEY NOT NULL, title TEXT NOT NULL CHECK(length(trim(title))>0),
 prompt TEXT NOT NULL CHECK(length(trim(prompt))>0), source_url TEXT NOT NULL DEFAULT '',
 project_id TEXT REFERENCES projects(id) ON DELETE SET NULL, favorite INTEGER NOT NULL DEFAULT 0 CHECK(favorite IN(0,1)),
 created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL, last_used_at INTEGER,
 use_count INTEGER NOT NULL DEFAULT 0 CHECK(use_count>=0), edit_revision INTEGER NOT NULL DEFAULT 1 CHECK(edit_revision>0),
 deleted_at INTEGER, deleted_project_name TEXT
) STRICT;
CREATE INDEX cases_live_used ON cases(deleted_at,last_used_at);
CREATE INDEX cases_live_created ON cases(deleted_at,created_at);
CREATE INDEX cases_project ON cases(project_id);
CREATE TABLE case_tags (
 case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
 tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE, position INTEGER NOT NULL CHECK(position>=0),
 PRIMARY KEY(case_id,tag_id), UNIQUE(case_id,position)
) STRICT;
CREATE INDEX case_tags_tag ON case_tags(tag_id);
CREATE TABLE assets (
 id TEXT PRIMARY KEY NOT NULL, sha256 TEXT NOT NULL UNIQUE CHECK(length(sha256)=64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
 extension TEXT NOT NULL CHECK(extension IN('png','jpg','webp')), mime TEXT NOT NULL,
 byte_size INTEGER NOT NULL CHECK(byte_size>0), width INTEGER NOT NULL CHECK(width>0), height INTEGER NOT NULL CHECK(height>0),
 relative_path TEXT NOT NULL UNIQUE CHECK(relative_path='assets/'||sha256||'.'||extension), created_at INTEGER NOT NULL,
 gc_state TEXT NOT NULL DEFAULT 'live' CHECK(gc_state IN('live','pending'))
) STRICT;
CREATE TRIGGER assets_immutable BEFORE UPDATE OF sha256,extension,mime,byte_size,width,height,relative_path ON assets
 BEGIN SELECT RAISE(ABORT,'immutable_asset'); END;
CREATE TABLE case_assets (
 id TEXT PRIMARY KEY NOT NULL, case_id TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
 asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT, role TEXT NOT NULL CHECK(role IN('result','reference')),
 position INTEGER NOT NULL CHECK(position>=0 AND (role!='result' OR position=0)), original_name TEXT NOT NULL,
 UNIQUE(case_id,role,position)
) STRICT;
CREATE UNIQUE INDEX case_one_result ON case_assets(case_id) WHERE role='result';
CREATE INDEX case_assets_asset ON case_assets(asset_id);
CREATE TABLE drafts (
 id TEXT PRIMARY KEY NOT NULL, case_id TEXT UNIQUE REFERENCES cases(id) ON DELETE RESTRICT,
 base_revision INTEGER NOT NULL DEFAULT 0 CHECK(base_revision>=0), title TEXT NOT NULL DEFAULT '', prompt TEXT NOT NULL DEFAULT '',
 source_url TEXT NOT NULL DEFAULT '', project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
 updated_at INTEGER NOT NULL, save_seq INTEGER NOT NULL DEFAULT 0 CHECK(save_seq>=0)
) STRICT;
CREATE INDEX drafts_project ON drafts(project_id);
CREATE TABLE draft_assets (
 id TEXT PRIMARY KEY NOT NULL, draft_id TEXT NOT NULL REFERENCES drafts(id) ON DELETE CASCADE,
 asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT, role TEXT NOT NULL CHECK(role IN('result','reference')),
 position INTEGER NOT NULL CHECK(position>=0 AND (role!='result' OR position=0)), original_name TEXT NOT NULL,
 UNIQUE(draft_id,role,position)
) STRICT;
CREATE UNIQUE INDEX draft_one_result ON draft_assets(draft_id) WHERE role='result';
CREATE INDEX draft_assets_asset ON draft_assets(asset_id);
CREATE TABLE draft_tags (
 draft_id TEXT NOT NULL REFERENCES drafts(id) ON DELETE CASCADE,
 tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE, position INTEGER NOT NULL CHECK(position>=0),
 PRIMARY KEY(draft_id,tag_id), UNIQUE(draft_id,position)
) STRICT;
CREATE INDEX draft_tags_tag ON draft_tags(tag_id);
CREATE TABLE settings (key TEXT PRIMARY KEY NOT NULL, value_json TEXT NOT NULL CHECK(json_valid(value_json))) STRICT;
CREATE TABLE synonym_groups (
 id TEXT PRIMARY KEY NOT NULL, terms_json TEXT NOT NULL CHECK(json_valid(terms_json) AND json_type(terms_json)='array'),
 sort_order INTEGER NOT NULL DEFAULT 0
) STRICT;
