mod matches;
mod weather;

use chrono::{Datelike, NaiveDate, Timelike, Weekday};
use dotenvy::dotenv;
use log::{info, warn};
use matches::{load_matches_for_date, match_window_hours, Audience, MatchEntry};
use pirate_utils::{build_http_client, paris_now, send_discord_webhook_as};
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use weather::{fetch_forecast_window, geocode_city, RainLevel, WeatherSummary};

const DEFAULT_LAT: &str = "48.8924";
const DEFAULT_LON: &str = "2.2069";
const DEFAULT_LOCATION: &str = "Nanterre";
const DEFAULT_BOT_NAME: &str = "Nami";
const DEFAULT_MATCH_FILE: &str = "matches.txt";
const TRAINING_HOURS: [&str; 4] = ["18", "19", "20", "21"];

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

fn format_french_date(date: NaiveDate) -> String {
    let day = match date.weekday() {
        Weekday::Mon => "Lundi",
        Weekday::Tue => "Mardi",
        Weekday::Wed => "Mercredi",
        Weekday::Thu => "Jeudi",
        Weekday::Fri => "Vendredi",
        Weekday::Sat => "Samedi",
        Weekday::Sun => "Dimanche",
    };
    let month = match date.month() {
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
    };
    format!("{} {} {}", day, date.day(), month)
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
                "🎒 *Pas de pluie prévue, mais il va faire frais : pensez aux couches chaudes !*"
                    .to_string()
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
    let body = format_weather_body(
        summary,
        Audience::Children,
        "🎒 *Tout bon pour l'entraînement au sec !*",
    );

    format!(
        "🏴‍☠️ **Météo de l'entraînement {} — {} (18h30 - 21h30)** 📍 *{}*\n\n{}",
        day_label,
        format_french_date(target_date),
        location,
        body
    )
}

fn format_match_message(
    entry: &MatchEntry,
    is_today: bool,
    resolved_city: &str,
    summary: &WeatherSummary,
) -> String {
    let day_label = if is_today { "d'aujourd'hui" } else { "de demain" };
    let cat_part = entry
        .category_label
        .as_deref()
        .map(|c| format!(" ({})", c))
        .unwrap_or_default();
    let matchup_part = match &entry.away_city {
        Some(away) => format!("{} vs {}", entry.home_city, away),
        None => entry.home_city.clone(),
    };
    let time_part = match entry.time {
        Some(t) => format!("à {:02}h{:02}", t.hour(), t.minute()),
        None => "après-midi (horaire non précisé)".to_string(),
    };
    let body = format_weather_body(
        summary,
        entry.audience,
        "🎒 *Tout bon pour le match au sec !*",
    );

    format!(
        "🏉 **Météo du match{} {} — {} ({}, {})** 📍 *{}*\n\n{}",
        cat_part,
        day_label,
        format_french_date(entry.date),
        matchup_part,
        time_part,
        resolved_city,
        body
    )
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

    let today = paris_now().date_naive();
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
            "Target date {} has neither training nor scheduled match. Skipping.",
            target_date
        );
        return Ok(());
    }

    let bot_name = env::var("WEATHER_BOT_NAME").unwrap_or_else(|_| DEFAULT_BOT_NAME.to_string());
    let default_lat = env::var("WEATHER_LAT").unwrap_or_else(|_| DEFAULT_LAT.to_string());
    let default_lon = env::var("WEATHER_LON").unwrap_or_else(|_| DEFAULT_LON.to_string());
    let default_location =
        env::var("WEATHER_LOCATION_NAME").unwrap_or_else(|_| DEFAULT_LOCATION.to_string());

    let client = build_http_client();

    if run_training {
        let hours = TRAINING_HOURS.map(str::to_string);
        let summary =
            fetch_forecast_window(&client, &default_lat, &default_lon, target_date, &hours)?;
        let message = format_training_message(target_date, opts.today, &default_location, &summary);
        dispatch_message(&client, opts, &bot_name, &message)?;
        info!("Training weather sent as {} for {}", bot_name, target_date);
    }

    let mut geocode_cache: HashMap<String, (String, String, String)> = HashMap::new();
    geocode_cache.insert(
        default_location.to_ascii_lowercase(),
        (default_lat, default_lon, default_location),
    );

    for entry in matches_today {
        let key = entry.home_city.to_ascii_lowercase();
        let coords = if let Some(cached) = geocode_cache.get(&key) {
            Ok(cached.clone())
        } else {
            geocode_city(&client, &entry.home_city).map(|res| {
                geocode_cache.insert(key, res.clone());
                res
            })
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
    use chrono::NaiveTime;

    #[test]
    fn training_days_are_tue_to_thu() {
        assert!(!is_training_day(Weekday::Mon));
        assert!(is_training_day(Weekday::Tue));
        assert!(is_training_day(Weekday::Wed));
        assert!(is_training_day(Weekday::Thu));
        assert!(!is_training_day(Weekday::Fri));
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
    fn formats_match_message_for_adults() {
        let entry = MatchEntry {
            date: NaiveDate::from_ymd_opt(2026, 11, 29).unwrap(),
            home_city: "Grigny".to_string(),
            away_city: Some("Nanterre".to_string()),
            time: Some(NaiveTime::from_hms_opt(15, 0, 0).unwrap()),
            category_label: Some("Seniors".to_string()),
            audience: Audience::Adults,
        };
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
