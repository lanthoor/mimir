//! External (sidecar) metadata loading.
//!
//! Some libraries carry metadata in files that sit alongside the audio:
//! `.lrc` for lyrics, `cover.jpg` / `folder.jpg` for cover art. These are
//! loaded as a fallback during ingest when the embedded tags don't have the
//! data (e.g. the audio file has no embedded lyrics, or no embedded cover).
//!
//! Conventions:
//! - Lyrics: `<basename>.lrc` or `<basename>.txt` in the same directory.
//!   `.lrc` files may contain `[mm:ss.xx]` timing lines; we strip those
//!   for the unsynced text body (the DB stores unsynced text only).
//! - Cover: `cover.{jpg,jpeg,png}`, `folder.{jpg,jpeg,png}`,
//!   `front.{jpg,jpeg,png}` in the same directory. Checked in that order;
//!   first hit wins.

use std::path::{Path, PathBuf};

/// Lyrics loaded from a sidecar file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarLyrics {
    pub text: String,
    pub language: String,
    pub source: String,
}

/// Cover art loaded from a sidecar file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarCover {
    pub mime_type: String,
    pub data: Vec<u8>,
    pub source: String,
}

/// Look for a lyrics sidecar next to `audio_path` and return its body.
///
/// Matches `<basename>.lrc` first, then `<basename>.txt`. Returns `None`
/// if neither file exists or is unreadable.
pub fn find_lyrics_sidecar(audio_path: &Path) -> Option<SidecarLyrics> {
    let stem = audio_path.file_stem()?;
    let dir = audio_path.parent()?;
    for ext in ["lrc", "txt"] {
        let candidate = dir.join(format!("{}.{}", stem.to_string_lossy(), ext));
        if let Some(lyrics) = read_lyrics_file(&candidate) {
            return Some(lyrics);
        }
    }
    None
}

fn read_lyrics_file(path: &Path) -> Option<SidecarLyrics> {
    let raw = std::fs::read_to_string(path).ok()?;
    let (text, lang) = strip_lrc_timing(&raw);
    let text = text.trim().to_string();
    if text.is_empty() {
        return None;
    }
    let source = format!("sidecar:{}", path.file_name()?.to_string_lossy());
    Some(SidecarLyrics {
        text,
        language: lang.unwrap_or_else(|| "und".into()),
        source,
    })
}

/// Strip LRC-style `[mm:ss.xx]` timing prefixes from each line. Returns the
/// cleaned body and a detected language tag (from `[lang:xx]` metadata).
/// Lines that are empty after stripping are dropped.
fn strip_lrc_timing(raw: &str) -> (String, Option<String>) {
    let mut lang: Option<String> = None;
    let mut out = String::with_capacity(raw.len());
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // LRC metadata: `[lang:en]`, `[ti:Title]`, `[ar:Artist]`, etc.
        if let Some(rest) = trimmed.strip_prefix('[') {
            if let Some(close) = rest.find(']') {
                let tag = &rest[..close];
                if let Some(value) = tag.strip_prefix("lang:") {
                    lang = Some(value.to_string());
                    continue;
                }
                // Other metadata tags are not part of the lyrics body.
                if !tag.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                    continue;
                }
            }
        }
        // Synchronized line: `[mm:ss.xx]lyrics text`. Keep only the text.
        let body = if trimmed.starts_with('[') {
            if let Some(close) = trimmed.find(']') {
                trimmed[close + 1..].trim()
            } else {
                trimmed
            }
        } else {
            trimmed
        };
        if !body.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(body);
        }
    }
    (out, lang)
}

/// Look for a cover-art sidecar in the same directory as `audio_path`.
///
/// Search order (first hit wins):
/// 1. `cover.jpg`, `cover.jpeg`, `cover.png`
/// 2. `folder.jpg`, `folder.jpeg`, `folder.png`
/// 3. `front.jpg`, `front.jpeg`, `front.png`
pub fn find_cover_sidecar(audio_path: &Path) -> Option<SidecarCover> {
    let dir = audio_path.parent()?;
    for base in ["cover", "folder", "front"] {
        for ext in ["jpg", "jpeg", "png"] {
            let p: PathBuf = dir.join(format!("{base}.{ext}"));
            if let Some(cover) = read_cover_file(&p) {
                return Some(cover);
            }
        }
    }
    None
}

fn read_cover_file(path: &Path) -> Option<SidecarCover> {
    let ext = path.extension()?.to_ascii_lowercase();
    let mime = match ext.to_string_lossy().as_ref() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        _ => return None,
    };
    let data = std::fs::read(path).ok()?;
    if data.is_empty() {
        return None;
    }
    let source = format!("sidecar:{}", path.file_name()?.to_string_lossy());
    Some(SidecarCover {
        mime_type: mime.into(),
        data,
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_lrc_timing_removes_timestamps_and_metadata() {
        let raw = "[ti:Bohemian Rhapsody]\n\
                   [ar:Queen]\n\
                   [lang:en]\n\
                   [00:00.00]Is this the real life?\n\
                   [00:05.12]Is this just fantasy?\n\
                   \n\
                   [00:11.00]Caught in a landslide\n";
        let (body, lang) = strip_lrc_timing(raw);
        assert_eq!(
            body,
            "Is this the real life?\nIs this just fantasy?\nCaught in a landslide"
        );
        assert_eq!(lang.as_deref(), Some("en"));
    }

    #[test]
    fn strip_lrc_timing_keeps_plain_text() {
        let raw = "All you need is love\nLove is all you need\n";
        let (body, lang) = strip_lrc_timing(raw);
        assert_eq!(body, "All you need is love\nLove is all you need");
        assert_eq!(lang, None);
    }

    #[test]
    fn find_lyrics_sidecar_reads_lrc() {
        let dir = tempfile::tempdir().expect("tempdir");
        let audio = dir.path().join("song.mp3");
        std::fs::write(&audio, b"fake").expect("write");
        let lrc = dir.path().join("song.lrc");
        std::fs::write(&lrc, "[00:01.00]hello\n[00:02.00]world\n").expect("write");

        let out = find_lyrics_sidecar(&audio).expect("lyrics");
        assert_eq!(out.text, "hello\nworld");
        assert!(out.source.starts_with("sidecar:"));
    }

    #[test]
    fn find_lyrics_sidecar_returns_none_when_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let audio = dir.path().join("song.mp3");
        std::fs::write(&audio, b"fake").expect("write");
        assert!(find_lyrics_sidecar(&audio).is_none());
    }

    #[test]
    fn find_cover_sidecar_picks_cover_over_folder() {
        let dir = tempfile::tempdir().expect("tempdir");
        let audio = dir.path().join("song.mp3");
        std::fs::write(&audio, b"fake").expect("write");
        std::fs::write(dir.path().join("folder.jpg"), b"folder-bytes").expect("write");
        std::fs::write(dir.path().join("cover.jpg"), b"cover-bytes").expect("write");

        let out = find_cover_sidecar(&audio).expect("cover");
        assert_eq!(out.mime_type, "image/jpeg");
        assert_eq!(out.data, b"cover-bytes");
        assert!(out.source.contains("cover.jpg"));
    }

    #[test]
    fn find_cover_sidecar_falls_back_to_folder() {
        let dir = tempfile::tempdir().expect("tempdir");
        let audio = dir.path().join("song.mp3");
        std::fs::write(&audio, b"fake").expect("write");
        std::fs::write(dir.path().join("folder.png"), b"folder-png").expect("write");

        let out = find_cover_sidecar(&audio).expect("cover");
        assert_eq!(out.mime_type, "image/png");
        assert_eq!(out.data, b"folder-png");
    }

    #[test]
    fn find_cover_sidecar_returns_none_when_no_candidate() {
        let dir = tempfile::tempdir().expect("tempdir");
        let audio = dir.path().join("song.mp3");
        std::fs::write(&audio, b"fake").expect("write");
        assert!(find_cover_sidecar(&audio).is_none());
    }
}
