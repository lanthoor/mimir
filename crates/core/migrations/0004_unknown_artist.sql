-- 0004_unknown_artist: seed a placeholder artist so untagged tracks can FK.
INSERT OR IGNORE INTO artist (name, sort_name) VALUES ('Unknown Artist', 'unknown artist');
