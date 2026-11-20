use chrono::Datelike;
use pirate_utils::ladder_tracker::{run_tracker_loop, LadderTracker, MATCH_DELIMITER};
use pirate_utils::{format_ladder, format_match_result, LadderRow, MatchResult};
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

const NRL_STATE_FILE: &str = "nrl_ladder_state.txt";
const NRL_MATCH_STATE_FILE: &str = "nrl_match_state.txt";

fn get_current_season() -> i32 {
    chrono::Utc::now().year()
}

fn get_team_emoji(theme_key: &str) -> &'static str {
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

fn parse_nrl_ladder_json(json_str: &str) -> Result<String, Box<dyn std::error::Error>> {
    let json_data: Value = serde_json::from_str(json_str)?;
    let positions = json_data["positions"]
        .as_array()
        .ok_or("Could not find positions array in JSON")?;

    let mut rows = Vec::with_capacity(positions.len());

    for (index, team_data) in positions.iter().enumerate() {
        let team_name = team_data["teamNickname"]
            .as_str()
            .unwrap_or("Unknown")
            .to_string();
        let theme_key = team_data["theme"]["key"].as_str().unwrap_or("unknown");
        let emoji = get_team_emoji(theme_key).to_string();

        let points = team_data["stats"]["points"].as_i64().unwrap_or(0) as i32;
        let wins = team_data["stats"]["wins"].as_u64().unwrap_or(0) as u32;
        let draws = team_data["stats"]["drawn"].as_u64().unwrap_or(0) as u32;
        let losses = team_data["stats"]["lost"].as_u64().unwrap_or(0) as u32;

        let next_opponent = if team_data["next"]["isBye"].as_bool().unwrap_or(false) {
            "Repos".to_string()
        } else {
            team_data["next"]["nickname"]
                .as_str()
                .unwrap_or("Unknown")
                .to_string()
        };

        rows.push(LadderRow {
            rank: index + 1,
            team_name,
            emoji,
            points,
            wins,
            draws,
            losses,
            suffix: Some(format!("🔜 {}", next_opponent)),
        });
    }

    Ok(format_ladder("CLASSEMENT NRL", &rows))
}

struct NrlTracker;

impl LadderTracker for NrlTracker {
    fn name(&self) -> &str {
        "NRL Tracker"
    }

    fn state_filename(&self) -> &str {
        NRL_STATE_FILE
    }

    fn match_state_filename(&self) -> &str {
        NRL_MATCH_STATE_FILE
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
        let stealth_url = format!(
            "https://www.nrl.com/draw/?competition=111&season={}&_t={}",
            season, timestamp
        );

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

        let mut completed_messages = Vec::new();

        for fixture in all_fixtures {
            if fixture["matchState"].as_str() == Some("FullTime") {
                let home_team = fixture["homeTeam"]["nickName"]
                    .as_str()
                    .unwrap_or("Unknown")
                    .to_string();
                let home_score = fixture["homeTeam"]["score"].as_i64().unwrap_or(0);
                let home_theme = fixture["homeTeam"]["theme"]["key"]
                    .as_str()
                    .unwrap_or("unknown");

                let away_team = fixture["awayTeam"]["nickName"]
                    .as_str()
                    .unwrap_or("Unknown")
                    .to_string();
                let away_score = fixture["awayTeam"]["score"].as_i64().unwrap_or(0);
                let away_theme = fixture["awayTeam"]["theme"]["key"]
                    .as_str()
                    .unwrap_or("unknown");

                let result = MatchResult {
                    header_suffix: None,
                    home_team,
                    home_emoji: get_team_emoji(home_theme).to_string(),
                    home_score,
                    away_team,
                    away_emoji: get_team_emoji(away_theme).to_string(),
                    away_score,
                    venue: None,
                };

                completed_messages.push(format_match_result(&result));
            }
        }

        if completed_messages.is_empty() {
            Ok(None)
        } else {
            Ok(Some(completed_messages.join(MATCH_DELIMITER)))
        }
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

        parse_nrl_ladder_json(json_str)
    }
}

fn main() {
    env_logger::init();
    run_tracker_loop(NrlTracker);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nrl_ladder_json() {
        let sample_json = r#"{
            "positions": [
                {
                    "teamNickname": "Storm",
                    "theme": { "key": "storm" },
                    "stats": { "points": 12, "wins": 6, "drawn": 0, "lost": 1 },
                    "next": { "isBye": false, "nickname": "Panthers" }
                }
            ]
        }"#;

        let formatted = parse_nrl_ladder_json(sample_json).expect("Should parse NRL ladder JSON");
        assert!(formatted.contains("🏆 **CLASSEMENT NRL** 🏆"));
        assert!(formatted.contains("**Storm** • **12 pts** *(V:6 N:0 D:1)* 🔜 Panthers"));
    }

    #[test]
    fn test_find_all_fixtures() {
        let nested = serde_json::json!({
            "round": {
                "matches": [
                    {
                        "matchState": "FullTime",
                        "homeTeam": { "nickName": "Roosters", "score": 20 },
                        "awayTeam": { "nickName": "Rabbitohs", "score": 14 }
                    }
                ]
            }
        });

        let mut found = Vec::new();
        find_all_fixtures(&nested, &mut found);
        assert_eq!(found.len(), 1);
    }
}
