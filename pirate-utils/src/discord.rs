use dotenvy::dotenv;
use reqwest::blocking::Client;
use serde_json::json;
use std::env;

/// Sends a message to a specific Discord Webhook URL with a custom username.
pub fn send_discord_webhook_as(
    client: &Client,
    webhook_url: &str,
    username: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let payload = json!({
        "username": username,
        "content": message
    });

    client
        .post(webhook_url)
        .json(&payload)
        .send()?
        .error_for_status()?;
    Ok(())
}

/// Sends a message to a specific Discord Webhook URL as "Captain Hook".
pub fn send_discord_webhook(
    client: &Client,
    webhook_url: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    send_discord_webhook_as(client, webhook_url, "Captain Hook", message)
}

/// Sends a message to the configured Captain Hook Discord Webhook.
pub fn send_discord_alert(
    client: &Client,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let webhook_url = env::var("DISCORD_CAPTAIN_HOOK_WEBHOOK_URL")
        .or_else(|_| env::var("DISCORD_WEBHOOK_URL"))
        .expect("DISCORD_CAPTAIN_HOOK_WEBHOOK_URL environment variable is not set");

    send_discord_webhook(client, &webhook_url, message)
}
