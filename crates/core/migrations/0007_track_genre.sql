-- 0007_track_genre: denormalize `track.genre` so we can power the
-- Genres / Years views without scanning the FTS shadow table on every
-- click. Existing rows are backfilled to NULL (re-ingest will populate).
ALTER TABLE track ADD COLUMN genre TEXT;
CREATE INDEX track_genre_idx ON track(genre);
