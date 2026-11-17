use dotenvy::dotenv;
use reqwest::blocking::Client;
use serde_json::json;
use std::env;

/// Sends a message to a specific Discord Webhook URL as "Captain Hook".
pub fn send_discord_webhook(
    client: &Client,
    webhook_url: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let payload = json!({
        "username": "Captain Hook",
        "content": message
    });

    client
        .post(webhook_url)
        .json(&payload)
        .send()?
        .error_for_status()?;
    Ok(())
}

/// Sends a message to the configured Discord Webhook as "Captain Hook".
pub fn send_discord_alert(
    client: &Client,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let webhook_url = env::var("DISCORD_WEBHOOK_URL")
        .expect("DISCORD_WEBHOOK_URL environment variable is not set");

    send_discord_webhook(client, &webhook_url, message)
}
