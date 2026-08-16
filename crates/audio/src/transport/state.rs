//! Transport state machine.

/// Where the transport is in the play/pause/stop lifecycle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TransportState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

impl TransportState {
    /// Start playing from a stopped or paused state. No-op if already playing.
    #[must_use]
    pub fn play(self) -> Self {
        match self {
            Self::Stopped | Self::Paused | Self::Playing => Self::Playing,
        }
    }

    /// Pause if playing. No-op if already stopped or paused.
    #[must_use]
    pub fn pause(self) -> Self {
        match self {
            Self::Playing => Self::Paused,
            other => other,
        }
    }

    /// Resume from paused. No-op if not paused.
    #[must_use]
    pub fn resume(self) -> Self {
        match self {
            Self::Paused => Self::Playing,
            other => other,
        }
    }

    /// Hard-stop. Always lands in `Stopped`.
    #[must_use]
    pub fn stop(self) -> Self {
        Self::Stopped
    }
}
