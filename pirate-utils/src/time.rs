use chrono::{DateTime, FixedOffset, NaiveDateTime, Offset, TimeZone, Utc};
use chrono_tz::Europe::Paris;

/// Returns the `Europe/Paris` fixed UTC offset (`+01:00` CET or `+02:00` CEST) for a UTC timestamp.
pub fn paris_offset_for_utc(utc_dt: DateTime<Utc>) -> FixedOffset {
    utc_dt.with_timezone(&Paris).offset().fix()
}

/// Returns the current date and time in `Europe/Paris` timezone.
pub fn paris_now() -> DateTime<FixedOffset> {
    let now_utc = Utc::now();
    now_utc.with_timezone(&paris_offset_for_utc(now_utc))
}

/// Parses a French local date (`YYYY-MM-DD`) and time (`HH:MM` or `HH:MM:SS`) into `Europe/Paris` `DateTime`.
pub fn parse_paris_datetime(date_str: &str, time_str: &str) -> Option<DateTime<FixedOffset>> {
    if date_str.is_empty() || time_str.is_empty() {
        return None;
    }
    let normalized_time = if time_str.len() == 5 {
        format!("{}:00", time_str)
    } else {
        time_str.to_string()
    };
    let dt_str = format!("{} {}", date_str.trim(), normalized_time.trim());
    let naive = NaiveDateTime::parse_from_str(&dt_str, "%Y-%m-%d %H:%M:%S").ok()?;

    Paris
        .from_local_datetime(&naive)
        .earliest()
        .map(|dt| dt.fixed_offset())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paris_timezone_dst_and_winter() {
        let winter_utc = Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
        assert_eq!(paris_offset_for_utc(winter_utc).local_minus_utc(), 3600);

        let summer_utc = Utc.with_ymd_and_hms(2026, 7, 15, 12, 0, 0).unwrap();
        assert_eq!(paris_offset_for_utc(summer_utc).local_minus_utc(), 7200);

        let parsed = parse_paris_datetime("2026-11-01", "16:30").unwrap();
        assert_eq!(parsed.offset().local_minus_utc(), 3600);
    }
}
