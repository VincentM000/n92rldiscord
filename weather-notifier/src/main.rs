use chrono::{Datelike, Local, NaiveDate, NaiveTime, Timelike, Weekday};
use dotenvy::dotenv;
use log::{info, warn};
use pirate_utils::send_discord_webhook_as;
use reqwest::blocking::Client;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::Path;

const DEFAULT_LAT: &str = "48.8924";
const DEFAULT_LON: &str = "2.2069";
const DEFAULT_LOCATION: &str = "Nanterre";
const DEFAULT_BOT_NAME: &str = "Nami";
const DEFAULT_MATCH_FILE: &str = "matches.txt";
const TRAINING_HOURS: [&str; 4] = ["18", "19", "20", "21"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RainLevel {
    Dry,
    Light,
    Moderate,
    Heavy,
}

#[derive(Debug, Clone, PartialEq)]
struct WeatherSummary {
    temp_min: f64,
    temp_max: f64,
    total_precip_mm: f64,
    max_hourly_precip_mm: f64,
    max_precip_prob: u64,
    rain_level: RainLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Audience {
    Adults,
    Children,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MatchEntry {
    date: NaiveDate,
    home_city: String,
    away_city: Option<String>,
    time: Option<NaiveTime>,
    category_label: Option<String>,
    audience: Audience,
}

#[derive(Debug, Clone, Copy)]
struct CliOptions {
    dry_run: bool,
    today: bool,
    force: bool,
}

impl CliOptions {
    fn from_args<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut opts = Self {
            dry_run: false,
            today: false,
            force: false,
        };
        for arg in args {
            match arg.as_ref() {
                "--dry-run" => opts.dry_run = true,
                "--today" => opts.today = true,
                "--force" => opts.force = true,
                _ => {}
            }
        }
        opts
    }
}

fn is_training_day(weekday: Weekday) -> bool {
    matches!(weekday, Weekday::Tue | Weekday::Wed | Weekday::Thu)
}

fn french_weekday(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Mon => "Lundi",
        Weekday::Tue => "Mardi",
        Weekday::Wed => "Mercredi",
        Weekday::Thu => "Jeudi",
        Weekday::Fri => "Vendredi",
        Weekday::Sat => "Samedi",
        Weekday::Sun => "Dimanche",
    }
}

fn french_month(month: u32) -> &'static str {
    match month {
        1 => "janvier",
        2 => "février",
        3 => "mars",
        4 => "avril",
        5 => "mai",
        6 => "juin",
        7 => "juillet",
        8 => "août",
        9 => "septembre",
        10 => "octobre",
        11 => "novembre",
        12 => "décembre",
        _ => "",
    }
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

/// Parses a match line such as:
/// - `22/11/2026 Nanterre/Grigny 15H - Seniors`
/// - `13/11/2026 Nanterre/Châtillon - Seniors`
/// - `22/11/2026;15:00;Lyon` (legacy semicolon format)
fn parse_match_line(line: &str) -> Option<MatchEntry> {
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
            let category_label = parts.get(3).map(|s| (*s).to_string()).filter(|s| !s.is_empty());
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

fn load_matches_for_date(path: &Path, target_date: NaiveDate) -> Vec<MatchEntry> {
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

fn classify_rain(
    total_precip_mm: f64,
    max_hourly_precip_mm: f64,
    max_precip_prob: u64,
    weather_codes: &[i64],
) -> RainLevel {
    let has_heavy_code = weather_codes
        .iter()
        .any(|c| matches!(c, 65 | 67 | 81 | 82 | 95 | 96 | 99));
    if max_hourly_precip_mm >= 4.0 || total_precip_mm >= 8.0 || has_heavy_code {
        return RainLevel::Heavy;
    }

    let has_moderate_code = weather_codes
        .iter()
        .any(|c| matches!(c, 61 | 63 | 66 | 80));
    if max_hourly_precip_mm >= 1.0 || total_precip_mm >= 2.0 || has_moderate_code {
        return RainLevel::Moderate;
    }

    let has_drizzle_code = weather_codes
        .iter()
        .any(|c| matches!(c, 51 | 53 | 55 | 56 | 57));
    if total_precip_mm >= 0.2 || max_precip_prob >= 30 || has_drizzle_code {
        return RainLevel::Light;
    }

    RainLevel::Dry
}

/// Extracts temperature and rain metrics for the given `hours` on `target_date`.
fn parse_forecast_window(
    json: &Value,
    target_date: NaiveDate,
    hours: &[String],
) -> Result<WeatherSummary, Box<dyn std::error::Error>> {
    let hourly = json
        .get("hourly")
        .ok_or("Missing 'hourly' field in Open-Meteo response")?;

    let times = hourly
        .get("time")
        .and_then(|v| v.as_array())
        .ok_or("Missing 'hourly.time' array")?;
    let temps = hourly
        .get("temperature_2m")
        .and_then(|v| v.as_array())
        .ok_or("Missing 'hourly.temperature_2m' array")?;
    let precips = hourly
        .get("precipitation")
        .and_then(|v| v.as_array())
        .ok_or("Missing 'hourly.precipitation' array")?;
    let probs = hourly
        .get("precipitation_probability")
        .and_then(|v| v.as_array())
        .ok_or("Missing 'hourly.precipitation_probability' array")?;
    let codes = hourly
        .get("weather_code")
        .and_then(|v| v.as_array())
        .ok_or("Missing 'hourly.weather_code' array")?;

    let date_prefix = format!("{}T", target_date.format("%Y-%m-%d"));

    let mut temp_min = f64::INFINITY;
    let mut temp_max = f64::NEG_INFINITY;
    let mut total_precip_mm = 0.0;
    let mut max_hourly_precip_mm = 0.0_f64;
    let mut max_precip_prob = 0_u64;
    let mut weather_codes = Vec::new();
    let mut matched_slots = 0;

    for (idx, time_val) in times.iter().enumerate() {
        let Some(time_str) = time_val.as_str() else {
            continue;
        };
        let Some(hour_part) = time_str.strip_prefix(&date_prefix) else {
            continue;
        };
        let hour = &hour_part[..hour_part.len().min(2)];
        if !hours.iter().any(|h| h == hour) {
            continue;
        }

        matched_slots += 1;

        if let Some(t) = temps.get(idx).and_then(|v| v.as_f64()) {
            temp_min = temp_min.min(t);
            temp_max = temp_max.max(t);
        }
        if let Some(p) = precips.get(idx).and_then(|v| v.as_f64()) {
            total_precip_mm += p;
            max_hourly_precip_mm = max_hourly_precip_mm.max(p);
        }
        if let Some(prob) = probs.get(idx).and_then(|v| v.as_u64()) {
            max_precip_prob = max_precip_prob.max(prob);
        }
        if let Some(code) = codes.get(idx).and_then(|v| v.as_i64()) {
            weather_codes.push(code);
        }
    }

    if matched_slots == 0 || !temp_min.is_finite() {
        return Err(format!("No forecast data found for {}", target_date).into());
    }

    let rain_level = classify_rain(
        total_precip_mm,
        max_hourly_precip_mm,
        max_precip_prob,
        &weather_codes,
    );

    Ok(WeatherSummary {
        temp_min,
        temp_max,
        total_precip_mm,
        max_hourly_precip_mm,
        max_precip_prob,
        rain_level,
    })
}

fn format_weather_body(summary: &WeatherSummary, audience: Audience, dry_tip: &str) -> String {
    let min_round = summary.temp_min.round() as i64;
    let max_round = summary.temp_max.round() as i64;
    let temp_line = if min_round == max_round {
        format!("🌡️ **Température :** ~{}°C", min_round)
    } else {
        format!(
            "🌡️ **Température :** entre {}°C et {}°C",
            min_round, max_round
        )
    };

    let prefix = match audience {
        Audience::Children => "Préparez le sac",
        Audience::Adults => "Préparez vos affaires",
    };

    let (rain_line, tip_line) = match summary.rain_level {
        RainLevel::Heavy => (
            format!(
                "🌧️ **Pluie :** **Oui, il va BEAUCOUP pleuvoir !** (cumul ~{:.1} mm, jusqu'à {:.1} mm/h, risque {}%)",
                summary.total_precip_mm, summary.max_hourly_precip_mm, summary.max_precip_prob
            ),
            format!("🎒 *{} : K-way / imperméable obligatoire et affaires de rechange !*", prefix),
        ),
        RainLevel::Moderate => (
            format!(
                "🌦️ **Pluie :** **Oui, il va pleuvoir** (pluie modérée, cumul ~{:.1} mm, jusqu'à {:.1} mm/h, risque {}%)",
                summary.total_precip_mm, summary.max_hourly_precip_mm, summary.max_precip_prob
            ),
            format!("🎒 *{} : pensez au K-way / coupe-vent imperméable !*", prefix),
        ),
        RainLevel::Light => (
            format!(
                "☁️ **Pluie :** Petite pluie possible (risque {}%, ~{:.1} mm)",
                summary.max_precip_prob, summary.total_precip_mm
            ),
            format!("🎒 *{} : un coupe-vent dans le sac par précaution.*", prefix),
        ),
        RainLevel::Dry => {
            let tip = if summary.temp_min <= 8.0 {
                "🎒 *Pas de pluie prévue, mais il va faire frais : pensez aux couches chaudes !*".to_string()
            } else {
                dry_tip.to_string()
            };
            (
                format!(
                    "☀️ **Pluie :** Non, pas de pluie prévue (risque {}%)",
                    summary.max_precip_prob
                ),
                tip,
            )
        }
    };

    format!("{}\n{}\n\n{}", temp_line, rain_line, tip_line)
}

fn format_training_message(
    target_date: NaiveDate,
    is_today: bool,
    location: &str,
    summary: &WeatherSummary,
) -> String {
    let day_label = if is_today { "d'aujourd'hui" } else { "de demain" };
    let date_label = format!(
        "{} {} {}",
        french_weekday(target_date.weekday()),
        target_date.day(),
        french_month(target_date.month())
    );
    let body = format_weather_body(
        summary,
        Audience::Children,
        "🎒 *Tout bon pour l'entraînement au sec !*",
    );

    format!(
        "🏴‍☠️ **Météo de l'entraînement {} — {} (18h30 - 21h30)** 📍 *{}*\n\n{}",
        day_label, date_label, location, body
    )
}

fn format_match_message(
    match_entry: &MatchEntry,
    is_today: bool,
    resolved_city: &str,
    summary: &WeatherSummary,
) -> String {
    let day_label = if is_today { "d'aujourd'hui" } else { "de demain" };
    let date_label = format!(
        "{} {} {}",
        french_weekday(match_entry.date.weekday()),
        match_entry.date.day(),
        french_month(match_entry.date.month())
    );
    let cat_part = match &match_entry.category_label {
        Some(cat) => format!(" ({})", cat),
        None => String::new(),
    };
    let matchup_part = match &match_entry.away_city {
        Some(away) => format!("{} vs {}", match_entry.home_city, away),
        None => match_entry.home_city.clone(),
    };
    let time_part = match match_entry.time {
        Some(t) => format!("à {:02}h{:02}", t.hour(), t.minute()),
        None => "après-midi (horaire non précisé)".to_string(),
    };

    let body = format_weather_body(
        summary,
        match_entry.audience,
        "🎒 *Tout bon pour le match au sec !*",
    );

    format!(
        "🏉 **Météo du match{} {} — {} ({}, {})** 📍 *{}*\n\n{}",
        cat_part, day_label, date_label, matchup_part, time_part, resolved_city, body
    )
}

fn url_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}

fn geocode_city(
    client: &Client,
    city: &str,
) -> Result<(String, String, String), Box<dyn std::error::Error>> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=5&language=fr&format=json",
        url_encode(city)
    );
    let resp: Value = client.get(&url).send()?.error_for_status()?.json()?;

    let results = resp
        .get("results")
        .and_then(|v| v.as_array())
        .filter(|arr| !arr.is_empty())
        .ok_or_else(|| format!("City '{}' not found via Open-Meteo geocoding", city))?;

    let chosen = results
        .iter()
        .find(|r| r.get("country_code").and_then(|c| c.as_str()) == Some("FR"))
        .unwrap_or(&results[0]);

    let lat = chosen
        .get("latitude")
        .and_then(|v| v.as_f64())
        .ok_or("Missing latitude in geocoding response")?;
    let lon = chosen
        .get("longitude")
        .and_then(|v| v.as_f64())
        .ok_or("Missing longitude in geocoding response")?;
    let name = chosen
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(city)
        .to_string();

    Ok((format!("{:.4}", lat), format!("{:.4}", lon), name))
}

fn fetch_forecast_window(
    client: &Client,
    lat: &str,
    lon: &str,
    target_date: NaiveDate,
    hours: &[String],
) -> Result<WeatherSummary, Box<dyn std::error::Error>> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&hourly=temperature_2m,precipitation,precipitation_probability,weather_code&timezone=Europe%2FParis&forecast_days=2",
        lat, lon
    );
    let json: Value = client.get(&url).send()?.error_for_status()?.json()?;
    parse_forecast_window(&json, target_date, hours)
}

fn match_window_hours(kickoff: Option<NaiveTime>) -> Vec<String> {
    let start = kickoff.map(|t| t.hour()).unwrap_or(14);
    let end = (start + 2).min(23);
    (start..=end).map(|h| format!("{:02}", h)).collect()
}

fn resolve_webhook_url() -> Result<String, Box<dyn std::error::Error>> {
    for var_name in ["DISCORD_NAMI_WEBHOOK_URL", "DISCORD_GENERAL_WEBHOOK_URL"] {
        if let Ok(url) = env::var(var_name) {
            if !url.trim().is_empty() {
                return Ok(url);
            }
        }
    }
    Err("DISCORD_NAMI_WEBHOOK_URL is not set".into())
}

fn dispatch_message(
    client: &Client,
    opts: CliOptions,
    bot_name: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        println!("{}", message);
    } else {
        let webhook_url = resolve_webhook_url()?;
        send_discord_webhook_as(client, &webhook_url, bot_name, message)?;
    }
    Ok(())
}

fn run(opts: CliOptions) -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let today = Local::now().date_naive();
    let target_date = if opts.today {
        today
    } else {
        today.succ_opt().ok_or("Failed to compute tomorrow's date")?
    };

    let match_file =
        env::var("MATCH_SCHEDULE_FILE").unwrap_or_else(|_| DEFAULT_MATCH_FILE.to_string());
    let matches_today = load_matches_for_date(Path::new(&match_file), target_date);
    let run_training = opts.force || is_training_day(target_date.weekday());

    if !run_training && matches_today.is_empty() {
        info!(
            "Target date {} ({}) has neither training nor scheduled match. Skipping.",
            target_date,
            french_weekday(target_date.weekday())
        );
        return Ok(());
    }

    let bot_name = env::var("WEATHER_BOT_NAME").unwrap_or_else(|_| DEFAULT_BOT_NAME.to_string());
    let default_lat = env::var("WEATHER_LAT").unwrap_or_else(|_| DEFAULT_LAT.to_string());
    let default_lon = env::var("WEATHER_LON").unwrap_or_else(|_| DEFAULT_LON.to_string());
    let default_location =
        env::var("WEATHER_LOCATION_NAME").unwrap_or_else(|_| DEFAULT_LOCATION.to_string());

    let client = Client::builder()
        .user_agent("NanterrePirates-Weather/1.0")
        .build()?;

    if run_training {
        let hours: Vec<String> = TRAINING_HOURS.iter().map(|h| (*h).to_string()).collect();
        let summary =
            fetch_forecast_window(&client, &default_lat, &default_lon, target_date, &hours)?;
        let message = format_training_message(target_date, opts.today, &default_location, &summary);
        dispatch_message(&client, opts, &bot_name, &message)?;
        info!("Training weather sent as {} for {}", bot_name, target_date);
    }

    for entry in matches_today {
        let coords = if entry.home_city.eq_ignore_ascii_case(&default_location) {
            Ok((
                default_lat.clone(),
                default_lon.clone(),
                default_location.clone(),
            ))
        } else {
            geocode_city(&client, &entry.home_city)
        };

        match coords {
            Ok((lat, lon, resolved_city)) => {
                let hours = match_window_hours(entry.time);
                match fetch_forecast_window(&client, &lat, &lon, entry.date, &hours) {
                    Ok(summary) => {
                        let message =
                            format_match_message(&entry, opts.today, &resolved_city, &summary);
                        dispatch_message(&client, opts, &bot_name, &message)?;
                        info!(
                            "Match weather sent as {} for {} in {}",
                            bot_name, entry.date, resolved_city
                        );
                    }
                    Err(e) => warn!(
                        "Failed to fetch match weather for {}: {}",
                        entry.home_city, e
                    ),
                }
            }
            Err(e) => warn!("Failed to geocode match city '{}': {}", entry.home_city, e),
        }
    }

    Ok(())
}

fn main() {
    env_logger::init();
    let opts = CliOptions::from_args(env::args().skip(1));

    if let Err(e) = run(opts) {
        warn!("Weather notifier failed: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn training_days_are_tue_to_thu() {
        assert!(!is_training_day(Weekday::Mon));
        assert!(is_training_day(Weekday::Tue));
        assert!(is_training_day(Weekday::Wed));
        assert!(is_training_day(Weekday::Thu));
        assert!(!is_training_day(Weekday::Fri));
        assert!(!is_training_day(Weekday::Sat));
        assert!(!is_training_day(Weekday::Sun));
    }

    #[test]
    fn parses_training_window_and_detects_heavy_rain() {
        let sample = json!({
            "hourly": {
                "time": [
                    "2026-11-18T17:00",
                    "2026-11-18T18:00",
                    "2026-11-18T19:00",
                    "2026-11-18T20:00",
                    "2026-11-18T21:00",
                    "2026-11-18T22:00"
                ],
                "temperature_2m": [15.0, 10.2, 9.4, 8.6, 8.1, 7.0],
                "precipitation": [0.0, 1.5, 4.5, 3.0, 1.4, 0.0],
                "precipitation_probability": [10, 60, 90, 85, 70, 20],
                "weather_code": [3, 61, 65, 63, 61, 3]
            }
        });

        let date = NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();
        let hours: Vec<String> = TRAINING_HOURS.iter().map(|h| (*h).to_string()).collect();
        let summary = parse_forecast_window(&sample, date, &hours).unwrap();

        assert!((summary.temp_min - 8.1).abs() < 1e-6);
        assert!((summary.temp_max - 10.2).abs() < 1e-6);
        assert!((summary.total_precip_mm - 10.4).abs() < 1e-6);
        assert!((summary.max_hourly_precip_mm - 4.5).abs() < 1e-6);
        assert_eq!(summary.max_precip_prob, 90);
        assert_eq!(summary.rain_level, RainLevel::Heavy);
    }

    #[test]
    fn classifies_dry_moderate_and_light_rain() {
        assert_eq!(classify_rain(0.0, 0.0, 10, &[1, 2, 3]), RainLevel::Dry);
        assert_eq!(classify_rain(0.5, 0.3, 40, &[51]), RainLevel::Light);
        assert_eq!(classify_rain(3.2, 1.4, 75, &[61]), RainLevel::Moderate);
        assert_eq!(classify_rain(9.0, 3.0, 85, &[63]), RainLevel::Heavy);
    }

    #[test]
    fn formats_french_message_for_tomorrow() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();
        let summary = WeatherSummary {
            temp_min: 6.2,
            temp_max: 7.4,
            total_precip_mm: 0.0,
            max_hourly_precip_mm: 0.0,
            max_precip_prob: 10,
            rain_level: RainLevel::Dry,
        };
        let msg = format_training_message(date, false, "Nanterre", &summary);
        assert!(msg.contains("Mercredi 18 novembre (18h30 - 21h30)"));
        assert!(msg.contains("entre 6°C et 7°C"));
        assert!(msg.contains("Non, pas de pluie prévue"));
        assert!(msg.contains("couches chaudes"));
    }

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
    fn formats_match_message_for_adults_and_children() {
        let entry = MatchEntry {
            date: NaiveDate::from_ymd_opt(2026, 11, 29).unwrap(),
            home_city: "Grigny".to_string(),
            away_city: Some("Nanterre".to_string()),
            time: Some(NaiveTime::from_hms_opt(15, 0, 0).unwrap()),
            category_label: Some("Seniors".to_string()),
            audience: Audience::Adults,
        };
        assert_eq!(match_window_hours(entry.time), vec!["15", "16", "17"]);
        assert_eq!(match_window_hours(None), vec!["14", "15", "16"]);

        let summary = WeatherSummary {
            temp_min: 11.0,
            temp_max: 12.0,
            total_precip_mm: 3.0,
            max_hourly_precip_mm: 1.5,
            max_precip_prob: 75,
            rain_level: RainLevel::Moderate,
        };
        let msg = format_match_message(&entry, false, "Grigny", &summary);
        assert!(msg.contains("Météo du match (Seniors) de demain"));
        assert!(msg.contains("Dimanche 29 novembre (Grigny vs Nanterre, à 15h00)"));
        assert!(msg.contains("📍 *Grigny*"));
        assert!(msg.contains("Préparez vos affaires"));
    }
}
