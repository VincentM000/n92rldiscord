use dotenvy::dotenv;
use reqwest::blocking::Client;
use serde_json::json;
use std::env;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Applies pseudo-random variation (+/- 20s) to a base duration to prevent bot cadence detection.
pub fn apply_jitter(base_secs: u64) -> Duration {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let jitter = (nanos % 41) as i64 - 20;
    let total = (base_secs as i64 + jitter).max(10) as u64;
    Duration::from_secs(total)
}

/// Reads state from a local file, returning an empty string if the file doesn't exist.
pub fn load_state(filepath: &str) -> String {
    if Path::new(filepath).exists() {
        fs::read_to_string(filepath).unwrap_or_default()
    } else {
        String::new()
    }
}

/// Saves state content to a local file.
pub fn save_state(filepath: &str, content: &str) -> std::io::Result<()> {
    fs::write(filepath, content)
}

/// Sends a message to the Discord channel using the Webhook URL stored in the .env file.
pub fn send_discord_alert(
    client: &Client,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load the .env file (ignores errors if it's not found)
    dotenv().ok();

    // 2. Safely extract the secret URL
    let webhook_url = env::var("DISCORD_WEBHOOK_URL")
        .expect("CRITICAL ERROR: DISCORD_WEBHOOK_URL is not set in the .env file!");

    // 3. Build and send the payload
    let payload = json!({
        "username": "Captain Hook",
        "content": message
    });

    // 4. Fire the webhook
    client.post(&webhook_url).json(&payload).send()?;

    Ok(())
}
