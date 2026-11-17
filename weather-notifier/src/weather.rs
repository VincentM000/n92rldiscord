use chrono::NaiveDate;
use reqwest::blocking::Client;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RainLevel {
    Dry,
    Light,
    Moderate,
    Heavy,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeatherSummary {
    pub temp_min: f64,
    pub temp_max: f64,
    pub total_precip_mm: f64,
    pub max_hourly_precip_mm: f64,
    pub max_precip_prob: u64,
    pub rain_level: RainLevel,
}

#[derive(Debug, Deserialize)]
pub struct ForecastResponse {
    pub hourly: HourlyData,
}

#[derive(Debug, Deserialize)]
pub struct HourlyData {
    pub time: Vec<String>,
    pub temperature_2m: Vec<f64>,
    pub precipitation: Vec<f64>,
    pub precipitation_probability: Vec<u64>,
    pub weather_code: Vec<i64>,
}

#[derive(Debug, Deserialize)]
struct GeocodingResponse {
    results: Option<Vec<GeocodingResult>>,
}

#[derive(Debug, Deserialize)]
struct GeocodingResult {
    name: String,
    latitude: f64,
    longitude: f64,
    country_code: Option<String>,
}

pub fn classify_rain(
    total_precip_mm: f64,
    max_hourly_precip_mm: f64,
    max_precip_prob: u64,
    weather_codes: &[i64],
) -> RainLevel {
    let has_heavy = weather_codes
        .iter()
        .any(|c| matches!(c, 65 | 67 | 81 | 82 | 95 | 96 | 99));
    if max_hourly_precip_mm >= 4.0 || total_precip_mm >= 8.0 || has_heavy {
        return RainLevel::Heavy;
    }

    let has_moderate = weather_codes
        .iter()
        .any(|c| matches!(c, 61 | 63 | 66 | 80));
    if max_hourly_precip_mm >= 1.0 || total_precip_mm >= 2.0 || has_moderate {
        return RainLevel::Moderate;
    }

    let has_drizzle = weather_codes
        .iter()
        .any(|c| matches!(c, 51 | 53 | 55 | 56 | 57));
    if total_precip_mm >= 0.2 || max_precip_prob >= 30 || has_drizzle {
        return RainLevel::Light;
    }

    RainLevel::Dry
}

/// Extracts temperature and rain metrics for the given `hours` on `target_date`.
pub fn summarize_forecast_window(
    forecast: &ForecastResponse,
    target_date: NaiveDate,
    hours: &[String],
) -> Result<WeatherSummary, Box<dyn std::error::Error>> {
    let date_prefix = format!("{}T", target_date.format("%Y-%m-%d"));
    let h = &forecast.hourly;

    let mut temp_min = f64::INFINITY;
    let mut temp_max = f64::NEG_INFINITY;
    let mut total_precip_mm = 0.0;
    let mut max_hourly_precip_mm = 0.0_f64;
    let mut max_precip_prob = 0_u64;
    let mut weather_codes = Vec::new();
    let mut matched_slots = 0;

    for (idx, time_str) in h.time.iter().enumerate() {
        let Some(hour_part) = time_str.strip_prefix(&date_prefix) else {
            continue;
        };
        let hour = &hour_part[..hour_part.len().min(2)];
        if !hours.iter().any(|target| target == hour) {
            continue;
        }

        matched_slots += 1;

        if let Some(&t) = h.temperature_2m.get(idx) {
            temp_min = temp_min.min(t);
            temp_max = temp_max.max(t);
        }
        if let Some(&p) = h.precipitation.get(idx) {
            total_precip_mm += p;
            max_hourly_precip_mm = max_hourly_precip_mm.max(p);
        }
        if let Some(&prob) = h.precipitation_probability.get(idx) {
            max_precip_prob = max_precip_prob.max(prob);
        }
        if let Some(&code) = h.weather_code.get(idx) {
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

pub fn geocode_city(
    client: &Client,
    city: &str,
) -> Result<(String, String, String), Box<dyn std::error::Error>> {
    let url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=5&language=fr&format=json",
        url_encode(city)
    );
    let resp: GeocodingResponse = client.get(&url).send()?.error_for_status()?.json()?;
    let results = resp
        .results
        .filter(|r| !r.is_empty())
        .ok_or_else(|| format!("City '{}' not found via Open-Meteo geocoding", city))?;

    let chosen = results
        .iter()
        .find(|r| r.country_code.as_deref() == Some("FR"))
        .unwrap_or(&results[0]);

    Ok((
        format!("{:.4}", chosen.latitude),
        format!("{:.4}", chosen.longitude),
        chosen.name.clone(),
    ))
}

pub fn fetch_forecast_window(
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
    let forecast: ForecastResponse = client.get(&url).send()?.error_for_status()?.json()?;
    summarize_forecast_window(&forecast, target_date, hours)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn summarizes_training_window_and_detects_heavy_rain() {
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

        let forecast: ForecastResponse = serde_json::from_value(sample).unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();
        let hours = ["18", "19", "20", "21"].map(str::to_string);
        let summary = summarize_forecast_window(&forecast, date, &hours).unwrap();

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
}

