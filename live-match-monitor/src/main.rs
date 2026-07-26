use pirate_utils::send_discord_alert;
use reqwest::blocking::Client;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration; // 🏴‍☠️ Import the shared Captain Hook function!

// The raw JSON API endpoint using Rugbyrama's exact internal filters
const API_URL: &str = "https://api-live.rugbyrama.fr/api/videos?channel=4&limit=5&order=asc&order_by=event_date&status[]=running&status[]=pending&status[]=ready&status[]=scheduled&type[]=live";
// The user-friendly link we will send in the Discord message
const WEB_URL: &str = "https://live.rugbyrama.fr/fr/1-directs/?ch=4";
const STATE_FILE: &str = "sl_state.txt";

fn main() {
    // 1. Set up the client with a Pirate User-Agent to avoid getting blocked
    let client = Client::builder()
        .user_agent("NanterrePirates-Scraper/1.0")
        .build()
        .expect("Failed to build HTTP client");

    println!("🏴‍☠️ Live Match Monitor started. Checking the API every 5 minutes...");

    // The continuous 5-minute loop
    loop {
        if let Err(e) = check_rugbyrama_api(&client) {
            eprintln!("⚠️ Error during API check: {}", e);
        }

        // Sleep for 300 seconds (5 minutes) before the next check
        thread::sleep(Duration::from_secs(300));
    }
}

fn check_rugbyrama_api(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Fetch and automatically parse the JSON response
    let json_response: Value = client.get(API_URL).send()?.json()?;

    // 2. Safely extract the "items" array
    let items = match json_response["items"].as_array() {
        Some(arr) => arr,
        None => return Ok(()), // If the API changes or returns no items, exit silently
    };

    let mut current_status = "unknown";

    // 3. Loop through the matches to find what's happening
    for item in items {
        let status = item["status"].as_str().unwrap_or("unknown");

        // The game is currently live
        if status == "running" {
            current_status = "live";
            break; // A live game is the highest priority, stop looking!
        }
        // The game is upcoming ("Le coup d'envoi approche")
        else if status == "pending" || status == "ready" || status == "scheduled" {
            current_status = "scheduled";
            // We don't break here, because there might be another match in the list that is currently "running"
        }
    }

    // 4. Read the last known status from our state file
    let last_status = if Path::new(STATE_FILE).exists() {
        fs::read_to_string(STATE_FILE).unwrap_or_default()
    } else {
        String::new()
    };

    // 5. Compare and Alert!
    if current_status != last_status && current_status != "unknown" {
        // We use Option<String> instead of an empty String for clean logic
        let message: Option<String> = match current_status {
            "scheduled" => {
                Some(format!("🏴‍☠️ **Ahoy Pirates!** A new Super League match has been scheduled!\nCheck the broadcast room here: {}", WEB_URL))
            },
            "live" => {
                Some(format!("⚔️ **THE GAME IS LIVE!** ⚔️\nThe Super League broadcast has officially started! Grab your rum and tune in now!\nLink: {}", WEB_URL))
            },
            _ => None,
        };

        // "if let" cleanly checks if there is 'Some' message inside the Option
        if let Some(msg) = message {
            // Call the secure function from pirate-utils
            send_discord_alert(client, &msg)?;
            println!("Alert sent to Discord: {}", current_status);
        }

        // Save the new status so we don't spam the channel on the next 5-minute loop
        fs::write(STATE_FILE, current_status)?;
    }

    Ok(())
}
