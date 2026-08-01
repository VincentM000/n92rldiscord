use dotenvy::dotenv;
use reqwest::blocking::Client;
use serde_json::json;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn apply_jitter(base_secs: u64) -> Duration {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let jitter = (nanos % 41) as i64 - 20;
    let total = (base_secs as i64 + jitter).max(10) as u64;
    Duration::from_secs(total)
}

fn resolve_state_path(filename: &str) -> PathBuf {
    if let Ok(dir) = env::var("STATE_DIR") {
        Path::new(&dir).join(filename)
    } else {
        Path::new(filename).to_path_buf()
    }
}

pub fn load_state(filename: &str) -> String {
    let path = resolve_state_path(filename);
    if path.exists() {
        fs::read_to_string(path).unwrap_or_default()
    } else {
        String::new()
    }
}

pub fn save_state(filename: &str, content: &str) -> std::io::Result<()> {
    let path = resolve_state_path(filename);
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }
    }
    fs::write(path, content)
}

pub fn send_discord_alert(
    client: &Client,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let webhook_url = env::var("DISCORD_WEBHOOK_URL")
        .expect("DISCORD_WEBHOOK_URL environment variable is not set");

    let payload = json!({
        "username": "Captain Hook",
        "content": message
    });

    client.post(&webhook_url).json(&payload).send()?;
    Ok(())
}
