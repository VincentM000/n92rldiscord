use pirate_utils::ladder_tracker::{run_tracker_loop, LadderTracker, MATCH_DELIMITER};
use pirate_utils::{format_ladder, format_match_result, LadderRow, MatchResult};
use reqwest::blocking::Client;
use scraper::{Html, Selector};

const LADDER_URL: &str =
    "https://www.skysports.com/rugby-league/competitions/betfred-super-league/table";
const RESULTS_URL: &str =
    "https://www.skysports.com/rugby-league/competitions/super-league/fixtures";
const SL_STATE_FILE: &str = "sl_ladder_state.txt";
const SL_MATCH_STATE_FILE: &str = "sl_match_state.txt";

fn get_sl_team_emoji(team_name: &str) -> &'static str {
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

fn parse_sl_ladder_html(html: &str) -> String {
    let document = Html::parse_document(html);
    let row_selector = Selector::parse("tbody tr").unwrap();
    let team_link_selector = Selector::parse("a.sdc-site-table__link").unwrap();
    let cell_selector = Selector::parse("td.sdc-site-table__cell").unwrap();

    let mut rows = Vec::new();
    let mut rank = 1;

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
            let wins = cells
                .get(2)
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            let draws = cells
                .get(3)
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            let losses = cells
                .get(4)
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            let points = cells
                .last()
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0);

            let emoji = get_sl_team_emoji(&team_name).to_string();

            rows.push(LadderRow {
                rank,
                team_name,
                emoji,
                points,
                wins,
                draws,
                losses,
                suffix: None,
            });

            rank += 1;
        }
    }

    format_ladder("CLASSEMENT SUPER LEAGUE", &rows)
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

        let match_selector =
            Selector::parse(".fixres__item .matches__link[data-status=\"FT\"]").unwrap();
        let team1_selector = Selector::parse(".matches__participant--side1").unwrap();
        let team2_selector = Selector::parse(".matches__participant--side2").unwrap();
        let score_selector = Selector::parse(".matches__teamscores-side").unwrap();

        let mut all_ft_matches = Vec::new();

        for match_node in document.select(&match_selector) {
            let team1 = match_node
                .select(&team1_selector)
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string())
                .unwrap_or_default();

            let team2 = match_node
                .select(&team2_selector)
                .next()
                .map(|e| e.text().collect::<String>().trim().to_string())
                .unwrap_or_default();

            let scores: Vec<i64> = match_node
                .select(&score_selector)
                .filter_map(|e| e.text().collect::<String>().trim().parse::<i64>().ok())
                .collect();

            if scores.len() >= 2 {
                let result = MatchResult {
                    header_suffix: None,
                    home_emoji: get_sl_team_emoji(&team1).to_string(),
                    home_team: team1,
                    home_score: scores[0],
                    away_emoji: get_sl_team_emoji(&team2).to_string(),
                    away_team: team2,
                    away_score: scores[1],
                    venue: None,
                };
                all_ft_matches.push(format_match_result(&result));
            }
        }

        if all_ft_matches.is_empty() {
            Ok(None)
        } else {
            Ok(Some(all_ft_matches.join(MATCH_DELIMITER)))
        }
    }

    fn fetch_ladder(&self, client: &Client) -> Result<String, Box<dyn std::error::Error>> {
        let response = client.get(LADDER_URL).send()?.text()?;
        Ok(parse_sl_ladder_html(&response))
    }
}

fn main() {
    env_logger::init();
    run_tracker_loop(SuperLeagueTracker);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sl_ladder_html() {
        let html = r#"
        <table>
            <tbody>
                <tr>
                    <td class="sdc-site-table__cell">1</td>
                    <td class="sdc-site-table__cell">
                        <a class="sdc-site-table__link" aria-label="Catalans Dragons">Catalans Dragons</a>
                    </td>
                    <td class="sdc-site-table__cell">10</td>
                    <td class="sdc-site-table__cell">0</td>
                    <td class="sdc-site-table__cell">2</td>
                    <td class="sdc-site-table__cell">300</td>
                    <td class="sdc-site-table__cell">150</td>
                    <td class="sdc-site-table__cell">150</td>
                    <td class="sdc-site-table__cell">20</td>
                </tr>
            </tbody>
        </table>
        "#;

        let formatted = parse_sl_ladder_html(html);
        assert!(formatted.contains("🏆 **CLASSEMENT SUPER LEAGUE** 🏆"));
        assert!(formatted.contains("**1.** 🐉 **Catalans Dragons** • **20 pts** *(V:10 N:0 D:2)*"));
    }
}
