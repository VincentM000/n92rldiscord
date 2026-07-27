use pirate_utils::send_discord_alert;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

const LADDER_URL: &str = "https://www.nrl.com/ladder/?competition=111&season=2026";
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

// 🏴‍☠️ Le dictionnaire de vos émojis Discord avec vos vrais IDs
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
        _ => "🏉", // Émoji par défaut si la clé n'est pas trouvée
    }
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
    let mut current_ladder = String::from("🏆 **NRL LADDER - FULL 17 TEAMS** 🏆\n\n");

    for (index, team_data) in positions.iter().enumerate() {
        let team_name = team_data["teamNickname"].as_str().unwrap_or("Unknown");

        // Utilisation du dictionnaire pour récupérer l'émoji
        let theme_key = team_data["theme"]["key"].as_str().unwrap_or("unknown");
        let emoji_str = get_team_emoji(theme_key);

        // Extracting all requested stats
        let points = team_data["stats"]["points"].as_u64().unwrap_or(0);
        let wins = team_data["stats"]["wins"].as_u64().unwrap_or(0);
        let drawn = team_data["stats"]["drawn"].as_u64().unwrap_or(0);
        let lost = team_data["stats"]["lost"].as_u64().unwrap_or(0);

        // Logic to determine the next opponent (catching Byes)
        let next_opponent = if team_data["next"]["isBye"].as_bool().unwrap_or(false) {
            "Bye".to_string()
        } else {
            team_data["next"]["nickname"]
                .as_str()
                .unwrap_or("Unknown")
                .to_string()
        };

        // Format ultra-compact pour respecter la limite des 2000 caractères de Discord
        current_ladder.push_str(&format!(
            "**{}.** {} **{}** • **{} pts** *(W:{} D:{} L:{})* 🔜 {}\n",
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
        println!("🏴‍☠️ The ladder has shifted! Firing message to Discord...");
        send_discord_alert(client, &current_ladder)?;
    }

    // Always update the state file (this also establishes the baseline on the very first run)
    if current_ladder != last_ladder {
        fs::write(STATE_FILE, &current_ladder)?;
    }

    Ok(())
}
