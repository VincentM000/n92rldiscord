use pirate_utils::{load_state, save_state, send_discord_alert};
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::thread;
use std::time::Duration;

const LADDER_URL: &str =
    "https://www.skysports.com/rugby-league/competitions/betfred-super-league/table";
const RESULTS_URL: &str =
    "https://www.skysports.com/rugby-league/competitions/super-league/results";
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

// Function to fetch the latest match result on Sky Sports
fn get_latest_match_result(client: &Client) -> Result<Option<String>, Box<dyn std::error::Error>> {
    // 1. Fetch the HTML code from the results page
    let response = client.get(RESULTS_URL).send()?.text()?;
    let document = Html::parse_document(&response);

    // 2. Prepare our grappling hooks (CSS Selectors)
    let match_selector = Selector::parse(".fixres__item .matches__link").unwrap();
    let team1_selector = Selector::parse(".matches__participant--side1").unwrap();
    let team2_selector = Selector::parse(".matches__participant--side2").unwrap();
    let score_selector = Selector::parse(".matches__teamscores-side").unwrap();

    // 3. We only take the VERY FIRST match in the list (the most recent)
    if let Some(first_match) = document.select(&match_selector).next() {
        // Extraction of team names
        let team1 = first_match
            .select(&team1_selector)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        let team2 = first_match
            .select(&team2_selector)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        // Extraction of scores
        let scores: Vec<String> = first_match
            .select(&score_selector)
            .map(|e| e.text().collect::<String>().trim().to_string())
            .collect();

        // 4. If we successfully retrieved both scores, we assemble the message
        if scores.len() >= 2 {
            let score1 = &scores[0];
            let score2 = &scores[1];

            // Formatting the Discord message with emojis
            let emoji1 = get_sl_team_emoji(&team1);
            let emoji2 = get_sl_team_emoji(&team2);

            // This message goes to Discord, so it's in French
            let message = format!(
                "🏁 **RÉSULTAT FINAL :**\n{} **{}** {} - {} **{}** {}",
                emoji1, team1, score1, score2, team2, emoji2
            );
            return Ok(Some(message));
        }
    }

    Ok(None)
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
    // This message goes to Discord, so it's in French
    let mut current_ladder = String::from("🏆 **CLASSEMENT SUPER LEAGUE** 🏆\n\n");
    let mut index = 1;

    for row in document.select(&row_selector) {
        let team_name = match row.select(&team_link_selector).next() {
            Some(link) => link
                .value()
                .attr("aria-label")
                .unwrap_or("Unknown")
                .to_string(),
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

            // 4. Compact format for Discord (V = Victoires, N = Nuls, D = Défaites)
            current_ladder.push_str(&format!(
                "**{}.** {} **{}** • **{} pts** *(V:{} N:{} D:{})*\n",
                index, emoji_str, team_name, points, wins, drawn, lost
            ));

            index += 1;
        }
    }

    // 5. Read the last known state from our state file
    let last_ladder = load_state(STATE_FILE);

    // 6. Compare and Alert if the ladder has changed
    if current_ladder != last_ladder && !current_ladder.is_empty() {
        // Ensure this is not the very first run (last_ladder empty) to avoid sending an old score
        if !last_ladder.is_empty() {
            println!("🏴‍☠️ The Super League ladder has shifted! Fetching the latest match result...");
            match get_latest_match_result(client) {
                Ok(Some(result_message)) => {
                    // We send the final score
                    if let Err(e) = send_discord_alert(client, &result_message) {
                        eprintln!("⚠️ Arrr! Failed to send match result: {}", e);
                    }
                }
                Ok(None) => println!("⚠️ No recent match result found on the page."),
                Err(e) => eprintln!("⚠️ Error fetching match result: {}", e),
            }
        }

        println!("🏴‍☠️ Firing ladder message to Discord...");
        send_discord_alert(client, &current_ladder)?;
    }

    // 7. Always update the state file (this also establishes the baseline on the very first run)
    if current_ladder != last_ladder {
        save_state(STATE_FILE, &current_ladder)?;
    }

    Ok(())
}
