use dotenvy::dotenv;
use reqwest::blocking::Client;
use serde_json::json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Structured Discord webhook notifier instance.
pub struct DiscordNotifier {
    client: Client,
    webhook_url: String,
}

impl DiscordNotifier {
    /// Creates a new notifier loading DISCORD_WEBHOOK_URL from environment / .env.
    pub fn from_env(client: Client) -> Result<Self, String> {
        dotenv().ok();
        let webhook_url = env::var("DISCORD_WEBHOOK_URL").map_err(|_| {
            "CRITICAL ERROR: DISCORD_WEBHOOK_URL is not set in environment or .env file!"
                .to_string()
        })?;
        Ok(Self {
            client,
            webhook_url,
        })
    }

    /// Sends a text message alert to the configured Discord webhook.
    pub fn send_alert(&self, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        let payload = json!({
            "username": "Captain Hook",
            "content": message
        });
        self.client.post(&self.webhook_url).json(&payload).send()?;
        Ok(())
    }
}

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

/// Resolves file path using STATE_DIR env variable if present, defaulting to current working directory.
fn resolve_state_path(filename: &str) -> PathBuf {
    if let Ok(dir) = env::var("STATE_DIR") {
        Path::new(&dir).join(filename)
    } else {
        Path::new(filename).to_path_buf()
    }
}

/// Reads state from a local file, returning an empty string if the file doesn't exist.
pub fn load_state(filename: &str) -> String {
    let path = resolve_state_path(filename);
    if path.exists() {
        fs::read_to_string(path).unwrap_or_default()
    } else {
        String::new()
    }
}

/// Saves state content to a local file, creating parent directories if needed.
pub fn save_state(filename: &str, content: &str) -> std::io::Result<()> {
    let path = resolve_state_path(filename);
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }
    }
    fs::write(path, content)
}

/// Sends a message to the Discord channel using the Webhook URL stored in the .env file.
pub fn send_discord_alert(
    client: &Client,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let webhook_url = env::var("DISCORD_WEBHOOK_URL")
        .expect("CRITICAL ERROR: DISCORD_WEBHOOK_URL is not set in the .env file!");

    let payload = json!({
        "username": "Captain Hook",
        "content": message
    });

    client.post(&webhook_url).json(&payload).send()?;
    Ok(())
}
