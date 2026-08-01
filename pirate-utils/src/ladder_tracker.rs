use chrono::{Datelike, Weekday};
use log::{info, warn};
use reqwest::blocking::Client;
use std::thread;
use std::time::Duration;

use crate::{apply_jitter, load_state, save_state, send_discord_alert};

pub trait LadderTracker {
    fn name(&self) -> &str;
    fn state_filename(&self) -> &str;
    fn fetch_ladder(&self, client: &Client) -> Result<String, Box<dyn std::error::Error>>;
    fn fetch_latest_match_result(
        &self,
        client: &Client,
    ) -> Result<Option<String>, Box<dyn std::error::Error>>;
}

pub fn get_sleep_duration() -> Duration {
    let weekday = chrono::Utc::now().weekday();
    let base_secs = match weekday {
        Weekday::Tue | Weekday::Wed => 3600,
        _ => 600,
    };
    apply_jitter(base_secs)
}

pub fn run_tracker_loop<T: LadderTracker>(tracker: T) {
    info!(
        "🏴‍☠️ Hoisting the colors! The {} will check the horizon with smart adaptive scheduling...",
        tracker.name()
    );

    let client = Client::builder()
        .user_agent("NanterrePirates-Scraper/1.0")
        .build()
        .expect("Failed to build HTTP client");

    loop {
        if let Err(e) = check_ladder_step(&client, &tracker) {
            warn!("⚠️ Arrr! Hit a reef checking the {}: {}", tracker.name(), e);
        }

        let sleep_duration = get_sleep_duration();
        thread::sleep(sleep_duration);
    }
}

fn check_ladder_step<T: LadderTracker>(
    client: &Client,
    tracker: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_ladder = tracker.fetch_ladder(client)?;
    let state_file = tracker.state_filename();
    let last_ladder = load_state(state_file);

    if current_ladder != last_ladder && !current_ladder.is_empty() {
        if !last_ladder.is_empty() {
            info!(
                "🏴‍☠️ The {} ladder has shifted! Fetching the latest match result...",
                tracker.name()
            );
            match tracker.fetch_latest_match_result(client) {
                Ok(Some(result_message)) => {
                    if let Err(e) = send_discord_alert(client, &result_message) {
                        warn!("⚠️ Arrr! Failed to send match result to Discord: {}", e);
                    }

                    info!("⏳ Pausing 2 seconds for message sequencing...");
                    thread::sleep(Duration::from_secs(2));
                }
                Ok(None) => info!("⚠️ No recent match result found on page."),
                Err(e) => warn!("⚠️ Arrr! Error fetching match result: {}", e),
            }
        }

        info!("🏴‍☠️ Firing updated ladder to Discord...");
        send_discord_alert(client, &current_ladder)?;
    }

    if current_ladder != last_ladder {
        save_state(state_file, &current_ladder)?;
    }

    Ok(())
}
