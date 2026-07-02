use std::{collections::HashMap, time::Duration};

use anyhow::Result;
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::media::Track;

#[derive(Deserialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchResult {
    #[serde(default)]
    track_name: String,
    #[serde(default)]
    artist_name: String,
    #[serde(default)]
    collection_name: String,
    artwork_url100: Option<String>,
}

pub struct ArtworkResolver {
    client: Client,
    cache: HashMap<String, Option<String>>,
}

impl ArtworkResolver {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .user_agent(concat!("tidal-discord-rpc/", env!("CARGO_PKG_VERSION")))
                .timeout(Duration::from_secs(5))
                .build()?,
            cache: HashMap::new(),
        })
    }

    pub fn resolve(&mut self, track: &Track) -> Option<String> {
        let key = track.identity();
        if let Some(cached) = self.cache.get(&key) {
            return cached.clone();
        }

        let found = self.lookup(track).ok().flatten();
        self.cache.insert(key, found.clone());
        found
    }

    fn lookup(&self, track: &Track) -> Result<Option<String>> {
        let query = format!("{} {} {}", track.title, track.artist, track.album);
        let response = self
            .client
            .get("https://itunes.apple.com/search")
            .query(&[
                ("term", query.as_str()),
                ("entity", "song"),
                ("limit", "10"),
            ])
            .send()?
            .error_for_status()?
            .json::<SearchResponse>()?;

        let best = response
            .results
            .into_iter()
            .filter_map(|candidate| {
                let score = match_score(track, &candidate);
                candidate.artwork_url100.map(|url| (score, url))
            })
            .filter(|(score, _)| *score >= 8)
            .max_by_key(|(score, _)| *score);

        Ok(best.map(|(_, url)| high_resolution(url)))
    }
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn compatible(left: &str, right: &str) -> bool {
    let left = normalize(left);
    let right = normalize(right);
    !left.is_empty() && !right.is_empty() && (left.contains(&right) || right.contains(&left))
}

fn match_score(track: &Track, candidate: &SearchResult) -> u8 {
    let mut score = 0;
    if normalize(&track.title) == normalize(&candidate.track_name) {
        score += 6;
    }
    if compatible(&track.artist, &candidate.artist_name) {
        score += 4;
    }
    if compatible(&track.album, &candidate.collection_name) {
        score += 3;
    }
    score
}

fn high_resolution(url: String) -> String {
    url.replace("/100x100bb.", "/600x600bb.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::PlaybackState;

    fn track() -> Track {
        Track {
            title: "Get Lucky".into(),
            artist: "Daft Punk".into(),
            album: "Random Access Memories".into(),
            state: PlaybackState::Playing,
            position_ms: 0,
            duration_ms: 1,
            timeline_updated_unix_ms: 0,
            artwork_url: None,
        }
    }

    #[test]
    fn accepts_featured_artist_and_exact_album() {
        let candidate = SearchResult {
            track_name: "Get Lucky".into(),
            artist_name: "Daft Punk, Pharrell Williams & Nile Rodgers".into(),
            collection_name: "Random Access Memories".into(),
            artwork_url100: None,
        };
        assert_eq!(match_score(&track(), &candidate), 13);
    }

    #[test]
    fn rejects_unrelated_song() {
        let candidate = SearchResult {
            track_name: "Around the World".into(),
            artist_name: "Daft Punk".into(),
            collection_name: "Homework".into(),
            artwork_url100: None,
        };
        assert!(match_score(&track(), &candidate) < 8);
    }
}
