use pirate_utils::send_discord_alert;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use serde_json::Value;

const LADDER_URL: &str = "https://www.nrl.com/ladder/?competition=111&season=2026";

fn main() {
    println!("🏴‍☠️ Hoisting the colors! Plundering the NRL ladder from the HTML attribute...");

    let client = Client::builder()
        .user_agent("NanterrePirates-Scraper/1.0")
        .build()
        .expect("Failed to build HTTP client");

    if let Err(e) = fetch_and_send_ladder(&client) {
        eprintln!("⚠️ Arrr! Hit a reef: {}", e);
    } else {
        println!("✅ Ladder successfully sent to Discord!");
    }
}

fn fetch_and_send_ladder(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
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
    // Based on your HTML dump, the ladder is directly in "positions"
    let positions = json_data["positions"]
        .as_array()
        .ok_or("Could not find positions array in JSON")?;

    // 6. Format the Discord message
    let mut message = String::from("🏆 **NRL LADDER - TOP 8** 🏆\n\n");

    for (index, team_data) in positions.iter().take(8).enumerate() {
        let team_name = team_data["teamNickname"].as_str().unwrap_or("Unknown");
        let points = team_data["stats"]["points"].as_u64().unwrap_or(0);
        let played = team_data["stats"]["played"].as_u64().unwrap_or(0);

        message.push_str(&format!(
            "**{}.** {} ({} pts, {} played)\n",
            index + 1,
            team_name,
            points,
            played
        ));
    }

    message.push_str("\n*May the best crew win the Premiership!* 🏴‍☠️");

    // 7. Fire the cannons!
    send_discord_alert(client, &message)?;

    Ok(())
}
