-- 0009_lyrics: per-track lyrics storage.
--
-- One row per (track_id, language). Today we only store the unsynced
-- lyrics body extracted from USLT / Vorbis `LYRICS`; LRC-style timestamps
-- could fit the `synced` column later if we want audio sync.
CREATE TABLE lyrics (
  track_id  INTEGER NOT NULL REFERENCES track(id) ON DELETE CASCADE,
  language  TEXT    NOT NULL DEFAULT 'und',
  text      TEXT    NOT NULL,
  synced    TEXT,
  source    TEXT    NOT NULL DEFAULT 'embedded',
  PRIMARY KEY (track_id, language)
);
CREATE INDEX lyrics_source_idx ON lyrics(source);
