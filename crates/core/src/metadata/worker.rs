//! Per-file ingest: probe + extract + upsert artist/album/track in a tx.

use std::path::Path;

use rusqlite::Connection;
use thiserror::Error;

use crate::db::{attach_album_cover, upsert_lyrics};
use crate::scanner::ScanJob;
use mimir_telemetry as telemetry;

use super::extract::{extract_tags, Tags};
use super::heuristic::{parse_filename, HeuristicTags};
use super::probe::{extract_cover, probe_file, Probe, ProbeError};
use super::upsert::{upsert_album, upsert_artist};

#[derive(Debug, Error)]
pub enum IngestError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("probe: {0}")]
    Probe(String),
}

/// Ingest a single `ScanJob`: probe the file, extract tags (or fall back to
/// filename heuristics), upsert artist + album + track in one transaction.
///
/// Returns the new track id. Idempotent on `(path_hash, mtime_ns,
/// size_bytes)`: a second call with the same triple updates in place rather
/// than inserting a duplicate.
#[allow(clippy::too_many_lines)]
pub fn ingest(conn: &Connection, job: ScanJob) -> Result<i64, IngestError> {
    let ScanJob {
        folder_id,
        path,
        file_hash,
    } = job;

    telemetry::log(
        "INFO",
        "ingest",
        &format!("ingest start folder_id={folder_id} path={}", path.display()),
    );

    let probe = match probe_or_default(&path) {
        Ok(p) => p,
        Err(e) => {
            telemetry::log(
                "ERROR",
                "ingest",
                &format!("probe failed path={} err={e}", path.display()),
            );
            return Err(e);
        }
    };
    telemetry::log(
        "DEBUG",
        "ingest",
        &format!("probe result path={} codec={}", path.display(), probe.codec),
    );

    let mut tags = match extract_tags(&path) {
        Ok(t) => t,
        Err(e) => {
            telemetry::log(
                "WARN",
                "ingest",
                &format!(
                    "extract_tags err using default path={} err={e}",
                    path.display()
                ),
            );
            Tags::default()
        }
    };
    apply_heuristic(&path, &mut tags);

    let artist_name = tags.artist.as_deref().or(tags.album_artist.as_deref());
    let artist_id = if let Some(name) = artist_name {
        let id = upsert_artist(conn, name)?;
        telemetry::log(
            "DEBUG",
            "ingest",
            &format!("artist upserted id={id} name={name}"),
        );
        id
    } else {
        let id: i64 = conn.query_row(
            "SELECT id FROM artist WHERE name = 'Unknown Artist'",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        telemetry::log(
            "DEBUG",
            "ingest",
            &format!("artist fallback Unknown Artist id={id}"),
        );
        id
    };

    let album_id = if let Some(album_title) = tags.album.as_deref() {
        let id = upsert_album(conn, album_title, artist_id, tags.year)?;
        telemetry::log(
            "DEBUG",
            "ingest",
            &format!(
                "album upserted id={id} title={album_title} year={:?}",
                tags.year
            ),
        );
        Some(id)
    } else {
        telemetry::log("DEBUG", "ingest", "no album title → album_id=None");
        None
    };

    if let (Some(album_id), Ok(Some(cover))) = (album_id, extract_cover(&path)) {
        match attach_album_cover(conn, album_id, &cover, "embedded") {
            Ok(_) => telemetry::log(
                "DEBUG",
                "ingest",
                &format!(
                    "cover attached album_id={album_id} mime={}",
                    cover.mime_type
                ),
            ),
            Err(e) => telemetry::log(
                "WARN",
                "ingest",
                &format!("attach_album_cover failed album_id={album_id} err={e}"),
            ),
        }
    }

    let tx = conn.unchecked_transaction()?;

    let path_str = path.to_string_lossy().into_owned();
    let codec = &probe.codec;
    let duration_ms = probe.duration_ms;
    let sample_rate = probe.sample_rate;
    let channels = probe.channels.map(i32::from);
    let bitrate = probe.bitrate;
    let title = tags.title.as_deref();
    let genre = tags.genre.as_deref();
    let track_no = tags.track_no.map(i64::from);
    let disc_no = tags.disc_no.map(i64::from);

    let updated = tx.execute(
        "INSERT INTO track (\
            path, path_hash, mtime_ns, size_bytes, codec, duration_ms, sample_rate, \
            channels, bitrate, title, genre, replaygain_track_db, replaygain_album_db, \
            track_no, disc_no, album_id, folder_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)\
         ON CONFLICT(path_hash, mtime_ns, size_bytes) DO UPDATE SET \
            path = excluded.path, codec = excluded.codec, \
            duration_ms = excluded.duration_ms, sample_rate = excluded.sample_rate, \
            channels = excluded.channels, bitrate = excluded.bitrate, \
            title = excluded.title, genre = excluded.genre, \
            replaygain_track_db = excluded.replaygain_track_db, \
            replaygain_album_db = excluded.replaygain_album_db, \
            track_no = excluded.track_no, disc_no = excluded.disc_no, \
            album_id = excluded.album_id, folder_id = excluded.folder_id",
        rusqlite::params![
            path_str,
            &file_hash.path_hash[..],
            file_hash.mtime_ns,
            file_hash.size_bytes,
            codec,
            duration_ms,
            sample_rate,
            channels,
            bitrate,
            title,
            genre,
            tags.replaygain_track_db,
            tags.replaygain_album_db,
            track_no,
            disc_no,
            album_id,
            folder_id,
        ],
    )?;
    telemetry::log(
        "DEBUG",
        "ingest",
        &format!("track upsert changed={updated} path={path_str}"),
    );

    let track_id: i64 = tx.query_row(
        "SELECT id FROM track WHERE path_hash = ?1 AND mtime_ns = ?2 AND size_bytes = ?3",
        rusqlite::params![
            &file_hash.path_hash[..],
            file_hash.mtime_ns,
            file_hash.size_bytes
        ],
        |row| row.get(0),
    )?;

    if let Some(lyrics) = tags.lyrics.as_deref() {
        match upsert_lyrics(conn, track_id, lyrics, "und", "embedded") {
            Ok(()) => telemetry::log(
                "DEBUG",
                "ingest",
                &format!("lyrics attached track_id={track_id} bytes={}", lyrics.len()),
            ),
            Err(e) => telemetry::log(
                "WARN",
                "ingest",
                &format!("lyrics upsert failed track_id={track_id} err={e}"),
            ),
        }
    }

    tx.commit()?;
    telemetry::log(
        "INFO",
        "ingest",
        &format!(
            "ingest done track_id={track_id} folder_id={folder_id} codec={codec} title={title:?} album_id={album_id:?} artist_id={artist_id} path={}",
            path.display()
        ),
    );
    Ok(track_id)
}

fn probe_or_default(path: &Path) -> Result<Probe, IngestError> {
    probe_file(path).map_err(|e| {
        telemetry::log(
            "WARN",
            "ingest",
            &format!("probe_or_default fallback err={e}"),
        );
        match e {
            ProbeError::Lofty(s) | ProbeError::UnknownExtension(s) => IngestError::Probe(s),
            ProbeError::Io(io) => IngestError::Io(io),
        }
    })
}

fn apply_heuristic(path: &Path, tags: &mut Tags) {
    if let Some(HeuristicTags {
        artist,
        album,
        track_no,
        title,
    }) = parse_filename(path)
    {
        let before = tags.clone();
        if tags.artist.is_none() && tags.album_artist.is_none() {
            tags.artist = artist;
        }
        if tags.album.is_none() {
            tags.album = album;
        }
        if tags.track_no.is_none() {
            tags.track_no = track_no;
        }
        if tags.title.is_none() {
            tags.title = title;
        }
        telemetry::log(
            "DEBUG",
            "ingest",
            &format!(
                "heuristic merged path={} before={:?} after={:?}",
                path.display(),
                before,
                tags
            ),
        );
    }
}

/// Drain `rx` and call `ingest` for each job. Returns when the sender
/// closes the channel. Per-job errors are logged to the file (and stderr).
/// The worker keeps going.
#[allow(clippy::needless_pass_by_value)]
pub fn run_worker(conn: &Connection, rx: std::sync::mpsc::Receiver<ScanJob>) {
    telemetry::log("INFO", "worker", "run_worker started");
    let mut processed = 0u64;
    while let Ok(job) = rx.recv() {
        processed += 1;
        telemetry::log(
            "DEBUG",
            "worker",
            &format!(
                "recv job n={processed} folder_id={} path={}",
                job.folder_id,
                job.path.display()
            ),
        );
        if let Err(e) = ingest(conn, job) {
            telemetry::log("ERROR", "ingest", &format!("{e}"));
        }
    }
    telemetry::log(
        "INFO",
        "worker",
        &format!("run_worker stopping after {processed} jobs"),
    );
}
