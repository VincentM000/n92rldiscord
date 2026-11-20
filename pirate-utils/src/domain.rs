/// Pure domain models and message formatters shared across all Rugby League trackers.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LadderRow {
    pub rank: usize,
    pub team_name: String,
    pub emoji: String,
    pub points: i32,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub suffix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchResult {
    pub header_suffix: Option<String>,
    pub home_team: String,
    pub home_emoji: String,
    pub home_score: i64,
    pub away_team: String,
    pub away_emoji: String,
    pub away_score: i64,
    pub venue: Option<String>,
}

/// Formats a complete league standings table for Discord.
pub fn format_ladder(competition_title: &str, rows: &[LadderRow]) -> String {
    let mut out = format!("🏆 **{}** 🏆\n\n", competition_title);

    for row in rows {
        let extra = match &row.suffix {
            Some(s) if !s.is_empty() => format!(" {}", s),
            _ => String::new(),
        };

        out.push_str(&format!(
            "**{}.** {} **{}** • **{} pts** *(V:{} N:{} D:{})*{}\n",
            row.rank, row.emoji, row.team_name, row.points, row.wins, row.draws, row.losses, extra
        ));
    }

    out
}

/// Formats a completed match result message for Discord.
pub fn format_match_result(result: &MatchResult) -> String {
    let header = match &result.header_suffix {
        Some(suffix) if !suffix.is_empty() => format!("🏁 **RÉSULTAT FINAL - {} :**", suffix),
        _ => "🏁 **RÉSULTAT FINAL :**".to_string(),
    };

    let mut message = format!(
        "{}\n{} **{}** {} - {} **{}** {}",
        header,
        result.home_emoji,
        result.home_team,
        result.home_score,
        result.away_score,
        result.away_team,
        result.away_emoji
    );

    if let Some(venue) = &result.venue {
        if !venue.is_empty() {
            message.push_str(&format!("\n📍 *{}*", venue));
        }
    }

    message
}

/// Calculates the sum of points across all teams in a formatted ladder string.
/// Used to detect administrative point deductions ("tapis vert").
pub fn extract_total_league_points(ladder: &str) -> i32 {
    let mut total = 0;
    for line in ladder.lines() {
        if let Some(start) = line.find("• **") {
            let after_bullet = &line[start + "• **".len()..];
            if let Some(end) = after_bullet.find(" pts**") {
                let pts_str = &after_bullet[..end];
                if let Ok(pts) = pts_str.parse::<i32>() {
                    total += pts;
                }
            }
        }
    }
    total
}

/// Extracts a substring located between `start_delim` and `end_delim`.
pub fn extract_between<'a>(src: &'a str, start_delim: &str, end_delim: &str) -> Option<&'a str> {
    let start = src.find(start_delim)? + start_delim.len();
    let rest = &src[start..];
    let end = rest.find(end_delim)?;
    Some(&rest[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_ladder_and_extract_points() {
        let rows = vec![
            LadderRow {
                rank: 1,
                team_name: "SO Avignon XIII".to_string(),
                emoji: "🦬".to_string(),
                points: 6,
                wins: 2,
                draws: 0,
                losses: 0,
                suffix: Some("• Diff: +13".to_string()),
            },
            LadderRow {
                rank: 2,
                team_name: "Carcassonne XIII".to_string(),
                emoji: "🐤".to_string(),
                points: 3,
                wins: 1,
                draws: 0,
                losses: 0,
                suffix: None,
            },
        ];

        let formatted = format_ladder("CLASSEMENT SUPER XIII", &rows);
        assert!(formatted.contains("🏆 **CLASSEMENT SUPER XIII** 🏆"));
        assert!(formatted
            .contains("**1.** 🦬 **SO Avignon XIII** • **6 pts** *(V:2 N:0 D:0)* • Diff: +13"));
        assert!(formatted.contains("**2.** 🐤 **Carcassonne XIII** • **3 pts** *(V:1 N:0 D:0)*\n"));
        assert_eq!(extract_total_league_points(&formatted), 9);
    }

    #[test]
    fn test_format_match_result_variants() {
        let simple = MatchResult {
            header_suffix: None,
            home_team: "Wigan Warriors".to_string(),
            home_emoji: "🍒".to_string(),
            home_score: 24,
            away_team: "St Helens".to_string(),
            away_emoji: "👼".to_string(),
            away_score: 12,
            venue: None,
        };
        assert_eq!(
            format_match_result(&simple),
            "🏁 **RÉSULTAT FINAL :**\n🍒 **Wigan Warriors** 24 - 12 **St Helens** 👼"
        );

        let detailed = MatchResult {
            header_suffix: Some("SUPER XIII (Journée 1)".to_string()),
            home_team: "VILLEFRANCHE XIII AVEYRON".to_string(),
            home_emoji: "🐺".to_string(),
            home_score: 17,
            away_team: "SO Avignon XIII".to_string(),
            away_emoji: "🦬".to_string(),
            away_score: 18,
            venue: Some("STADE HENRI LAGARDE".to_string()),
        };
        let msg = format_match_result(&detailed);
        assert!(msg.contains("🏁 **RÉSULTAT FINAL - SUPER XIII (Journée 1) :**"));
        assert!(msg.contains("📍 *STADE HENRI LAGARDE*"));
    }

    #[test]
    fn test_extract_between() {
        let sample = r#"{"watchEndpoint":{"videoId":"3WgCfirniC4"}}"#;
        assert_eq!(
            extract_between(sample, r#""watchEndpoint":{"videoId":""#, r#"""#),
            Some("3WgCfirniC4")
        );
        assert_eq!(extract_between(sample, "missing", "\""), None);
    }
}
