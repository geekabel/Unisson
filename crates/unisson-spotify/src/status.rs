//! Where Spotify plays, and why it does not play where it should.

use std::time::SystemTime;

/// Where the sound of Spotify comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    /// Inside Unisson, through librespot.
    Builtin,
    /// In the official Spotify application, driven by Unisson.
    App,
}

/// Why the built-in playback does not play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Problem {
    /// Spotify limits it for now: too many requests, or its audio servers
    /// fail. It comes back on its own.
    Limited,
    /// Spotify refuses the audio of this account. It does not come back
    /// on its own: the user asks to try again.
    AccountRefused,
    /// The account is not Premium. The backup needs Premium too.
    PremiumRequired,
    /// Spotify cannot be reached.
    Offline,
}

impl Problem {
    /// Tells whether the backup application can play instead.
    pub fn backup_helps(self) -> bool {
        matches!(self, Self::Limited | Self::AccountRefused)
    }
}

/// Where Spotify plays now, as the interface shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlaybackStatus {
    /// Where the sound comes from. `None` when Spotify cannot play.
    pub route: Option<Route>,
    /// Why the built-in playback is not used, if it is not.
    pub problem: Option<Problem>,
    /// When the built-in playback is tried again on its own.
    pub retry_at: Option<SystemTime>,
}

impl PlaybackStatus {
    /// Spotify plays where it should.
    pub fn normal(route: Option<Route>) -> Self {
        Self {
            route,
            problem: None,
            retry_at: None,
        }
    }
}
