use std::time::{Duration, SystemTime, UNIX_EPOCH};

use discord_rich_presence::{
    DiscordIpc, DiscordIpcClient,
    activity::{Activity, ActivityType, Assets, Button, StatusDisplayType, Timestamps},
};

use crate::media::{PlaybackState, Track};

const RESYNC_INTERVAL: Duration = Duration::from_secs(15);

pub struct DiscordPresence {
    client: DiscordIpcClient,
    connected: bool,
    last_signature: String,
    last_update: SystemTime,
}

impl DiscordPresence {
    pub fn new(app_id: String) -> Self {
        Self {
            client: DiscordIpcClient::new(app_id),
            connected: false,
            last_signature: String::new(),
            last_update: UNIX_EPOCH,
        }
    }

    pub fn publish(&mut self, track: &Track) {
        let now = SystemTime::now();
        let now_ms = unix_ms(now);
        let position_ms = track.effective_position_ms(now_ms);
        let signature = format!(
            "{}|{:?}|{}|{}",
            track.identity(),
            track.state,
            position_ms / 1_000,
            track.artwork_url.as_deref().unwrap_or_default()
        );
        let needs_resync = track.state == PlaybackState::Playing
            && now.duration_since(self.last_update).unwrap_or_default() >= RESYNC_INTERVAL;
        let state_changed = signature_without_live_position(&signature)
            != signature_without_live_position(&self.last_signature);
        let paused_position_changed =
            track.state == PlaybackState::Paused && signature != self.last_signature;

        if !state_changed && !paused_position_changed && !needs_resync {
            return;
        }
        if !self.ensure_connected() {
            return;
        }

        let progress = format_duration(position_ms);
        let duration = format_duration(track.duration_ms);
        let state = match track.state {
            PlaybackState::Playing => format!("{} • Playing", track.artist),
            PlaybackState::Paused => {
                format!("{} • Paused at {progress} / {duration}", track.artist)
            }
        };
        let tidal_url = format!(
            "https://tidal.com/search?q={}",
            percent_encode(&format!("{} {}", track.title, track.artist))
        );

        let mut activity = Activity::new()
            .activity_type(ActivityType::Listening)
            .status_display_type(StatusDisplayType::Details)
            .details(truncate(&track.title, 128))
            .state(truncate(&state, 128))
            .buttons(vec![Button::new("Listen on TIDAL", tidal_url)]);

        if track.state == PlaybackState::Playing && track.duration_ms > 0 {
            let start = now_ms - position_ms;
            activity = activity.timestamps(
                Timestamps::new()
                    .start(start)
                    .end(start + track.duration_ms),
            );
        }

        if let Some(url) = track.artwork_url.as_deref() {
            let label = if track.album.trim().is_empty() {
                track.title.as_str()
            } else {
                track.album.as_str()
            };
            activity = activity.assets(
                Assets::new()
                    .large_image(url)
                    .large_text(truncate(label, 128)),
            );
        }

        if let Err(error) = self.client.set_activity(activity) {
            eprintln!("Discord update failed: {error}");
            self.connected = false;
            return;
        }
        self.last_signature = signature;
        self.last_update = now;
    }

    pub fn clear(&mut self) {
        if self.connected && !self.last_signature.is_empty() {
            if self.client.clear_activity().is_err() {
                self.connected = false;
            }
            self.last_signature.clear();
        }
    }

    fn ensure_connected(&mut self) -> bool {
        if self.connected {
            return true;
        }
        match self.client.connect() {
            Ok(()) => {
                self.connected = true;
                true
            }
            Err(_) => false,
        }
    }
}

fn unix_ms(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn format_duration(milliseconds: i64) -> String {
    let seconds = milliseconds.max(0) / 1_000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn signature_without_live_position(signature: &str) -> String {
    let mut parts = signature.split('|').collect::<Vec<_>>();
    if parts.len() >= 3 {
        parts[2] = "";
    }
    parts.join("|")
}

fn percent_encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            output.push(byte as char);
        } else {
            output.push_str(&format!("%{byte:02X}"));
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_progress() {
        assert_eq!(format_duration(183_900), "3:03");
    }

    #[test]
    fn encodes_utf8_search_text() {
        assert_eq!(
            percent_encode("Beyoncé & Jay-Z"),
            "Beyonc%C3%A9%20%26%20Jay-Z"
        );
    }
}
