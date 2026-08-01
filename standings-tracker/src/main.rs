use pirate_utils::send_discord_alert;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH}; // 🏴‍☠️ Imported for Cache-Busting

const LADDER_URL: &str = "https://www.nrl.com/ladder/?competition=111&season=2026";
const HOME_URL: &str = "https://www.nrl.com/";
const STATE_FILE: &str = "ladder_state.txt";

fn main() {
    println!(
        "🏴‍☠️ Hoisting the colors! The Standings Tracker will check the horizon every 10 minutes..."
    );

    let client = Client::builder()
        .user_agent("NanterrePirates-Scraper/1.0")
        .build()
        .expect("Failed to build HTTP client");

    // The continuous 10-minute loop
    loop {
        if let Err(e) = check_ladder(&client) {
            eprintln!("⚠️ Arrr! Hit a reef: {}", e);
        }

        // Sleep for 600 seconds (10 minutes) before the next check
        thread::sleep(Duration::from_secs(600));
    }
}

// 🏴‍☠️ Dictionary mapping theme keys to your custom Discord emojis
fn get_team_emoji(theme_key: &str) -> &str {
    match theme_key {
        "panthers" => "<:panthers:1531243678763323512>",
        "roosters" => "<:roosters:1531242666422046901>",
        "warriors" => "<:warriors:1531242707371036782>",
        "sharks" => "<:sharks:1531242648441065532>",
        "dolphins" => "<:dolphins:1531242640199254056>",
        "rabbitohs" => "<:rabbitohs:1531242606477181070>",
        "knights" => "<:knights:1531242659899772988>",
        "cowboys" => "<:cowboys:1531243674002919515>",
        "bulldogs" => "<:bulldogs:1531243677211558132>",
        "sea-eagles" => "<:seaeagles:1531242700496699483>",
        "raiders" => "<:raiders:1531242654309023764>",
        "storm" => "<:storm:1531243670123319476>",
        "titans" => "<:titans:1531243672840966185>",
        "broncos" => "<:broncos:1531243675525316658>",
        "eels" => "<:eels:1531242628665049198>",
        "wests-tigers" => "<:weststigers:1531243671729471538>",
        "dragons" => "<:dragons:1531242582405943337>",
        _ => "🏉", // Default emoji if key is not found
    }
}

// Function to fetch the latest match result from the NRL homepage, bypassing the CDN cache
fn get_latest_match_result(client: &Client) -> Result<Option<String>, Box<dyn std::error::Error>> {
    // 1. Generate a unique timestamp for Cache-Busting
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();

    // 2. Create the stealth URL (e.g., https://www.nrl.com/?_t=123456789)
    let stealth_url = format!("{}?_t={}", HOME_URL, timestamp);

    // 3. Fetch the FRESH raw HTML from the homepage using the stealth URL
    let html_content = client.get(&stealth_url).send()?.text()?;
    let document = Html::parse_document(&html_content);

    // 4. Select the div containing the scoreboard data
    let selector = Selector::parse("#vue-draw-overview").unwrap();
    let element = document
        .select(&selector)
        .next()
        .ok_or("Could not find #vue-draw-overview div in HTML")?;

    // 5. Extract the JSON string from the "q-data" attribute
    let json_str = element
        .value()
        .attr("q-data")
        .ok_or("Could not find q-data attribute")?;

    // 6. Parse the JSON
    let json_data: Value = serde_json::from_str(json_str)?;

    // 7. Navigate to the fixtures array
    let competitions = json_data["competitions"]
        .as_array()
        .ok_or("Could not find competitions array in JSON")?;

    let fixtures = competitions
        .get(0)
        .and_then(|c| c["fixtures"].as_array())
        .ok_or("Could not find fixtures array in JSON")?;

    // 8. Iterate backwards to find the most recently completed match ("FullTime")
    for fixture in fixtures.iter().rev() {
        if fixture["matchState"].as_str() == Some("FullTime") {
            let home_team = fixture["homeTeam"]["nickName"]
                .as_str()
                .unwrap_or("Unknown");
            let home_score = fixture["homeTeam"]["score"].as_u64().unwrap_or(0);
            let home_theme = fixture["homeTeam"]["theme"]["key"]
                .as_str()
                .unwrap_or("unknown");

            let away_team = fixture["awayTeam"]["nickName"]
                .as_str()
                .unwrap_or("Unknown");
            let away_score = fixture["awayTeam"]["score"].as_u64().unwrap_or(0);
            let away_theme = fixture["awayTeam"]["theme"]["key"]
                .as_str()
                .unwrap_or("unknown");

            let emoji_home = get_team_emoji(home_theme);
            let emoji_away = get_team_emoji(away_theme);

            // This message goes to Discord, so it's in French
            let message = format!(
                "🏁 **RÉSULTAT FINAL :**\n{} **{}** {} - {} **{}** {}",
                emoji_home, home_team, home_score, away_score, away_team, emoji_away
            );

            return Ok(Some(message));
        }
    }

    Ok(None)
}

fn check_ladder(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Fetch the raw HTML
    let html_content = client.get(LADDER_URL).send()?.text()?;
    let document = Html::parse_document(&html_content);

    // 2. Select the div that holds the treasure (id="vue-ladder")
    let selector = Selector::parse("#vue-ladder").unwrap();
    let element = document
        .select(&selector)
        .next()
        .ok_or("Could not find #vue-ladder div in HTML")?;

    // 3. Extract the JSON string from the "q-data" attribute
    let json_str = element
        .value()
        .attr("q-data")
        .ok_or("Could not find q-data attribute")?;

    // 4. Parse the JSON
    let json_data: Value = serde_json::from_str(json_str)?;

    // 5. Navigate to the ladder positions
    let positions = json_data["positions"]
        .as_array()
        .ok_or("Could not find positions array in JSON")?;

    // 6. Build the new ladder state string for all 17 teams
    // This message goes to Discord, so it's in French
    let mut current_ladder = String::from("🏆 **CLASSEMENT NRL** 🏆\n\n");

    for (index, team_data) in positions.iter().enumerate() {
        let team_name = team_data["teamNickname"].as_str().unwrap_or("Unknown");

        // Fetch emoji using the theme key
        let theme_key = team_data["theme"]["key"].as_str().unwrap_or("unknown");
        let emoji_str = get_team_emoji(theme_key);

        // Extracting all requested stats
        let points = team_data["stats"]["points"].as_u64().unwrap_or(0);
        let wins = team_data["stats"]["wins"].as_u64().unwrap_or(0);
        let drawn = team_data["stats"]["drawn"].as_u64().unwrap_or(0);
        let lost = team_data["stats"]["lost"].as_u64().unwrap_or(0);

        // Logic to determine the next opponent (catching Byes). In French for Discord.
        let next_opponent = if team_data["next"]["isBye"].as_bool().unwrap_or(false) {
            "Repos".to_string()
        } else {
            team_data["next"]["nickname"]
                .as_str()
                .unwrap_or("Unknown")
                .to_string()
        };

        // Compact format to respect Discord's 2000 character limit (V = Victoires, N = Nuls, D = Défaites)
        current_ladder.push_str(&format!(
            "**{}.** {} **{}** • **{} pts** *(V:{} N:{} D:{})* 🔜 {}\n",
            index + 1,
            emoji_str,
            team_name,
            points,
            wins,
            drawn,
            lost,
            next_opponent
        ));
    }

    // 7. Read the last known state from our state file
    let last_ladder = if Path::new(STATE_FILE).exists() {
        fs::read_to_string(STATE_FILE).unwrap_or_default()
    } else {
        String::new()
    };

    // 8. Compare and Alert if the ladder has changed
    if current_ladder != last_ladder {
        // Ensure this is not the very first run (last_ladder empty) to avoid sending an old score
        if !last_ladder.is_empty() {
            println!("🏴‍☠️ The ladder has shifted! Fetching the latest match result...");
            match get_latest_match_result(client) {
                Ok(Some(result_message)) => {
                    // Send the final score
                    if let Err(e) = send_discord_alert(client, &result_message) {
                        eprintln!("⚠️ Arrr! Failed to send match result: {}", e);
                    }

                    // Delaying for 2 seconds to ensure Discord processes the score before the ladder
                    println!(
                        "⏳ Pausing for 2 seconds to ensure correct message order on Discord..."
                    );
                    thread::sleep(Duration::from_secs(2));
                }
                Ok(None) => println!("⚠️ No recent match result found on the homepage."),
                Err(e) => eprintln!("⚠️ Error fetching match result: {}", e),
            }
        }

        println!("🏴‍☠️ Firing ladder message to Discord...");
        send_discord_alert(client, &current_ladder)?;
    }

    // Always update the state file (this also establishes the baseline on the very first run)
    if current_ladder != last_ladder {
        fs::write(STATE_FILE, &current_ladder)?;
    }

    Ok(())
}
