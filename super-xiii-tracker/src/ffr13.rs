use crate::clubs::get_club_display;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use pirate_utils::{format_ladder, format_match_result, LadderRow, MatchResult};
use scraper::{Html, Selector};
use serde_json::Value;
use std::time::Duration;

pub const FFR13_SUPER_XIII_URL: &str = "https://www.ffr13.fr/competitions/elite1/";

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FfrMatch {
    pub id: i64,
    pub num_rencontre: String,
    pub round: String,
    pub date_str: String,
    pub time_str: String,
    pub kickoff: Option<DateTime<Local>>,
    pub home_team: String,
    pub away_team: String,
    pub score_home: Option<i64>,
    pub score_away: Option<i64>,
    pub status_id: Option<i64>,
    pub stadium: String,
}

impl FfrMatch {
    pub fn is_finished(&self) -> bool {
        self.score_home.is_some() && self.score_away.is_some()
    }
}

/// Extract the official ranking table from the FFR13 page using shared `LadderRow` and `format_ladder`.
pub fn parse_ranking_table(html: &str) -> (Vec<LadderRow>, String) {
    let document = Html::parse_document(html);
    let row_selector = Selector::parse("table.ranking tr.rank-line").unwrap();
    let rank_selector = Selector::parse("td.rank").unwrap();
    let team_selector = Selector::parse("td.team").unwrap();
    let pts_selector = Selector::parse("td.points").unwrap();
    let win_selector = Selector::parse("td.win").unwrap();
    let draw_selector = Selector::parse("td.draw").unwrap();
    let lost_selector = Selector::parse("td.lost").unwrap();
    let diff_selector = Selector::parse("td.diff").unwrap();

    let mut rows = Vec::new();

    for (idx, row) in document.select(&row_selector).enumerate() {
        let rank = row
            .select(&rank_selector)
            .next()
            .and_then(|e| e.text().collect::<String>().trim().parse::<usize>().ok())
            .unwrap_or(idx + 1);

        let raw_team = row
            .select(&team_selector)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        let points = row
            .select(&pts_selector)
            .next()
            .and_then(|e| e.text().collect::<String>().trim().parse::<i32>().ok())
            .unwrap_or(0);

        let wins = row
            .select(&win_selector)
            .next()
            .and_then(|e| e.text().collect::<String>().trim().parse::<u32>().ok())
            .unwrap_or(0);

        let draws = row
            .select(&draw_selector)
            .next()
            .and_then(|e| e.text().collect::<String>().trim().parse::<u32>().ok())
            .unwrap_or(0);

        let losses = row
            .select(&lost_selector)
            .next()
            .and_then(|e| e.text().collect::<String>().trim().parse::<u32>().ok())
            .unwrap_or(0);

        let diff = row
            .select(&diff_selector)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| "0".to_string());

        let (canonical_name, emoji) = get_club_display(&raw_team);

        let diff_formatted = if !diff.starts_with('-') && diff != "0" {
            format!("+{}", diff)
        } else {
            diff
        };

        rows.push(LadderRow {
            rank,
            team_name: canonical_name,
            emoji: emoji.to_string(),
            points,
            wins,
            draws,
            losses,
            suffix: Some(format!("• Diff: {}", diff_formatted)),
        });
    }

    let ladder_message = format_ladder("CLASSEMENT SUPER XIII", &rows);
    (rows, ladder_message)
}

/// Parse matches from the embedded `jsonContent` JavaScript variable.
pub fn parse_all_matches(html: &str) -> Vec<FfrMatch> {
    let mut matches = Vec::new();

    let start_needle = "var jsonContent =";
    let start_idx = match html.find(start_needle) {
        Some(idx) => idx + start_needle.len(),
        None => return matches,
    };

    let remaining = &html[start_idx..];
    let end_idx = match remaining.find(";\n").or_else(|| remaining.find(';')) {
        Some(idx) => idx,
        None => return matches,
    };

    let json_str = remaining[..end_idx].trim();
    let json_data: Value = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return matches,
    };

    let map = match json_data.as_object() {
        Some(m) => m,
        None => return matches,
    };

    for (_journee_id, games_val) in map {
        if let Some(games) = games_val.as_array() {
            for g in games {
                let id = g["id"].as_i64().unwrap_or(0);
                let num_rencontre = g["num_rencontre"].as_str().unwrap_or_default().to_string();
                let round = g["num_journee"].as_str().unwrap_or("?").to_string();
                let date_str = g["dt_rencontre"].as_str().unwrap_or_default().to_string();
                let time_str = g["time_rencontre"].as_str().unwrap_or_default().to_string();

                let home = g["equipe_domicile"]["lb_equipe"]
                    .as_str()
                    .or_else(|| g["lb_club_domicile"].as_str())
                    .unwrap_or("Unknown")
                    .to_string();

                let away = g["equipe_visiteur"]["lb_equipe"]
                    .as_str()
                    .or_else(|| g["lb_club_visiteur"].as_str())
                    .unwrap_or("Unknown")
                    .to_string();

                let score_home = g["resultat_domicile"].as_i64();
                let score_away = g["resultat_visiteur"].as_i64();
                let status_id = g["statut_match_id"].as_i64();
                let stadium = g["lb_stade"].as_str().unwrap_or_default().to_string();

                let kickoff = if !date_str.is_empty() && !time_str.is_empty() {
                    let dt_str = format!("{} {}:00", date_str, time_str);
                    NaiveDateTime::parse_from_str(&dt_str, "%Y-%m-%d %H:%M:%S")
                        .ok()
                        .and_then(|naive| Local.from_local_datetime(&naive).single())
                } else {
                    None
                };

                matches.push(FfrMatch {
                    id,
                    num_rencontre,
                    round,
                    date_str,
                    time_str,
                    kickoff,
                    home_team: home,
                    away_team: away,
                    score_home,
                    score_away,
                    status_id,
                    stadium,
                });
            }
        }
    }

    matches
}

/// Returns matches whose kickoff is within the pre-game window [-10 mins, +15 mins].
pub fn get_matches_in_prematch_window(matches: &[FfrMatch]) -> Vec<FfrMatch> {
    let now = Local::now();
    let mut prematch = Vec::new();

    for m in matches {
        if m.is_finished() {
            continue;
        }
        if let Some(kickoff) = m.kickoff {
            let diff_secs = kickoff.signed_duration_since(now).num_seconds();
            if (-600..=900).contains(&diff_secs) {
                prematch.push(m.clone());
            }
        }
    }

    prematch
}

/// Calculates adaptive polling sleep duration based on the schedule of rounds and matches.
pub fn calculate_adaptive_sleep(matches: &[FfrMatch]) -> Duration {
    let now = Local::now();
    let mut min_sleep_secs: i64 = 1800;

    let mut is_match_live_now = false;
    let mut is_prematch_window = false;
    let mut closest_future_kickoff_secs: Option<i64> = None;
    let mut match_finished_recently_missing_score = false;

    for m in matches {
        if let Some(kickoff) = m.kickoff {
            let diff_secs = kickoff.signed_duration_since(now).num_seconds();

            if !m.is_finished() && (-600..=900).contains(&diff_secs) {
                is_prematch_window = true;
            }

            if (-8100..=0).contains(&diff_secs) && !m.is_finished() {
                is_match_live_now = true;
            }

            if diff_secs > 0 && diff_secs < 86400 {
                closest_future_kickoff_secs = match closest_future_kickoff_secs {
                    Some(cur) => Some(cur.min(diff_secs)),
                    None => Some(diff_secs),
                };
            }

            if diff_secs < -8100 && diff_secs > -28800 && !m.is_finished() {
                match_finished_recently_missing_score = true;
            }
        }
    }

    if is_prematch_window {
        min_sleep_secs = 60;
    } else if is_match_live_now {
        min_sleep_secs = 180;
    } else if let Some(secs_to_start) = closest_future_kickoff_secs {
        if secs_to_start <= 900 {
            min_sleep_secs = 60;
        } else if secs_to_start <= 3600 {
            min_sleep_secs = 300;
        } else if secs_to_start <= 7200 {
            min_sleep_secs = 900;
        }
    } else if match_finished_recently_missing_score {
        min_sleep_secs = 300;
    }

    Duration::from_secs(min_sleep_secs as u64)
}

/// Formats the Discord match result message using shared `pirate_utils::format_match_result`.
pub fn format_match_result_message(m: &FfrMatch) -> String {
    let (home_name, home_emoji) = get_club_display(&m.home_team);
    let (away_name, away_emoji) = get_club_display(&m.away_team);

    let result = MatchResult {
        header_suffix: Some(format!("SUPER XIII (Journée {})", m.round)),
        home_team: home_name,
        home_emoji: home_emoji.to_string(),
        home_score: m.score_home.unwrap_or(0),
        away_team: away_name,
        away_emoji: away_emoji.to_string(),
        away_score: m.score_away.unwrap_or(0),
        venue: if m.stadium.is_empty() {
            None
        } else {
            Some(m.stadium.clone())
        },
    };

    format_match_result(&result)
}

/// Formats Discord pre-match imminent alert.
pub fn format_prematch_alert_message(m: &FfrMatch, video_title: &str, video_url: &str) -> String {
    let (home_name, home_emoji) = get_club_display(&m.home_team);
    let (away_name, away_emoji) = get_club_display(&m.away_team);

    let mut message = format!(
        "⏰ **COUP D'ENVOI IMMINENT - SUPER XIII !** 🏉 🏴‍☠️\n\nLe match **{}** {} vs **{}** {} (Journée {}) débute dans quelques minutes !\n📺 Retransmission en direct sur YouTube :\n🎥 **{}**\n\n🔗 {}",
        home_name, home_emoji, away_name, away_emoji, m.round, video_title, video_url
    );

    if !m.stadium.is_empty() {
        message.push_str(&format!("\n📍 *{}*", m.stadium));
    }

    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_match_result_message() {
        let m = FfrMatch {
            id: 12629,
            num_rencontre: "26102468".to_string(),
            round: "1".to_string(),
            date_str: "2026-10-25".to_string(),
            time_str: "15:30".to_string(),
            kickoff: None,
            home_team: "VILLEFRANCHE XIII AVEYRON - SUPER XIII".to_string(),
            away_team: "SO AVIGNON XIII".to_string(),
            score_home: Some(17),
            score_away: Some(18),
            status_id: Some(1),
            stadium: "STADE HENRI LAGARDE".to_string(),
        };

        let msg = format_match_result_message(&m);
        assert!(msg.contains("RÉSULTAT FINAL - SUPER XIII (Journée 1)"));
        assert!(msg.contains("🐺 **VILLEFRANCHE XIII AVEYRON** 17 - 18 **SO Avignon XIII** 🦬"));
        assert!(msg.contains("STADE HENRI LAGARDE"));
    }

    #[test]
    fn test_format_prematch_alert_message() {
        let m = FfrMatch {
            id: 12630,
            num_rencontre: "26102469".to_string(),
            round: "2".to_string(),
            date_str: "2026-11-01".to_string(),
            time_str: "16:30".to_string(),
            kickoff: None,
            home_team: "Carcassonne XIII".to_string(),
            away_team: "FC Lézignan XIII".to_string(),
            score_home: None,
            score_away: None,
            status_id: Some(4),
            stadium: "Stade Albert Domec".to_string(),
        };

        let msg = format_prematch_alert_message(
            &m,
            "🟡⚫️SUPER XIII | Carcassonne XIII V Lézignan XIII",
            "https://www.youtube.com/watch?v=3WgCfirniC4",
        );

        assert!(msg.contains("COUP D'ENVOI IMMINENT - SUPER XIII !"));
        assert!(msg.contains("🐤"));
        assert!(msg.contains("🟢"));
        assert!(msg.contains("https://www.youtube.com/watch?v=3WgCfirniC4"));
    }

    #[test]
    fn test_get_matches_in_prematch_window() {
        let now = Local::now();
        let imminent_match = FfrMatch {
            id: 9999,
            num_rencontre: "test1".to_string(),
            round: "2".to_string(),
            date_str: "2026-11-01".to_string(),
            time_str: "16:30".to_string(),
            kickoff: Some(now + chrono::Duration::minutes(5)),
            home_team: "Carcassonne XIII".to_string(),
            away_team: "FC Lézignan XIII".to_string(),
            score_home: None,
            score_away: None,
            status_id: Some(4),
            stadium: "".to_string(),
        };

        let distant_match = FfrMatch {
            id: 8888,
            num_rencontre: "test2".to_string(),
            round: "2".to_string(),
            date_str: "2026-11-01".to_string(),
            time_str: "20:30".to_string(),
            kickoff: Some(now + chrono::Duration::hours(5)),
            home_team: "Albi RL XIII".to_string(),
            away_team: "Pia XIII Baroudeur".to_string(),
            score_home: None,
            score_away: None,
            status_id: Some(4),
            stadium: "".to_string(),
        };

        let list = vec![imminent_match, distant_match];
        let prematch = get_matches_in_prematch_window(&list);
        assert_eq!(prematch.len(), 1);
        assert_eq!(prematch[0].id, 9999);
    }

    #[test]
    fn test_parse_ranking_table_mock() {
        let mock_html = r#"
            <table class="ranking">
                <tr class="rank-line">
                    <td class="rank">1</td>
                    <td class="team">SO Avignon XIII</td>
                    <td class="points">6</td>
                    <td class="played">2</td>
                    <td class="win">2</td>
                    <td class="draw">0</td>
                    <td class="lost">0</td>
                    <td class="diff">13</td>
                </tr>
            </table>
        "#;

        let (rows, ladder_msg) = parse_ranking_table(mock_html);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].team_name, "SO Avignon XIII");
        assert_eq!(rows[0].points, 6);
        assert!(ladder_msg
            .contains("**1.** 🦬 **SO Avignon XIII** • **6 pts** *(V:2 N:0 D:0)* • Diff: +13"));
    }
}
