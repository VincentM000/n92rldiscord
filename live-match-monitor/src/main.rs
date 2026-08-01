use chrono::{DateTime, Utc};
use log::{info, warn};
use pirate_utils::{apply_jitter, load_state, save_state, send_discord_alert};
use reqwest::blocking::Client;
use serde_json::Value;
use std::thread;
use std::time::Duration;

// The raw JSON API endpoint using Rugbyrama's exact internal filters
const API_URL: &str = "https://api-live.rugbyrama.fr/api/videos?channel=4&limit=5&order=asc&order_by=event_date&status[]=running&status[]=pending&status[]=ready&status[]=scheduled&type[]=live";
// The user-friendly link we will send in the Discord message
const WEB_URL: &str = "https://live.rugbyrama.fr/fr/1-directs/?ch=4";
const STATE_FILE: &str = "sl_state.txt";

fn main() {
    env_logger::init();

    let client = Client::builder()
        .user_agent("NanterrePirates-Scraper/1.0")
        .build()
        .expect("Failed to build HTTP client");

    info!(
        "🏴‍☠️ Stealth Live Match Monitor started. Using adaptive radar with jitter to avoid bans..."
    );

    // The dynamic loop
    loop {
        let sleep_duration = match check_rugbyrama_api(&client) {
            Ok(duration) => duration,
            Err(e) => {
                warn!(
                    "⚠️ Error during API check: {}. Falling back to 10 min sleep.",
                    e
                );
                apply_jitter(600) // Default to ~10 mins on error
            }
        };

        info!(
            "💤 Sleeping for {} seconds before the next check...",
            sleep_duration.as_secs()
        );
        thread::sleep(sleep_duration);
    }
}

fn check_rugbyrama_api(client: &Client) -> Result<Duration, Box<dyn std::error::Error>> {
    let json_response: Value = client.get(API_URL).send()?.json()?;

    let items = match json_response["items"].as_array() {
        Some(arr) => arr,
        None => return Ok(apply_jitter(3600)), // No matches? Sleep ~1 hour
    };

    let mut current_status = "unknown";
    let mut closest_match_in_secs: Option<i64> = None;

    for item in items {
        let status = item["status"].as_str().unwrap_or("unknown");

        // Analyze the time for ALL matches to find the closest one
        if let Some(date_str) = item["event_date"].as_str() {
            if let Ok(event_date) = date_str.parse::<DateTime<Utc>>() {
                let now = Utc::now();
                let seconds_until = event_date.signed_duration_since(now).num_seconds();

                if seconds_until > 0 {
                    // Keep track of the match that is starting the soonest
                    closest_match_in_secs = match closest_match_in_secs {
                        Some(current_min) => Some(current_min.min(seconds_until)),
                        None => Some(seconds_until),
                    };
                }
            }
        }

        if status == "running" {
            current_status = "live";
        } else if status == "pending" || status == "ready" || status == "scheduled" {
            if current_status != "live" && current_status != "about_to_start" {
                current_status = "scheduled";
            }
        }
    }

    // Determine if any match is exactly in the "1 minute" window
    if let Some(secs) = closest_match_in_secs {
        if secs <= 60 && secs > 0 && current_status != "live" {
            current_status = "about_to_start";
        }
    }

    // 🏴‍☠️ SMART POLLING LOGIC
    let mut next_sleep_secs = 600; // Default sleep is 10 minutes

    match closest_match_in_secs {
        Some(secs_to_match) if secs_to_match > 43200 => {
            // Next game is more than 12 hours away: sleep for 1 hour
            next_sleep_secs = 3600;
        }
        Some(secs_to_match) if secs_to_match > 65 && secs_to_match < 600 => {
            // Wake up exactly ~62 seconds before the match starts to catch the 1-minute window safely
            next_sleep_secs = secs_to_match - 62;
        }
        Some(secs_to_match) if secs_to_match <= 65 => {
            // Very close to start: sleep for 30 seconds
            next_sleep_secs = 30;
        }
        _ => {}
    }

    if current_status == "live" {
        // Once live, checking every 10 minutes is enough until it ends
        next_sleep_secs = 600;
    }

    let last_status = load_state(STATE_FILE);

    if current_status != last_status && current_status != "unknown" {
        let message: Option<String> = match current_status {
            "scheduled" => Some(format!("🏴‍☠️ **Ahoy Pirates!** A new Super League match has been scheduled!\nCheck the broadcast room here: {}", WEB_URL)),
            "about_to_start" => Some(format!("⏰ **ALERTE IMMINENTE !** Le coup d'envoi approche, le match commence dans une minute ! Soyez prêts : {}", WEB_URL)),
            "live" => Some(format!("⚔️ **THE GAME IS LIVE!** ⚔️\nThe Super League broadcast has officially started! Grab your rum and tune in now!\nLink: {}", WEB_URL)),
            _ => None,
        };

        if let Some(msg) = message {
            send_discord_alert(client, &msg)?;
            info!("Alert sent to Discord: {}", current_status);
        }

        save_state(STATE_FILE, current_status)?;
    }

    Ok(apply_jitter(next_sleep_secs as u64))
}
