use anyhow::{Context, Result};
use std::future::IntoFuture;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession, GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as WinStatus,
};

const TICKS_PER_MILLISECOND: i64 = 10_000;
const WINDOWS_TO_UNIX_EPOCH_MS: i64 = 11_644_473_600_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackState {
    Playing,
    Paused,
}

#[derive(Clone, Debug)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub state: PlaybackState,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub timeline_updated_unix_ms: i64,
    pub artwork_url: Option<String>,
}

impl Track {
    pub fn effective_position_ms(&self, now_ms: i64) -> i64 {
        let elapsed = if self.state == PlaybackState::Playing {
            (now_ms - self.timeline_updated_unix_ms).max(0)
        } else {
            0
        };
        (self.position_ms + elapsed).clamp(0, self.duration_ms.max(0))
    }

    pub fn identity(&self) -> String {
        format!("{}\u{1f}{}\u{1f}{}", self.title, self.artist, self.album)
    }
}

pub struct TidalMedia {
    manager: GlobalSystemMediaTransportControlsSessionManager,
}

impl TidalMedia {
    pub fn connect() -> Result<Self> {
        let manager = futures_lite::future::block_on(
            GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?.into_future(),
        )
        .context("Windows denied global media-session access")?;
        Ok(Self { manager })
    }

    pub fn current(&self) -> Result<Option<Track>> {
        let Some(session) = self.tidal_session()? else {
            return Ok(None);
        };

        let playback = session.GetPlaybackInfo()?;
        let state = match playback.PlaybackStatus()? {
            WinStatus::Playing => PlaybackState::Playing,
            WinStatus::Paused => PlaybackState::Paused,
            _ => return Ok(None),
        };

        let properties =
            futures_lite::future::block_on(session.TryGetMediaPropertiesAsync()?.into_future())?;
        let timeline = session.GetTimelineProperties()?;
        let title = properties.Title()?.to_string();
        if title.trim().is_empty() {
            return Ok(None);
        }

        let artist = properties.Artist()?.to_string();
        let album_artist = properties.AlbumArtist()?.to_string();

        Ok(Some(Track {
            title,
            artist: if artist.trim().is_empty() {
                album_artist
            } else {
                artist
            },
            album: properties.AlbumTitle()?.to_string(),
            state,
            position_ms: timeline.Position()?.Duration / TICKS_PER_MILLISECOND,
            duration_ms: timeline.EndTime()?.Duration / TICKS_PER_MILLISECOND,
            timeline_updated_unix_ms: timeline.LastUpdatedTime()?.UniversalTime
                / TICKS_PER_MILLISECOND
                - WINDOWS_TO_UNIX_EPOCH_MS,
            artwork_url: None,
        }))
    }

    fn tidal_session(&self) -> Result<Option<GlobalSystemMediaTransportControlsSession>> {
        let sessions = self.manager.GetSessions()?;
        for session in &sessions {
            let source = session
                .SourceAppUserModelId()?
                .to_string()
                .to_ascii_lowercase();
            if source.contains("tidal") {
                return Ok(Some(session));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playing_position_advances_from_timeline_update() {
        let track = Track {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            state: PlaybackState::Playing,
            position_ms: 10_000,
            duration_ms: 30_000,
            timeline_updated_unix_ms: 100_000,
            artwork_url: None,
        };
        assert_eq!(track.effective_position_ms(103_500), 13_500);
    }

    #[test]
    fn paused_position_stays_fixed() {
        let mut track = Track {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            state: PlaybackState::Playing,
            position_ms: 10_000,
            duration_ms: 30_000,
            timeline_updated_unix_ms: 100_000,
            artwork_url: None,
        };
        track.state = PlaybackState::Paused;
        assert_eq!(track.effective_position_ms(103_500), 10_000);
    }
}
