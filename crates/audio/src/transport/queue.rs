//! Ordered playback queue with a current-position cursor.

/// Ordered list of track ids, with a cursor pointing at the one currently
/// playing. The cursor is *not* reset by `push`; it's a logical index
/// (`current_index`) advanced by `next`/`previous`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PlaybackQueue {
    items: Vec<i64>,
    current_index: Option<usize>,
}

impl PlaybackQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a track id to the end of the queue. If the queue was empty
    /// (or the cursor was past the end), the cursor advances to the new
    /// item.
    pub fn push(&mut self, track_id: i64) {
        self.items.push(track_id);
        if self.current_index.is_none() {
            self.current_index = Some(self.items.len() - 1);
        }
    }

    /// Drop everything and reset the cursor.
    pub fn clear(&mut self) {
        self.items.clear();
        self.current_index = None;
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn items(&self) -> &[i64] {
        &self.items
    }

    pub fn current(&self) -> Option<i64> {
        self.current_index.and_then(|i| self.items.get(i).copied())
    }

    /// Advance the cursor; return the new current track id, or `None` at
    /// the end of the queue.
    pub fn next(&mut self) -> Option<i64> {
        let i = self.current_index?;
        let next_i = i + 1;
        if next_i < self.items.len() {
            self.current_index = Some(next_i);
            Some(self.items[next_i])
        } else {
            None
        }
    }

    /// Step the cursor back; return the new current track id, or `None`
    /// if already at the first item.
    pub fn previous(&mut self) -> Option<i64> {
        let i = self.current_index?;
        if i == 0 {
            return None;
        }
        self.current_index = Some(i - 1);
        Some(self.items[i - 1])
    }
}
