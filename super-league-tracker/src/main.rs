use pirate_utils::send_discord_alert;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

const LADDER_URL: &str = "https://www.skysports.com/rugby-league/competitions/betfred-super-league/table";
const STATE_FILE: &str = "sl_ladder_state.txt";

// 🏴‍☠️ Standard emoji dictionary (no Discord configuration required)
fn get_sl_team_emoji(team_name: &str) -> &str {
    match team_name {
        "Wigan Warriors" => "🍒",
        "Hull K R" => "🔴",
        "Warrington Wolves" => "🐺",
        "Salford Red Devils" => "😈",
        "Leigh Leopards" => "🐆",
        "St Helens" => "👼",
        "Catalans Dragons" => "🐉",
        "Leeds Rhinos" => "🦏",
        "Castleford Tigers" => "🐯",
        "Huddersfield Giants" => "🔔",
        "Hull FC" => "⚫",
        "Wakefield Trinity" => "⚜️",
        "Toulouse Olympique" => "✈️",
        "York Knights" => "⚔️",
        "Bradford Bulls" => "🐂",
        "London Broncos" => "🐎",
        _ => "🏉", // Default rugby ball emoji for unknown teams
    }
}

fn main() {
    println!("🏴‍☠️ Hoisting the colors! The Super League Tracker will check the horizon every 10 minutes...");

    let client = Client::builder()
        .user_agent("NanterrePirates-Scraper/1.0")
        .build()
        .expect("Failed to build HTTP client");

    // The continuous 10-minute loop
    loop {
        if let Err(e) = check_super_league_ladder(&client) {
            eprintln!("⚠️ Arrr! Hit a reef: {}", e);
        }

        // Sleep for 600 seconds (10 minutes) before the next check
        thread::sleep(Duration::from_secs(600));
    }
}

fn check_super_league_ladder(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Fetch the raw HTML
    let response = client.get(LADDER_URL).send()?.text()?;
    let document = Html::parse_document(&response);

    // 2. Select the rows and cells
    let row_selector = Selector::parse("tbody tr").unwrap();
    let team_link_selector = Selector::parse("a.sdc-site-table__link").unwrap();
    let cell_selector = Selector::parse("td.sdc-site-table__cell").unwrap();

    // 3. Build the new ladder state string
    let mut current_ladder = String::from("🏆 **SUPER LEAGUE LADDER** 🏆\n\n");
    let mut index = 1;

    for row in document.select(&row_selector) {
        let team_name = match row.select(&team_link_selector).next() {
            Some(link) => link.value().attr("aria-label").unwrap_or("Unknown").to_string(),
            None => continue,
        };

        let cells: Vec<String> = row
            .select(&cell_selector)
            .map(|cell| cell.text().collect::<String>().trim().to_string())
            .filter(|text| !text.is_empty())
            .collect();

        // Extracting stats based on HTML structure
        if cells.len() >= 8 {
            let wins = cells.get(2).unwrap_or(&"0".to_string()).clone();
            let drawn = cells.get(3).unwrap_or(&"0".to_string()).clone();
            let lost = cells.get(4).unwrap_or(&"0".to_string()).clone();
            let points = cells.last().unwrap_or(&"0".to_string()).clone(); // Last column is always Pts

            let emoji_str = get_sl_team_emoji(&team_name);

            // 4. Compact format for Discord
            current_ladder.push_str(&format!(
                "**{}.** {} **{}** • **{} pts** *(W:{} D:{} L:{})*\n",
                index, emoji_str, team_name, points, wins, drawn, lost
            ));
            
            index += 1;
        }
    }

    // 5. Read the last known state from our state file
    let last_ladder = if Path::new(STATE_FILE).exists() {
        fs::read_to_string(STATE_FILE).unwrap_or_default()
    } else {
        String::new()
    };

    // 6. Compare and Alert if the ladder has changed
    if current_ladder != last_ladder && !current_ladder.is_empty() {
        println!("🏴‍☠️ The Super League ladder has shifted! Firing message to Discord...");
        send_discord_alert(client, &current_ladder)?;
    }

    // 7. Always update the state file (this also establishes the baseline on the very first run)
    if current_ladder != last_ladder {
        fs::write(STATE_FILE, &current_ladder)?;
    }

    Ok(())
}
