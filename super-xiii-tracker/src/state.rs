use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

const STATE_FILE: &str = "super_xiii_state.json";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppState {
    pub completed_matches: HashSet<i64>,
    pub sent_youtube_videos: HashSet<String>,
    #[serde(default)]
    pub alerted_pre_matches: HashSet<i64>,
    pub last_ladder: String,
    pub last_yt_scan_date: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            completed_matches: HashSet::new(),
            sent_youtube_videos: HashSet::new(),
            alerted_pre_matches: HashSet::new(),
            last_ladder: String::new(),
            last_yt_scan_date: None,
        }
    }
}

impl AppState {
    pub fn load() -> (Self, bool) {
        if Path::new(STATE_FILE).exists() {
            if let Ok(content) = fs::read_to_string(STATE_FILE) {
                if let Ok(state) = serde_json::from_str::<AppState>(&content) {
                    return (state, false);
                }
            }
        }
        (AppState::default(), true)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(STATE_FILE, content)?;
        Ok(())
    }
}

