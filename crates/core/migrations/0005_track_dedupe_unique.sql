-- 0005_track_dedupe_unique: enable upsert on (path_hash, mtime_ns, size_bytes)
-- so re-ingests overwrite existing rows instead of failing.
CREATE UNIQUE INDEX track_dedupe_idx ON track(path_hash, mtime_ns, size_bytes);
