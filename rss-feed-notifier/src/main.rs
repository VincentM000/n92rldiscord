use chrono::Timelike;
use log::{info, warn};
use pirate_utils::{apply_jitter, load_state, save_state, send_discord_alert};
use reqwest::blocking::Client;
use rss::Channel;
use std::thread;

const FEED_URL: &str = "https://treizemondial.fr/feed/";
const STATE_FILE: &str = "last_article_url.txt";

fn get_sleep_duration() -> std::time::Duration {
    let hour = chrono::Utc::now().hour();
    // Daytime (06:00 - 22:00 UTC): check every 5 minutes (300s)
    // Overnight (22:00 - 06:00 UTC): check every 30 minutes (1800s)
    let base_secs = if (6..22).contains(&hour) { 300 } else { 1800 };
    apply_jitter(base_secs)
}

fn main() {
    env_logger::init();

    let client = Client::builder()
        .user_agent("NanterrePirates-RSS/1.0")
        .build()
        .expect("Failed to build HTTP client");

    info!("🏴‍☠️ RSS Feed Notifier started with smart day/night polling and jitter...");

    loop {
        if let Err(e) = check_rss_feed(&client) {
            warn!("⚠️ Error checking RSS feed: {}", e);
        }

        let sleep_duration = get_sleep_duration();
        thread::sleep(sleep_duration);
    }
}

fn check_rss_feed(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Fetch the raw XML from the feed
    let xml_content = client.get(FEED_URL).send()?.bytes()?;

    // 2. Parse the XML into an RSS Channel object
    let channel = Channel::read_from(&xml_content[..])?;

    // 3. Extract the very first (newest) article in the feed
    let latest_item = match channel.items().first() {
        Some(item) => item,
        None => return Ok(()), // If the feed is completely empty, exit silently
    };

    // We use the article's link as our unique identifier
    let latest_link = match latest_item.link() {
        Some(link) => link,
        None => return Ok(()), // If there's no link, we can't really share it
    };

    // Grab the title safely (fallback to a generic string if missing)
    let latest_title = latest_item.title().unwrap_or("Nouvel article");

    // 4. Read the last known article link from our state file
    let last_link = load_state(STATE_FILE);

    // 5. Compare and Alert!
    if latest_link != last_link && !last_link.is_empty() {
        // We ensure last_link isn't empty so we don't spam the channel on the very first boot of the program

        let message = format!(
            "🗞️ **Nouvel article sur Treize Mondial !** 🏴‍☠️\n\n**{}**\n{}",
            latest_title, latest_link
        );

        // Call the secure function from pirate-utils
        send_discord_alert(client, &message)?;
        info!("Alert sent to Discord: {}", latest_title);
    }

    // Always save the latest link so we establish our baseline (even on the first run)
    if latest_link != last_link {
        save_state(STATE_FILE, latest_link)?;
    }

    Ok(())
}
