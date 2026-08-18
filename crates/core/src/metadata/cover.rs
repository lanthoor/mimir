//! Cover art selection from a set of embedded picture frames.
//!
//! Pick the front-cover variant if present, otherwise fall back to the
//! first picture. Returns `None` when the list is empty.

use lofty::picture::{MimeType, Picture};

/// A single cover art candidate extracted from a tag block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverArt {
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// Choose the best cover art from `pictures`.
///
/// Preference order:
/// 1. A picture whose `mime_type` matches `image/*` and whose tag type is
///    `CoverFront` (`ID3v2` APIC #0).
/// 2. The first picture in the list.
pub fn select_cover(pictures: &[&Picture]) -> Option<CoverArt> {
    let first = pictures.first()?;
    let picked = pictures
        .iter()
        .find(|p| matches!(p.pic_type(), lofty::picture::PictureType::CoverFront))
        .copied()
        .unwrap_or(*first);
    let mime = picked
        .mime_type()
        .map_or("application/octet-stream", mime_to_str)
        .to_string();
    Some(CoverArt {
        mime_type: mime,
        data: picked.data().to_vec(),
    })
}

fn mime_to_str(m: &MimeType) -> &'static str {
    match m {
        MimeType::Jpeg => "image/jpeg",
        MimeType::Png => "image/png",
        MimeType::Bmp => "image/bmp",
        MimeType::Gif => "image/gif",
        MimeType::Tiff => "image/tiff",
        _ => "application/octet-stream",
    }
}
