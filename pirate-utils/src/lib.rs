use dotenvy::dotenv;
use reqwest::blocking::Client;
use serde_json::json;
use std::env;

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
