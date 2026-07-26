use pirate_utils::send_discord_alert;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::fs;
use std::path::Path; // 🏴‍☠️ Import the shared Captain Hook function!

const TARGET_URL: &str = "https://live.rugbyrama.fr/fr/1-directs/?ch=4";
const STATE_FILE: &str = "sl_state.txt";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Set up the client with a Pirate User-Agent to avoid getting blocked
    let client = Client::builder()
        .user_agent("NanterrePirates-Scraper/1.0")
        .build()?;

    // 2. Fetch the webpage
    let response_text = client.get(TARGET_URL).send()?.text()?;

    // 3. Parse the HTML
    let document = Html::parse_document(&response_text);

    // Note: We use a broad selector here for the draft.
    let body_selector = Selector::parse("body").unwrap();

    let mut super_league_text = String::new();

    for element in document.select(&body_selector) {
        let text = element.text().collect::<Vec<_>>().join(" ");
        if text.contains("Super League") {
            super_league_text = text;
            break;
        }
    }

    // If there's no mention of Super League, we just exit silently.
    if super_league_text.is_empty() {
        return Ok(());
    }

    // 4. Determine the current status
    let mut current_status = "unknown";

    if super_league_text.contains("Le coup d'envoi approche") {
        current_status = "scheduled";
    } else if super_league_text.contains("en direct") || super_league_text.contains("Live") {
        current_status = "live";
    } else if super_league_text.contains("Super League") {
        // Fallback: It's in the list, but doesn't have the specific tags yet
        current_status = "listed";
    }

    // 5. Read the last known status from our state file
    let last_status = if Path::new(STATE_FILE).exists() {
        fs::read_to_string(STATE_FILE).unwrap_or_default()
    } else {
        String::new()
    };

    // 6. Compare and Alert!
    if current_status != last_status && current_status != "unknown" {
        // We now use Option<String> instead of just String
        let message: Option<String> = match current_status {
        "listed" | "scheduled" => {
            Some(format!("🏴‍☠️ **Ahoy Pirates!** A new Super League match is on the schedule! Prepare your gear.\nCheck it here: {}", TARGET_URL))
        },
        "live" => {
            Some(format!("⚔️ **THE GAME IS LIVE!** ⚔️\nThe Super League broadcast has officially started! Grab your rum and tune in now!\nLink: {}", TARGET_URL))
        },
        _ => None, // <-- Replaces String::new()
    };

        // "if let" cleanly checks if there is 'Some' message inside the Option
        if let Some(msg) = message {
            // Call the secure function from pirate-utils
            send_discord_alert(&client, &msg)?;
        }

        // Save the new status so we don't spam the channel on the next cron run
        fs::write(STATE_FILE, current_status)?;
    }

    Ok(())
}
