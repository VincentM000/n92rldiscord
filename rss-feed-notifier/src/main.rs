use chrono::Timelike;
use log::{info, warn};
use pirate_utils::{apply_jitter, build_http_client, load_state, save_state, send_discord_alert};
use reqwest::blocking::Client;
use rss::Channel;
use std::thread;

const FEED_URL: &str = "https://treizemondial.fr/feed/";
const STATE_FILE: &str = "last_article_url.txt";

fn base_sleep_secs_for_hour(hour: u32) -> u64 {
    if (6..22).contains(&hour) {
        300
    } else {
        1800
    }
}

fn get_sleep_duration() -> std::time::Duration {
    let hour = chrono::Utc::now().hour();
    apply_jitter(base_sleep_secs_for_hour(hour))
}

fn main() {
    env_logger::init();
    let client = build_http_client();

    info!("RSS Feed Notifier started.");

    loop {
        if let Err(e) = check_rss_feed(&client) {
            warn!("Error checking RSS feed: {}", e);
        }

        let sleep_duration = get_sleep_duration();
        thread::sleep(sleep_duration);
    }
}

fn check_rss_feed(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    let xml_content = client.get(FEED_URL).send()?.bytes()?;
    let channel = Channel::read_from(&xml_content[..])?;

    let latest_item = match channel.items().first() {
        Some(item) => item,
        None => return Ok(()),
    };

    let latest_link = match latest_item.link() {
        Some(link) => link,
        None => return Ok(()),
    };

    let latest_title = latest_item.title().unwrap_or("Nouvel article");
    let last_link = load_state(STATE_FILE);

    if latest_link != last_link && !last_link.is_empty() {
        let message = format!(
            "🗞️ **Nouvel article sur Treize Mondial !** 🏴‍☠️\n\n**{}**\n{}",
            latest_title, latest_link
        );

        send_discord_alert(client, &message)?;
        info!("Alert sent to Discord: {}", latest_title);
    }

    if latest_link != last_link {
        save_state(STATE_FILE, latest_link)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_sleep_secs_for_hour() {
        assert_eq!(base_sleep_secs_for_hour(12), 300);
        assert_eq!(base_sleep_secs_for_hour(3), 1800);
    }
}
