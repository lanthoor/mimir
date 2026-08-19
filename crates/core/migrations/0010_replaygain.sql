-- 0010_replaygain: per-track + per-album gain in dB.
--
-- Both columns are nullable; absent values mean "track wasn't tagged" or
-- "no ReplayGain analysis is available". Values are signed dB.
ALTER TABLE track ADD COLUMN replaygain_track_db REAL;
ALTER TABLE track ADD COLUMN replaygain_album_db REAL;
