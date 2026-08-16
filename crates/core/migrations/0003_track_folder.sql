-- 0003_track_folder: link tracks to their originating watched folder.

ALTER TABLE track ADD COLUMN folder_id INTEGER REFERENCES folder(id);
CREATE INDEX track_folder_idx ON track(folder_id);
