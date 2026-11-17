use crate::clubs::get_club_display;
use chrono::{DateTime, Datelike, FixedOffset, Weekday};
use log::{info, warn};
pub use pirate_utils::{paris_now, paris_offset_for_utc, parse_paris_datetime};
use pirate_utils::{format_ladder, format_match_result, LadderRow, MatchResult};
use scraper::{Html, Selector};
use serde_json::Value;
use std::collections::HashMap;
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
    pub kickoff: Option<DateTime<FixedOffset>>,
    pub home_team: String,
    pub away_team: String,
    pub score_home: Option<i64>,
    pub score_away: Option<i64>,
    pub status_id: Option<i64>,
    pub stadium: String,
}

/// Minimum elapsed time (in seconds) after kickoff before a score can be considered final (1h50m = 6600s).
/// Prevents half-time or live intermediate scores (e.g. at H+50m) from being mistaken for final results.
pub const MIN_MATCH_DURATION_SECS: i64 = 6600;

impl FfrMatch {
    pub fn has_score(&self) -> bool {
        self.score_home.is_some() && self.score_away.is_some()
    }

    pub fn is_finished(&self) -> bool {
        self.is_finished_at(paris_now())
    }

    pub fn is_finished_at(&self, now: DateTime<FixedOffset>) -> bool {
        if !self.has_score() {
            return false;
        }
        if let Some(kickoff) = self.kickoff {
            let elapsed_secs = now.signed_duration_since(kickoff).num_seconds();
            elapsed_secs >= MIN_MATCH_DURATION_SECS
        } else {
            true
        }
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

fn parse_diff_from_suffix(suffix: &Option<String>) -> i32 {
    suffix
        .as_deref()
        .and_then(|s| s.split("Diff:").nth(1))
        .and_then(|v| v.trim().trim_start_matches('+').parse::<i32>().ok())
        .unwrap_or(0)
}

fn format_diff_suffix(diff: i32) -> String {
    if diff > 0 {
        format!("• Diff: +{}", diff)
    } else {
        format!("• Diff: {}", diff)
    }
}

fn count_finished_matches_by_team(all_matches: &[FfrMatch]) -> HashMap<String, u32> {
    let mut counts = HashMap::new();
    for m in all_matches {
        if m.is_finished() {
            let (home_name, _) = get_club_display(&m.home_team);
            let (away_name, _) = get_club_display(&m.away_team);
            *counts.entry(home_name).or_insert(0) += 1;
            *counts.entry(away_name).or_insert(0) += 1;
        }
    }
    counts
}

fn apply_match_to_team_row(
    rows: &mut [LadderRow],
    finished_count_by_team: &HashMap<String, u32>,
    team_name: &str,
    scored: i64,
    conceded: i64,
) {
    let Some(row) = rows.iter_mut().find(|r| r.team_name == team_name) else {
        return;
    };
    let played_in_table = row.wins + row.draws + row.losses;
    let expected_played = finished_count_by_team.get(team_name).copied().unwrap_or(0);
    if played_in_table >= expected_played {
        return;
    }

    if scored > conceded {
        row.wins += 1;
        row.points += 3;
    } else if scored == conceded {
        row.draws += 1;
        row.points += 2;
    } else {
        row.losses += 1;
        if (conceded - scored) <= 12 {
            row.points += 1; // Bonus défensif Super XIII (défaite <= 12 pts)
        }
    }
    let new_diff = parse_diff_from_suffix(&row.suffix) + (scored - conceded) as i32;
    row.suffix = Some(format_diff_suffix(new_diff));
}

/// Checks whether the official FFR13 HTML `table.ranking` has incorporated all finished matches
/// from `jsonContent` (i.e. no team in `rows` has fewer played games than its finished matches).
pub fn is_official_table_up_to_date(rows: &[LadderRow], all_matches: &[FfrMatch]) -> bool {
    if rows.is_empty() {
        return false;
    }

    let finished_count_by_team = count_finished_matches_by_team(all_matches);
    rows.iter().all(|row| {
        let played_in_table = row.wins + row.draws + row.losses;
        let expected_played = finished_count_by_team
            .get(&row.team_name)
            .copied()
            .unwrap_or(0);
        played_in_table >= expected_played
    })
}

/// Updates the standings in Rust with any newly finished matches that FFR13 has not yet
/// incorporated into its HTML `table.ranking`, then re-sorts and formats the ladder.
pub fn apply_uncounted_matches_to_ladder(
    rows: &mut [LadderRow],
    all_matches: &[FfrMatch],
    newly_finished: &[FfrMatch],
) -> String {
    let finished_count_by_team = count_finished_matches_by_team(all_matches);

    for m in newly_finished {
        let (Some(sh), Some(sa)) = (m.score_home, m.score_away) else {
            continue;
        };
        let (home_name, _) = get_club_display(&m.home_team);
        let (away_name, _) = get_club_display(&m.away_team);
        apply_match_to_team_row(rows, &finished_count_by_team, &home_name, sh, sa);
        apply_match_to_team_row(rows, &finished_count_by_team, &away_name, sa, sh);
    }

    // Sort by points DESC, then point differential DESC
    rows.sort_by(|a, b| {
        let diff_a = parse_diff_from_suffix(&a.suffix);
        let diff_b = parse_diff_from_suffix(&b.suffix);
        b.points.cmp(&a.points).then_with(|| diff_b.cmp(&diff_a))
    });

    for (idx, row) in rows.iter_mut().enumerate() {
        row.rank = idx + 1;
    }

    format_ladder("CLASSEMENT SUPER XIII", rows)
}

/// Parse matches from the embedded `jsonContent` JavaScript variable.
pub fn parse_all_matches(html: &str) -> Vec<FfrMatch> {
    let mut matches = Vec::new();

    let start_needle = "var jsonContent =";
    let start_idx = match html.find(start_needle) {
        Some(idx) => idx + start_needle.len(),
        None => return matches,
    };

    let remaining = html[start_idx..].trim_start();
    let json_data: Value = match serde_json::Deserializer::from_str(remaining)
        .into_iter::<Value>()
        .next()
    {
        Some(Ok(v)) => v,
        Some(Err(e)) => {
            warn!("Erreur de parsing JSON dans jsonContent FFR13 : {}", e);
            return matches;
        }
        None => return matches,
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

                let kickoff = parse_paris_datetime(&date_str, &time_str);

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

/// Returns matches whose kickoff is within the pre-game/live-start window [-45 mins, +15 mins] (Paris time).
pub fn get_matches_in_prematch_window(matches: &[FfrMatch]) -> Vec<FfrMatch> {
    let now = paris_now();
    let mut prematch = Vec::new();

    for m in matches {
        if m.is_finished() {
            continue;
        }
        if let Some(kickoff) = m.kickoff {
            let diff_secs = kickoff.signed_duration_since(now).num_seconds();
            if (-2700..=900).contains(&diff_secs) {
                prematch.push(m.clone());
            }
        }
    }

    prematch
}

/// Calculates adaptive polling sleep duration based on the schedule of rounds and matches (Paris time).
/// - On match days (or Sundays): polls frequently around matches and every 30m otherwise.
/// - Outside match days (e.g. Mon-Fri with no match today): polls once per day (24h) to check
///   for official FFR13 standings corrections and daily YouTube replays.
pub fn calculate_adaptive_sleep(matches: &[FfrMatch]) -> Duration {
    let now = paris_now();
    let today_str = now.format("%Y-%m-%d").to_string();
    let is_match_day =
        now.weekday() == Weekday::Sun || matches.iter().any(|m| m.date_str == today_str);

    // Default: 30 mins on match days / Sundays, 24 hours (86400s) on non-match days
    let mut min_sleep_secs: i64 = if is_match_day { 1800 } else { 86400 };

    let mut is_match_live_now = false;
    let mut is_prematch_window = false;
    let mut closest_future_match: Option<(i64, &FfrMatch)> = None;
    let mut match_finished_recently_missing_score = false;

    for m in matches {
        if let Some(kickoff) = m.kickoff {
            let diff_secs = kickoff.signed_duration_since(now).num_seconds();

            if !m.is_finished() && (-600..=900).contains(&diff_secs) {
                is_prematch_window = true;
            }

            if (-8100..=0).contains(&diff_secs) && !m.is_finished_at(now) {
                is_match_live_now = true;
                if let (Some(sh), Some(sa)) = (m.score_home, m.score_away) {
                    info!(
                        "🏉 Match en cours : {} vs {} (score provisoire : {} - {}, coup d'envoi à {})",
                        m.home_team, m.away_team, sh, sa, m.time_str
                    );
                } else {
                    info!(
                        "🏉 Match en cours : {} vs {} (coup d'envoi à {}, heure de Paris)",
                        m.home_team, m.away_team, m.time_str
                    );
                }
            }

            if diff_secs > 0 && !m.is_finished() {
                closest_future_match = match closest_future_match {
                    Some((cur_secs, _)) if cur_secs <= diff_secs => closest_future_match,
                    _ => Some((diff_secs, m)),
                };
            }

            if diff_secs < -8100 && diff_secs > -28800 && !m.is_finished() {
                match_finished_recently_missing_score = true;
            }
        }
    }

    if let Some((secs_to_start, next_match)) = closest_future_match {
        let hours = secs_to_start / 3600;
        let mins = (secs_to_start % 3600) / 60;
        let (home_display, _) = get_club_display(&next_match.home_team);
        let (away_display, _) = get_club_display(&next_match.away_team);
        info!(
            "📅 Prochain match à {} : {} vs {} (le {}, coup d'envoi dans {}h{:02}m - heure de Paris {})",
            next_match.time_str,
            home_display,
            away_display,
            next_match.date_str,
            hours,
            mins,
            now.format("%H:%M")
        );
    }

    if is_prematch_window {
        min_sleep_secs = 60;
    } else if is_match_live_now {
        min_sleep_secs = 180;
    } else if let Some((secs_to_start, _)) = closest_future_match {
        if secs_to_start <= 900 {
            min_sleep_secs = 60;
        } else if secs_to_start <= 1800 {
            // Wake up right when the [H-15m] pre-match window opens!
            min_sleep_secs = (secs_to_start - 895).max(60);
        } else if secs_to_start <= 3600 {
            // Between 30m and 1h before kickoff: poll every 5m, or wake up at H-15m
            min_sleep_secs = 300.min((secs_to_start - 895).max(60));
        } else if secs_to_start <= 7200 {
            // Between 1h and 2h before kickoff: poll every 15m, or wake up at H-1h
            min_sleep_secs = 900.min((secs_to_start - 3600).max(60));
        } else if secs_to_start <= 86400 {
            // Within 24h of kickoff: poll every 30m on match day, or wake up at H-2h
            let base = if is_match_day { 1800 } else { 86400 };
            min_sleep_secs = base.min((secs_to_start - 7200).max(60));
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

/// Formats Discord pre-match imminent alert (waiting room / upcoming).
pub fn format_prematch_alert_message(m: &FfrMatch, video_title: &str, video_url: &str) -> String {
    let (home_name, home_emoji) = get_club_display(&m.home_team);
    let (away_name, away_emoji) = get_club_display(&m.away_team);

    let mut message = format!(
        "⏰ **COUP D'ENVOI IMMINENT - SUPER XIII !** 🏉 🏴‍☠️\n\nLe match **{}** {} vs **{}** {} (Journée {}) débute dans quelques minutes !\n📺 Salle d'attente ouverte sur YouTube :\n🎥 **{}**\n\n🔗 {}",
        home_name, home_emoji, away_name, away_emoji, m.round, video_title, video_url
    );

    if !m.stadium.is_empty() {
        message.push_str(&format!("\n📍 *{}*", m.stadium));
    }

    message
}

/// Formats Discord alert when the live broadcast officially starts (`is_live_now == true`).
pub fn format_live_started_alert_message(
    m: &FfrMatch,
    video_title: &str,
    video_url: &str,
) -> String {
    let (home_name, home_emoji) = get_club_display(&m.home_team);
    let (away_name, away_emoji) = get_club_display(&m.away_team);

    let mut message = format!(
        "⚔️ **LE MATCH EST EN DIRECT - SUPER XIII !** ⚔️ 🏴‍☠️\n\nLe coup d'envoi de **{}** {} vs **{}** {} (Journée {}) vient d'être donné !\n🔴 **La retransmission en direct a commencé sur YouTube :**\n🎥 **{}**\n\n🔗 {}",
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
        let now = paris_now();
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
    fn test_paris_timezone_dst_and_winter() {
        use chrono::{TimeZone, Utc};
        // Nov 1 2026 14:00 UTC is in winter time (CET = UTC+1 -> 15:00 Paris)
        let winter_utc = Utc.with_ymd_and_hms(2026, 11, 1, 14, 0, 0).unwrap();
        assert_eq!(paris_offset_for_utc(winter_utc).local_minus_utc(), 3600);

        // July 15 2026 14:00 UTC is in summer time (CEST = UTC+2 -> 16:00 Paris)
        let summer_utc = Utc.with_ymd_and_hms(2026, 7, 15, 14, 0, 0).unwrap();
        assert_eq!(paris_offset_for_utc(summer_utc).local_minus_utc(), 7200);

        let parsed = parse_paris_datetime("2026-11-01", "16:30").unwrap();
        assert_eq!(parsed.offset().local_minus_utc(), 3600);
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

    #[test]
    fn test_parse_all_matches_stream_deserializer() {
        let mock_html = "<html><script>\n            var jsonContent = {\"5191\":[{\"id\":12639,\"journee_id\":5191,\"num_rencontre\":\"26103176\",\"dt_rencontre\":\"2026-11-01\",\"time_rencontre\":\"16:30\",\"equipe_domicile\":{\"lb_equipe\":\"Carcassonne XIII\"},\"equipe_visiteur\":{\"lb_equipe\":\"FC Lézignan XIII\"},\"resultat_domicile\":null,\"resultat_visiteur\":null,\"statut_match_id\":4,\"lb_stade\":\"STADE ALBERT DOMEC\",\"num_journee\":\"2\"}]};\n            var currentJourneeId = 5191;\r\n</script></html>";
        let matches = parse_all_matches(mock_html);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, 12639);
        assert_eq!(matches[0].home_team, "Carcassonne XIII");
        assert_eq!(matches[0].away_team, "FC Lézignan XIII");
        assert_eq!(matches[0].time_str, "16:30");
        assert!(matches[0].kickoff.is_some());
    }

    #[test]
    fn test_halftime_score_not_treated_as_finished() {
        let kickoff = parse_paris_datetime("2026-11-01", "16:30").unwrap();
        let m = FfrMatch {
            id: 12639,
            num_rencontre: "26103176".to_string(),
            round: "2".to_string(),
            date_str: "2026-11-01".to_string(),
            time_str: "16:30".to_string(),
            kickoff: Some(kickoff),
            home_team: "Carcassonne XIII".to_string(),
            away_team: "FC Lézignan XIII".to_string(),
            score_home: Some(12),
            score_away: Some(6),
            status_id: Some(4),
            stadium: "STADE ALBERT DOMEC".to_string(),
        };

        // At 17:22 (52 mins after kickoff -> half-time), must NOT be considered finished
        let at_17_22 = parse_paris_datetime("2026-11-01", "17:22").unwrap();
        assert!(m.has_score());
        assert!(!m.is_finished_at(at_17_22));

        // At 18:25 (1h55m after kickoff -> full-time), MUST be considered finished
        let at_18_25 = parse_paris_datetime("2026-11-01", "18:25").unwrap();
        assert!(m.is_finished_at(at_18_25));
    }

    #[test]
    fn test_apply_uncounted_matches_to_ladder() {
        let mut rows = vec![
            LadderRow {
                rank: 1,
                team_name: "Carcassonne XIII".to_string(),
                emoji: "🐤".to_string(),
                points: 3,
                wins: 1,
                draws: 0,
                losses: 0,
                suffix: Some("• Diff: +52".to_string()),
            },
            LadderRow {
                rank: 2,
                team_name: "FC Lézignan XIII".to_string(),
                emoji: "🟢".to_string(),
                points: 1,
                wins: 0,
                draws: 0,
                losses: 1,
                suffix: Some("• Diff: -10".to_string()),
            },
        ];

        let past_j1_carc = FfrMatch {
            id: 12627,
            num_rencontre: "1".to_string(),
            round: "1".to_string(),
            date_str: "2026-10-24".to_string(),
            time_str: "18:00".to_string(),
            kickoff: None,
            home_team: "RC ST GAUDENS".to_string(),
            away_team: "Carcassonne XIII".to_string(),
            score_home: Some(0),
            score_away: Some(52),
            status_id: Some(1),
            stadium: "".to_string(),
        };
        let past_j1_fcl = FfrMatch {
            id: 12625,
            num_rencontre: "2".to_string(),
            round: "1".to_string(),
            date_str: "2026-10-24".to_string(),
            time_str: "17:00".to_string(),
            kickoff: None,
            home_team: "FC Lézignan XIII".to_string(),
            away_team: "Saint-Estève XIII Catalan".to_string(),
            score_home: Some(18),
            score_away: Some(28),
            status_id: Some(1),
            stadium: "".to_string(),
        };
        let new_j2 = FfrMatch {
            id: 12639,
            num_rencontre: "3".to_string(),
            round: "2".to_string(),
            date_str: "2026-11-01".to_string(),
            time_str: "16:30".to_string(),
            kickoff: None,
            home_team: "Carcassonne XIII".to_string(),
            away_team: "FC Lézignan XIII".to_string(),
            score_home: Some(24),
            score_away: Some(18),
            status_id: Some(4),
            stadium: "STADE ALBERT DOMEC".to_string(),
        };

        let all_matches = vec![past_j1_carc, past_j1_fcl, new_j2.clone()];
        assert!(!is_official_table_up_to_date(&rows, &all_matches));

        let updated = apply_uncounted_matches_to_ladder(&mut rows, &all_matches, &[new_j2]);
        assert!(is_official_table_up_to_date(&rows, &all_matches));

        // Carcassonne gets +3 pts (3 -> 6), wins 1 -> 2, diff +52 + 6 = +58
        assert_eq!(rows[0].team_name, "Carcassonne XIII");
        assert_eq!(rows[0].points, 6);
        assert_eq!(rows[0].wins, 2);
        assert!(updated.contains("Diff: +58"));

        // Lézignan loses by 6 (<= 12) -> +1 defensive bonus pt (1 -> 2), losses 1 -> 2, diff -10 - 6 = -16
        assert_eq!(rows[1].team_name, "FC Lézignan XIII");
        assert_eq!(rows[1].points, 2);
        assert_eq!(rows[1].losses, 2);
        assert!(updated.contains("Diff: -16"));
    }
}
