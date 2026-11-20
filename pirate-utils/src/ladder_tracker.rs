use crate::discord::send_discord_alert;
use crate::domain::extract_total_league_points;
use crate::http::{apply_jitter, build_http_client};
use crate::state::{load_state, save_state};
use reqwest::blocking::Client;
use std::thread;
use std::time::Duration;

pub const MATCH_DELIMITER: &str = "|||";
pub const DEFAULT_POLL_SECS: u64 = 600;

/// Port interface implemented by any league standings & match results tracker.
pub trait LadderTracker {
    fn name(&self) -> &str;
    fn state_filename(&self) -> &str;
    fn match_state_filename(&self) -> &str;
    fn fetch_latest_match_result(
        &self,
        client: &Client,
    ) -> Result<Option<String>, Box<dyn std::error::Error>>;
    fn fetch_ladder(&self, client: &Client) -> Result<String, Box<dyn std::error::Error>>;

    /// Optional hook executed on each polling cycle (e.g. YouTube live/replay checks, adaptive sleep).
    fn on_poll_cycle(
        &mut self,
        _client: &Client,
    ) -> Result<Option<Duration>, Box<dyn std::error::Error>> {
        Ok(None)
    }
}

/// Executes a single tracking cycle and returns the duration to sleep before the next cycle.
pub fn run_tracker_step<T: LadderTracker>(tracker: &mut T, client: &Client) -> Duration {
    let custom_sleep = match tracker.on_poll_cycle(client) {
        Ok(duration) => duration,
        Err(e) => {
            log::warn!("[{}] Error in auxiliary poll cycle: {}", tracker.name(), e);
            None
        }
    };

    let sleep_duration = custom_sleep.unwrap_or_else(|| apply_jitter(DEFAULT_POLL_SECS));

    match tracker.fetch_ladder(client) {
        Ok(current_ladder) => {
            let last_ladder = load_state(tracker.state_filename());

            if current_ladder != last_ladder && !current_ladder.is_empty() {
                if !last_ladder.is_empty() {
                    let current_pts = extract_total_league_points(&current_ladder);
                    let last_pts = extract_total_league_points(&last_ladder);

                    if current_pts < last_pts {
                        log::warn!(
                            "[{}] Administrative point deduction detected!",
                            tracker.name()
                        );
                        let penalty_msg = "⚖️ **DÉCISION ADMINISTRATIVE :**\nUn retrait de points a été appliqué au classement !";
                        let _ = send_discord_alert(client, penalty_msg);
                        thread::sleep(Duration::from_secs(2));
                    } else {
                        let current_match =
                            tracker.fetch_latest_match_result(client).unwrap_or(None);
                        let last_match = load_state(tracker.match_state_filename());

                        if let Some(ref match_msg) = current_match {
                            if match_msg == &last_match && !last_match.is_empty() {
                                log::info!(
                                    "[{}] ⏳ Classement modifié, mais aucun nouveau score détecté. On patiente...",
                                    tracker.name()
                                );
                                return sleep_duration;
                            }

                            let mut has_new_match = false;

                            for single_match in match_msg.split(MATCH_DELIMITER) {
                                if !single_match.is_empty() && !last_match.contains(single_match) {
                                    has_new_match = true;
                                    let _ = send_discord_alert(client, single_match);
                                    thread::sleep(Duration::from_secs(2));
                                }
                            }

                            if !has_new_match && !last_match.is_empty() {
                                return sleep_duration;
                            }

                            let _ = save_state(tracker.match_state_filename(), match_msg);
                        }
                    }
                } else {
                    // First run baseline for match state as well
                    if let Ok(Some(initial_matches)) = tracker.fetch_latest_match_result(client) {
                        let _ = save_state(tracker.match_state_filename(), &initial_matches);
                    }
                }

                log::info!("[{}] Envoi du nouveau classement...", tracker.name());
                if !last_ladder.is_empty() {
                    if send_discord_alert(client, &current_ladder).is_ok() {
                        let _ = save_state(tracker.state_filename(), &current_ladder);
                    }
                } else {
                    log::info!(
                        "[{}] Initialisation de la base de référence du classement.",
                        tracker.name()
                    );
                    let _ = save_state(tracker.state_filename(), &current_ladder);
                }
            }
        }
        Err(e) => {
            log::warn!(
                "[{}] Erreur lors de la récupération du classement: {}",
                tracker.name(),
                e
            );
        }
    }

    sleep_duration
}

/// Runs the continuous monitoring loop for a `LadderTracker`.
pub fn run_tracker_loop<T: LadderTracker>(mut tracker: T) {
    let client = build_http_client();
    log::info!("🏴‍☠️ {} started.", tracker.name());

    loop {
        let sleep_duration = run_tracker_step(&mut tracker, &client);
        log::info!(
            "[{}] Sleeping for {} seconds before next check...",
            tracker.name(),
            sleep_duration.as_secs()
        );
        thread::sleep(sleep_duration);
    }
}
