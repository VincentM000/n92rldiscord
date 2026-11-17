use chrono::{Datelike, Local, NaiveDate, Weekday};
use dotenvy::dotenv;
use log::{info, warn};
use pirate_utils::send_discord_webhook_as;
use reqwest::blocking::Client;
use serde_json::Value;
use std::env;

const DEFAULT_LAT: &str = "48.8924";
const DEFAULT_LON: &str = "2.2069";
const DEFAULT_LOCATION: &str = "Nanterre";
const DEFAULT_BOT_NAME: &str = "Nami";
const TRAINING_HOURS: [&str; 4] = ["18", "19", "20", "21"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RainLevel {
    Dry,
    Light,
    Moderate,
    Heavy,
}

#[derive(Debug, Clone, PartialEq)]
struct TrainingForecast {
    temp_min: f64,
    temp_max: f64,
    total_precip_mm: f64,
    max_hourly_precip_mm: f64,
    max_precip_prob: u64,
    rain_level: RainLevel,
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

/// Extracts temperature and rain metrics for the 18h30-21h30 training window on `target_date`.
fn parse_training_forecast(
    json: &Value,
    target_date: NaiveDate,
) -> Result<TrainingForecast, Box<dyn std::error::Error>> {
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
        if !TRAINING_HOURS.contains(&hour) {
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

    Ok(TrainingForecast {
        temp_min,
        temp_max,
        total_precip_mm,
        max_hourly_precip_mm,
        max_precip_prob,
        rain_level,
    })
}

fn format_discord_message(
    target_date: NaiveDate,
    is_today: bool,
    location: &str,
    forecast: &TrainingForecast,
) -> String {
    let day_label = if is_today { "d'aujourd'hui" } else { "de demain" };
    let date_label = format!(
        "{} {} {}",
        french_weekday(target_date.weekday()),
        target_date.day(),
        french_month(target_date.month())
    );

    let min_round = forecast.temp_min.round() as i64;
    let max_round = forecast.temp_max.round() as i64;
    let temp_line = if min_round == max_round {
        format!("🌡️ **Température :** ~{}°C", min_round)
    } else {
        format!(
            "🌡️ **Température :** entre {}°C et {}°C",
            min_round, max_round
        )
    };

    let (rain_line, tip_line) = match forecast.rain_level {
        RainLevel::Heavy => (
            format!(
                "🌧️ **Pluie :** **Oui, il va BEAUCOUP pleuvoir !** (cumul ~{:.1} mm, jusqu'à {:.1} mm/h, risque {}%)",
                forecast.total_precip_mm, forecast.max_hourly_precip_mm, forecast.max_precip_prob
            ),
            "🎒 *Préparez le sac : K-way / imperméable obligatoire et affaires de rechange !*",
        ),
        RainLevel::Moderate => (
            format!(
                "🌦️ **Pluie :** **Oui, il va pleuvoir** (pluie modérée, cumul ~{:.1} mm, jusqu'à {:.1} mm/h, risque {}%)",
                forecast.total_precip_mm, forecast.max_hourly_precip_mm, forecast.max_precip_prob
            ),
            "🎒 *Préparez le sac : pensez au K-way / coupe-vent imperméable !*",
        ),
        RainLevel::Light => (
            format!(
                "☁️ **Pluie :** Petite pluie possible (risque {}%, ~{:.1} mm)",
                forecast.max_precip_prob, forecast.total_precip_mm
            ),
            "🎒 *Préparez le sac : un coupe-vent dans le sac par précaution.*",
        ),
        RainLevel::Dry => {
            let tip = if forecast.temp_min <= 8.0 {
                "🎒 *Pas de pluie prévue, mais il va faire frais : pensez aux couches chaudes !*"
            } else {
                "🎒 *Tout bon pour l'entraînement au sec !*"
            };
            (
                format!(
                    "☀️ **Pluie :** Non, pas de pluie prévue (risque {}%)",
                    forecast.max_precip_prob
                ),
                tip,
            )
        }
    };

    format!(
        "🏴‍☠️ **Météo de l'entraînement {} — {} (18h30 - 21h30)** 📍 *{}*\n\n{}\n{}\n\n{}",
        day_label, date_label, location, temp_line, rain_line, tip_line
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

fn run(opts: CliOptions) -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let today = Local::now().date_naive();
    let target_date = if opts.today {
        today
    } else {
        today.succ_opt().ok_or("Failed to compute tomorrow's date")?
    };

    if !opts.force && !is_training_day(target_date.weekday()) {
        info!(
            "Target date {} ({}) is not a training day (Tue-Thu). Skipping.",
            target_date,
            french_weekday(target_date.weekday())
        );
        return Ok(());
    }

    let lat = env::var("WEATHER_LAT").unwrap_or_else(|_| DEFAULT_LAT.to_string());
    let lon = env::var("WEATHER_LON").unwrap_or_else(|_| DEFAULT_LON.to_string());
    let location = env::var("WEATHER_LOCATION_NAME").unwrap_or_else(|_| DEFAULT_LOCATION.to_string());
    let bot_name = env::var("WEATHER_BOT_NAME").unwrap_or_else(|_| DEFAULT_BOT_NAME.to_string());

    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&hourly=temperature_2m,precipitation,precipitation_probability,weather_code&timezone=Europe%2FParis&forecast_days=2",
        lat, lon
    );

    let client = Client::builder()
        .user_agent("NanterrePirates-Weather/1.0")
        .build()?;

    let json: Value = client.get(&url).send()?.error_for_status()?.json()?;
    let forecast = parse_training_forecast(&json, target_date)?;
    let message = format_discord_message(target_date, opts.today, &location, &forecast);

    if opts.dry_run {
        println!("{}", message);
    } else {
        let webhook_url = resolve_webhook_url()?;
        send_discord_webhook_as(&client, &webhook_url, &bot_name, &message)?;
        info!("Weather forecast sent to #general as {} for {}", bot_name, target_date);
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
        let forecast = parse_training_forecast(&sample, date).unwrap();

        assert!((forecast.temp_min - 8.1).abs() < 1e-6);
        assert!((forecast.temp_max - 10.2).abs() < 1e-6);
        assert!((forecast.total_precip_mm - 10.4).abs() < 1e-6);
        assert!((forecast.max_hourly_precip_mm - 4.5).abs() < 1e-6);
        assert_eq!(forecast.max_precip_prob, 90);
        assert_eq!(forecast.rain_level, RainLevel::Heavy);
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
        let forecast = TrainingForecast {
            temp_min: 6.2,
            temp_max: 7.4,
            total_precip_mm: 0.0,
            max_hourly_precip_mm: 0.0,
            max_precip_prob: 10,
            rain_level: RainLevel::Dry,
        };
        let msg = format_discord_message(date, false, "Nanterre", &forecast);
        assert!(msg.contains("Mercredi 18 novembre (18h30 - 21h30)"));
        assert!(msg.contains("entre 6°C et 7°C"));
        assert!(msg.contains("Non, pas de pluie prévue"));
        assert!(msg.contains("couches chaudes"));
    }
}

