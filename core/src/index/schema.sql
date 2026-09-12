CREATE TABLE IF NOT EXISTS meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notes (
  path TEXT PRIMARY KEY,
  path_lower TEXT NOT NULL,
  stem_lower TEXT NOT NULL,
  title TEXT NOT NULL,
  mtime_ms INTEGER NOT NULL,
  size INTEGER NOT NULL,
  hash TEXT NOT NULL,
  frontmatter TEXT NOT NULL,
  body TEXT NOT NULL,
  body_line INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS notes_stem ON notes(stem_lower);
CREATE INDEX IF NOT EXISTS notes_path_lower ON notes(path_lower);

CREATE TABLE IF NOT EXISTS links (
  src_path TEXT NOT NULL REFERENCES notes(path) ON DELETE CASCADE,
  target_raw TEXT NOT NULL,
  target_path TEXT,
  kind TEXT NOT NULL,
  heading TEXT,
  alias TEXT,
  line INTEGER NOT NULL,
  start INTEGER NOT NULL,
  "end" INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS links_src ON links(src_path);
CREATE INDEX IF NOT EXISTS links_target ON links(target_path);

CREATE TABLE IF NOT EXISTS tags (
  path TEXT NOT NULL REFERENCES notes(path) ON DELETE CASCADE,
  tag TEXT NOT NULL,
  PRIMARY KEY (path, tag)
);
CREATE INDEX IF NOT EXISTS tags_tag ON tags(tag);

CREATE TABLE IF NOT EXISTS properties (
  path TEXT NOT NULL REFERENCES notes(path) ON DELETE CASCADE,
  key TEXT NOT NULL,
  value_json TEXT NOT NULL,
  value_type TEXT NOT NULL,
  PRIMARY KEY (path, key)
);
CREATE INDEX IF NOT EXISTS properties_key ON properties(key);

CREATE TABLE IF NOT EXISTS headings (
  path TEXT NOT NULL REFERENCES notes(path) ON DELETE CASCADE,
  level INTEGER NOT NULL,
  text TEXT NOT NULL,
  line INTEGER NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
  title, body, path UNINDEXED,
  content='notes', content_rowid='rowid',
  tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS notes_ai AFTER INSERT ON notes BEGIN
  INSERT INTO notes_fts(rowid, title, body, path) VALUES (new.rowid, new.title, new.body, new.path);
END;
CREATE TRIGGER IF NOT EXISTS notes_ad AFTER DELETE ON notes BEGIN
  INSERT INTO notes_fts(notes_fts, rowid, title, body, path) VALUES ('delete', old.rowid, old.title, old.body, old.path);
END;
CREATE TRIGGER IF NOT EXISTS notes_au AFTER UPDATE ON notes BEGIN
  INSERT INTO notes_fts(notes_fts, rowid, title, body, path) VALUES ('delete', old.rowid, old.title, old.body, old.path);
  INSERT INTO notes_fts(rowid, title, body, path) VALUES (new.rowid, new.title, new.body, new.path);
END;
