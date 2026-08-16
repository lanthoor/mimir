-- 0001_base: core tables, FTS5, and triggers.

CREATE TABLE artist (
  id        INTEGER PRIMARY KEY,
  name      TEXT NOT NULL,
  sort_name TEXT COLLATE NOCASE,
  mb_id     TEXT UNIQUE,
  UNIQUE(name)
);
CREATE INDEX artist_sort_idx ON artist(sort_name);

CREATE TABLE album (
  id              INTEGER PRIMARY KEY,
  title           TEXT NOT NULL,
  album_artist_id INTEGER REFERENCES artist(id),
  year            INTEGER,
  mb_id           TEXT UNIQUE,
  release_type    TEXT
);
CREATE INDEX album_artist_idx ON album(album_artist_id);

CREATE TABLE track (
  id          INTEGER PRIMARY KEY,
  path        TEXT NOT NULL UNIQUE,
  path_hash   BLOB NOT NULL,
  mtime_ns    INTEGER NOT NULL,
  size_bytes  INTEGER NOT NULL,
  codec       TEXT NOT NULL,
  duration_ms INTEGER,
  sample_rate INTEGER,
  channels    INTEGER,
  bitrate     INTEGER,
  title       TEXT,
  track_no    INTEGER,
  disc_no     INTEGER,
  album_id    INTEGER REFERENCES album(id),
  fingerprint BLOB,
  mb_id       TEXT,
  missing     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX track_album_idx ON track(album_id);

CREATE TABLE folder (
  id        INTEGER PRIMARY KEY,
  path      TEXT NOT NULL UNIQUE,
  path_hash BLOB NOT NULL,
  active    INTEGER NOT NULL DEFAULT 1,
  added_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE playlist (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  smart       INTEGER NOT NULL DEFAULT 0,
  rules_json  TEXT,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE playlist_track (
  playlist_id INTEGER NOT NULL REFERENCES playlist(id) ON DELETE CASCADE,
  position    INTEGER NOT NULL,
  track_id    INTEGER NOT NULL REFERENCES track(id) ON DELETE CASCADE,
  PRIMARY KEY (playlist_id, position)
);
CREATE INDEX playlist_track_track_idx ON playlist_track(track_id);

-- Full-text search across track title + album/artist/genre/composer strings.
CREATE VIRTUAL TABLE track_fts USING fts5(
  title, album, artist, genre, composer,
  content='track', content_rowid='id', tokenize='unicode61'
);

-- Keep the FTS index in sync with track rows.
CREATE TRIGGER track_ai AFTER INSERT ON track BEGIN
  INSERT INTO track_fts(rowid, title, album, artist, genre, composer)
  VALUES (
    new.id,
    new.title,
    (SELECT title FROM album WHERE id = new.album_id),
    (SELECT name  FROM artist WHERE id IN
       (SELECT album_artist_id FROM album WHERE id = new.album_id)),
    NULL,
    NULL
  );
END;

CREATE TRIGGER track_ad AFTER DELETE ON track BEGIN
  DELETE FROM track_fts WHERE rowid = old.id;
END;

CREATE TRIGGER track_au AFTER UPDATE ON track BEGIN
  DELETE FROM track_fts WHERE rowid = old.id;
  INSERT INTO track_fts(rowid, title, album, artist, genre, composer)
  VALUES (
    new.id,
    new.title,
    (SELECT title FROM album WHERE id = new.album_id),
    (SELECT name  FROM artist WHERE id IN
       (SELECT album_artist_id FROM album WHERE id = new.album_id)),
    NULL,
    NULL
  );
END;
