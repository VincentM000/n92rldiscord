use reqwest::blocking::Client;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) NanterrePirates-Scraper/1.0";

/// Builds a pre-configured blocking HTTP client with standard User-Agent and timeout.
pub fn build_http_client() -> Client {
    Client::builder()
        .user_agent(DEFAULT_USER_AGENT)
        .timeout(Duration::from_secs(20))
        .build()
        .expect("Failed to build HTTP client")
}

/// Applies a pseudo-random jitter in the range [-20s, +20s] to avoid synchronized request spikes.
pub fn apply_jitter(base_secs: u64) -> Duration {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let jitter = (nanos % 41) as i64 - 20;
    let total = (base_secs as i64 + jitter).max(10) as u64;
    Duration::from_secs(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_jitter_bounds() {
        let d = apply_jitter(600);
        assert!(d.as_secs() >= 580 && d.as_secs() <= 620);

        let small = apply_jitter(5);
        assert!(small.as_secs() >= 10);
    }

    #[test]
    fn test_build_http_client() {
        let _client = build_http_client();
    }
}
