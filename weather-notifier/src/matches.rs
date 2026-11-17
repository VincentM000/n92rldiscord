use chrono::{NaiveDate, NaiveTime, Timelike};
use log::warn;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    Adults,
    Children,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchEntry {
    pub date: NaiveDate,
    pub home_city: String,
    pub away_city: Option<String>,
    pub time: Option<NaiveTime>,
    pub category_label: Option<String>,
    pub audience: Audience,
}

fn parse_match_date(raw: &str) -> Option<NaiveDate> {
    let s = raw.trim();
    NaiveDate::parse_from_str(s, "%d/%m/%Y")
        .or_else(|_| NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .ok()
}

fn parse_match_time(raw: &str) -> Option<NaiveTime> {
    let s = raw.trim().to_ascii_lowercase();
    if let Some(hour_str) = s.strip_suffix('h') {
        let hour: u32 = hour_str.parse().ok()?;
        return NaiveTime::from_hms_opt(hour, 0, 0);
    }
    NaiveTime::parse_from_str(&s, "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(&s, "%Hh%M"))
        .ok()
}

fn detect_audience(category: Option<&str>) -> Audience {
    let Some(cat) = category else {
        return Audience::Adults;
    };
    let lower = cat.to_ascii_lowercase();
    if lower.contains("senior") || lower.contains("adulte") {
        Audience::Adults
    } else {
        Audience::Children
    }
}

/// Parses a match line such as `22/11/2026 Nanterre/Grigny 15H - Seniors`.
pub fn parse_match_line(line: &str) -> Option<MatchEntry> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    if trimmed.contains(';') || trimmed.contains('|') {
        let sep = if trimmed.contains(';') { ';' } else { '|' };
        let parts: Vec<&str> = trimmed.split(sep).map(str::trim).collect();
        if parts.len() >= 3 && !parts[2].is_empty() {
            let date = parse_match_date(parts[0])?;
            let time = parse_match_time(parts[1]);
            let category_label = parts
                .get(3)
                .map(|s| (*s).to_string())
                .filter(|s| !s.is_empty());
            let audience = detect_audience(category_label.as_deref());
            return Some(MatchEntry {
                date,
                home_city: parts[2].to_string(),
                away_city: None,
                time,
                category_label,
                audience,
            });
        }
        return None;
    }

    let (left, category_label) = match trimmed.split_once(" - ") {
        Some((l, r)) => {
            let cat = r.trim();
            (l.trim(), (!cat.is_empty()).then(|| cat.to_string()))
        }
        None => (trimmed, None),
    };

    let tokens: Vec<&str> = left.split_whitespace().collect();
    if tokens.len() < 2 {
        return None;
    }

    let date = parse_match_date(tokens[0])?;
    let mut time = None;
    let mut matchup_end = tokens.len();

    if tokens.len() >= 3 {
        if let Some(parsed_time) = parse_match_time(tokens[tokens.len() - 1]) {
            time = Some(parsed_time);
            matchup_end = tokens.len() - 1;
        }
    }

    let matchup = tokens[1..matchup_end].join(" ");
    if matchup.is_empty() {
        return None;
    }

    let (home_city, away_city) = match matchup.split_once('/') {
        Some((home, away)) => {
            let h = home.trim();
            let a = away.trim();
            if h.is_empty() {
                return None;
            }
            (h.to_string(), (!a.is_empty()).then(|| a.to_string()))
        }
        None => (matchup, None),
    };

    let audience = detect_audience(category_label.as_deref());

    Some(MatchEntry {
        date,
        home_city,
        away_city,
        time,
        category_label,
        audience,
    })
}

pub fn load_matches_for_date(path: &Path, target_date: NaiveDate) -> Vec<MatchEntry> {
    let Ok(content) = fs::read_to_string(path) else {
        return Vec::new();
    };

    let mut matches = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match parse_match_line(trimmed) {
            Some(entry) if entry.date == target_date => matches.push(entry),
            Some(_) => {}
            None => warn!(
                "Ignoring invalid match line {} in {}: '{}'",
                idx + 1,
                path.display(),
                trimmed
            ),
        }
    }
    matches
}

pub fn match_window_hours(kickoff: Option<NaiveTime>) -> Vec<String> {
    let start = kickoff.map(|t| t.hour()).unwrap_or(14);
    let end = (start + 2).min(23);
    (start..=end).map(|h| format!("{:02}", h)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_match_lines_with_home_city_time_and_category() {
        assert_eq!(parse_match_line("# comment"), None);
        assert_eq!(parse_match_line("   "), None);

        let m1 = parse_match_line("22/11/2026 Nanterre/Grigny 15H - Seniors").unwrap();
        assert_eq!(m1.date, NaiveDate::from_ymd_opt(2026, 11, 22).unwrap());
        assert_eq!(m1.home_city, "Nanterre");
        assert_eq!(m1.away_city.as_deref(), Some("Grigny"));
        assert_eq!(m1.time, NaiveTime::from_hms_opt(15, 0, 0));
        assert_eq!(m1.category_label.as_deref(), Some("Seniors"));
        assert_eq!(m1.audience, Audience::Adults);

        let m2 = parse_match_line("29/11/2026 Grigny/Nanterre 15H - Seniors").unwrap();
        assert_eq!(m2.home_city, "Grigny");
        assert_eq!(m2.away_city.as_deref(), Some("Nanterre"));
        assert_eq!(m2.audience, Audience::Adults);

        let m3 = parse_match_line("13/11/2026 Nanterre/Châtillon - Seniors").unwrap();
        assert_eq!(m3.home_city, "Nanterre");
        assert_eq!(m3.away_city.as_deref(), Some("Châtillon"));
        assert_eq!(m3.time, None);
        assert_eq!(m3.audience, Audience::Adults);

        let m4 = parse_match_line("21/11/2026 Nanterre/Paris 14h30 - Enfants").unwrap();
        assert_eq!(m4.time, NaiveTime::from_hms_opt(14, 30, 0));
        assert_eq!(m4.audience, Audience::Children);
    }

    #[test]
    fn computes_three_hour_match_window() {
        assert_eq!(
            match_window_hours(NaiveTime::from_hms_opt(15, 0, 0)),
            vec!["15", "16", "17"]
        );
        assert_eq!(match_window_hours(None), vec!["14", "15", "16"]);
    }
}
