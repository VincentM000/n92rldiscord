use pirate_utils::ladder_tracker::{run_tracker_loop, LadderTracker};
use reqwest::blocking::Client;
use scraper::{Html, Selector};

const LADDER_URL: &str =
    "https://www.skysports.com/rugby-league/competitions/betfred-super-league/table";
const RESULTS_URL: &str =
    "https://www.skysports.com/rugby-league/competitions/super-league/results";
const SL_STATE_FILE: &str = "sl_ladder_state.txt";
const SL_MATCH_STATE_FILE: &str = "sl_match_state.txt";

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
        _ => "🏉",
    }
}

struct SuperLeagueTracker;

impl LadderTracker for SuperLeagueTracker {
    fn name(&self) -> &str {
        "Super League Tracker"
    }

    fn state_filename(&self) -> &str {
        SL_STATE_FILE
    }

    fn match_state_filename(&self) -> &str {
        SL_MATCH_STATE_FILE
    }

    fn fetch_latest_match_result(
        &self,
        client: &Client,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let response = client.get(RESULTS_URL).send()?.text()?;
        let document = Html::parse_document(&response);

        let match_selector = Selector::parse(".fixres__item .matches__link").unwrap();
        let team1_selector = Selector::parse(".matches__participant--side1").unwrap();
        let team2_selector = Selector::parse(".matches__participant--side2").unwrap();
        let score_selector = Selector::parse(".matches__teamscores-side").unwrap();

        if let Some(first_match) = document.select(&match_selector).next() {
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

            let scores: Vec<String> = first_match
                .select(&score_selector)
                .map(|e| e.text().collect::<String>().trim().to_string())
                .collect();

            if scores.len() >= 2 {
                let score1 = &scores[0];
                let score2 = &scores[1];

                let emoji1 = get_sl_team_emoji(&team1);
                let emoji2 = get_sl_team_emoji(&team2);

                let message = format!(
                    "🏁 **RÉSULTAT FINAL :**\n{} **{}** {} - {} **{}** {}",
                    emoji1, team1, score1, score2, team2, emoji2
                );
                return Ok(Some(message));
            }
        }

        Ok(None)
    }

    fn fetch_ladder(&self, client: &Client) -> Result<String, Box<dyn std::error::Error>> {
        let response = client.get(LADDER_URL).send()?.text()?;
        let document = Html::parse_document(&response);

        let row_selector = Selector::parse("tbody tr").unwrap();
        let team_link_selector = Selector::parse("a.sdc-site-table__link").unwrap();
        let cell_selector = Selector::parse("td.sdc-site-table__cell").unwrap();

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

            if cells.len() >= 8 {
                let wins = cells.get(2).unwrap_or(&"0".to_string()).clone();
                let drawn = cells.get(3).unwrap_or(&"0".to_string()).clone();
                let lost = cells.get(4).unwrap_or(&"0".to_string()).clone();
                let points = cells.last().unwrap_or(&"0".to_string()).clone();

                let emoji_str = get_sl_team_emoji(&team_name);

                current_ladder.push_str(&format!(
                    "**{}.** {} **{}** • **{} pts** *(V:{} N:{} D:{})*\n",
                    index, emoji_str, team_name, points, wins, drawn, lost
                ));

                index += 1;
            }
        }

        Ok(current_ladder)
    }
}

fn main() {
    env_logger::init();
    run_tracker_loop(SuperLeagueTracker);
}
