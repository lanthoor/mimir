-- 0006_cover_art: per-album cover art bytes + mime.
--
-- One row per (mime, content_hash) tuple so covers dedupe across albums
-- that share the same image (e.g. a "Various Artists" rerelease).
-- We store the *primary* cover on album (`album.cover_art_id`) and keep a
-- unique index on its content_hash for cache-friendly identity.
CREATE TABLE cover_art (
  id           INTEGER PRIMARY KEY,
  mime_type    TEXT    NOT NULL,
  data         BLOB    NOT NULL,
  content_hash BLOB    NOT NULL UNIQUE,
  width        INTEGER,
  height       INTEGER,
  source       TEXT    NOT NULL DEFAULT 'embedded',
  created_at   INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX cover_art_source_idx ON cover_art(source);

ALTER TABLE album ADD COLUMN cover_art_id INTEGER REFERENCES cover_art(id);
CREATE INDEX album_cover_art_idx ON album(cover_art_id);
