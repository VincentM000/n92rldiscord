use pirate_utils::{load_json_state, save_json_state};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const STATE_FILE: &str = "super_xiii_state.json";

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct AppState {
    pub completed_matches: HashSet<i64>,
    pub sent_youtube_videos: HashSet<String>,
    #[serde(default)]
    pub alerted_pre_matches: HashSet<i64>,
    #[serde(default)]
    pub alerted_live_matches: HashSet<i64>,
    pub last_ladder: String,
    pub last_yt_scan_date: Option<String>,
}

impl AppState {
    pub fn load() -> (Self, bool) {
        load_json_state(STATE_FILE)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        save_json_state(STATE_FILE, self)
    }
}
