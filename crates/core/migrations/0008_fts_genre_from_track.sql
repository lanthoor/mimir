-- 0008_fts_genre_from_track: now that `track.genre` exists (0007),
-- populate the FTS5 shadow `genre` column from the row instead of NULL.
DROP TRIGGER IF EXISTS track_ai;
DROP TRIGGER IF EXISTS track_au;

CREATE TRIGGER track_ai AFTER INSERT ON track BEGIN
  INSERT INTO track_fts(rowid, title, album, artist, genre, composer)
  VALUES (
    new.id,
    new.title,
    (SELECT title FROM album WHERE id = new.album_id),
    (SELECT name  FROM artist WHERE id IN
       (SELECT album_artist_id FROM album WHERE id = new.album_id)),
    new.genre,
    NULL
  );
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
    new.genre,
    NULL
  );
END;
