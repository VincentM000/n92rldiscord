use chrono::Datelike;
use pirate_utils::ladder_tracker::{run_tracker_loop, LadderTracker};
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH}; // 🏴‍☠️ Imported for Cache-Busting

const STATE_FILE: &str = "ladder_state.txt";

fn get_current_season() -> i32 {
    chrono::Utc::now().year()
}

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
        _ => "🏉",
    }
}

fn find_all_fixtures(value: &Value, fixtures: &mut Vec<Value>) {
    if let Some(obj) = value.as_object() {
        if obj.contains_key("matchState")
            && obj.contains_key("homeTeam")
            && obj.contains_key("awayTeam")
        {
            fixtures.push(value.clone());
        }
        for (_, v) in obj {
            find_all_fixtures(v, fixtures);
        }
    } else if let Some(arr) = value.as_array() {
        for v in arr {
            find_all_fixtures(v, fixtures);
        }
    }
}

struct NrlTracker;

impl LadderTracker for NrlTracker {
    fn name(&self) -> &str {
        "NRL Tracker"
    }

    fn state_filename(&self) -> &str {
        STATE_FILE
    }

    fn fetch_latest_match_result(
        &self,
        client: &Client,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let season = get_current_season();
        let draw_url = format!(
            "https://www.nrl.com/draw/?competition=111&season={}",
            season
        );
        let stealth_url = format!("{}&_t={}", draw_url, timestamp);

        let html_content = client.get(&stealth_url).send()?.text()?;
        let document = Html::parse_document(&html_content);

        let selector = Selector::parse("[q-data]").unwrap();
        let mut all_fixtures = Vec::new();

        for element in document.select(&selector) {
            if let Some(json_str) = element.value().attr("q-data") {
                if let Ok(json_data) = serde_json::from_str::<Value>(json_str) {
                    find_all_fixtures(&json_data, &mut all_fixtures);
                }
            }
        }

        let mut last_fulltime_message = None;

        for fixture in all_fixtures {
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

                last_fulltime_message = Some(format!(
                    "🏁 **RÉSULTAT FINAL :**\n{} **{}** {} - {} **{}** {}",
                    emoji_home, home_team, home_score, away_score, away_team, emoji_away
                ));
            }
        }

        Ok(last_fulltime_message)
    }

    fn fetch_ladder(&self, client: &Client) -> Result<String, Box<dyn std::error::Error>> {
        let season = get_current_season();
        let ladder_url = format!(
            "https://www.nrl.com/ladder/?competition=111&season={}",
            season
        );
        let html_content = client.get(&ladder_url).send()?.text()?;
        let document = Html::parse_document(&html_content);

        let selector = Selector::parse("#vue-ladder").unwrap();
        let element = document
            .select(&selector)
            .next()
            .ok_or("Could not find #vue-ladder div in HTML")?;

        let json_str = element
            .value()
            .attr("q-data")
            .ok_or("Could not find q-data attribute")?;

        let json_data: Value = serde_json::from_str(json_str)?;

        let positions = json_data["positions"]
            .as_array()
            .ok_or("Could not find positions array in JSON")?;

        let mut current_ladder = String::from("🏆 **CLASSEMENT NRL** 🏆\n\n");

        for (index, team_data) in positions.iter().enumerate() {
            let team_name = team_data["teamNickname"].as_str().unwrap_or("Unknown");

            let theme_key = team_data["theme"]["key"].as_str().unwrap_or("unknown");
            let emoji_str = get_team_emoji(theme_key);

            let points = team_data["stats"]["points"].as_u64().unwrap_or(0);
            let wins = team_data["stats"]["wins"].as_u64().unwrap_or(0);
            let drawn = team_data["stats"]["drawn"].as_u64().unwrap_or(0);
            let lost = team_data["stats"]["lost"].as_u64().unwrap_or(0);

            let next_opponent = if team_data["next"]["isBye"].as_bool().unwrap_or(false) {
                "Repos".to_string()
            } else {
                team_data["next"]["nickname"]
                    .as_str()
                    .unwrap_or("Unknown")
                    .to_string()
            };

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

        Ok(current_ladder)
    }
}

fn main() {
    env_logger::init();
    run_tracker_loop(NrlTracker);
}
