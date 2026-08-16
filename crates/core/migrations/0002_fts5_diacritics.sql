-- 0002_fts5_diacritics: recreate the FTS5 virtual table with
-- `unicode61 remove_diacritics 2` so diacritic-insensitive search works.

DROP TRIGGER IF EXISTS track_ai;
DROP TRIGGER IF EXISTS track_ad;
DROP TRIGGER IF EXISTS track_au;
DROP TABLE IF EXISTS track_fts;

CREATE VIRTUAL TABLE track_fts USING fts5(
  title, album, artist, genre, composer,
  tokenize="unicode61 remove_diacritics 2"
);

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

INSERT INTO track_fts(track_fts, rowid, title, album, artist, genre, composer)
  SELECT 'rebuild',
         t.id,
         t.title,
         (SELECT title FROM album WHERE id = t.album_id),
         (SELECT name  FROM artist WHERE id IN
            (SELECT album_artist_id FROM album WHERE id = t.album_id)),
         NULL,
         NULL
  FROM track t;
