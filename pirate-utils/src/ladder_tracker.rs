use crate::{load_state, save_state, send_discord_alert};
use reqwest::blocking::Client;
use std::thread;
use std::time::Duration;

pub trait LadderTracker {
    fn name(&self) -> &str;
    fn state_filename(&self) -> &str;
    fn match_state_filename(&self) -> &str;
    fn fetch_latest_match_result(
        &self,
        client: &Client,
    ) -> Result<Option<String>, Box<dyn std::error::Error>>;
    fn fetch_ladder(&self, client: &Client) -> Result<String, Box<dyn std::error::Error>>;
}

// 🧮 NOUVEAU : Fonction pour calculer le total des points de toutes les équipes
fn extract_total_league_points(ladder: &str) -> i32 {
    let mut total = 0;
    for line in ladder.lines() {
        // On cherche précisément la balise de vos points dans la chaîne formatée
        if let Some(start) = line.find("• **") {
            let after_bullet = &line[start + 4..];
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

pub fn run_tracker_loop<T: LadderTracker>(tracker: T) {
    let client = Client::builder()
        .user_agent("NanterrePirates-Scraper/1.0")
        .build()
        .expect("Failed to build HTTP client");

    loop {
        if let Ok(current_ladder) = tracker.fetch_ladder(&client) {
            let last_ladder = load_state(tracker.state_filename());

            if current_ladder != last_ladder && !current_ladder.is_empty() {
                if !last_ladder.is_empty() {
                    // Calcul de la somme des points de la ligue
                    let current_pts = extract_total_league_points(&current_ladder);
                    let last_pts = extract_total_league_points(&last_ladder);

                    // 🚨 VÉRIFICATION DU TAPIS VERT
                    if current_pts < last_pts {
                        log::warn!("Déduction de points détectée !");
                        let penalty_msg = "⚖️ **DÉCISION ADMINISTRATIVE :**\nUn retrait de points a été appliqué au classement !";
                        let _ = send_discord_alert(&client, penalty_msg);
                        thread::sleep(Duration::from_secs(2));
                    } else {
                        // 🏉 DÉROULEMENT NORMAL DES MATCHS
                        let current_match =
                            tracker.fetch_latest_match_result(&client).unwrap_or(None);
                        let last_match = load_state(tracker.match_state_filename());

                        if let Some(ref match_msg) = current_match {
                            // On bloque si le fichier global n'a pas bougé d'un iota
                            if match_msg == &last_match && !last_match.is_empty() {
                                log::info!("⏳ Classement modifié, mais aucun nouveau score détecté. On patiente...");
                                thread::sleep(Duration::from_secs(600));
                                continue;
                            }

                            let mut has_new_match = false;

                            // ⚓ LA MAGIE EST ICI : On découpe le filet et on cherche les prises inédites !
                            for single_match in match_msg.split("|||") {
                                if !last_match.contains(single_match) {
                                    has_new_match = true;
                                    let _ = send_discord_alert(&client, single_match);
                                    thread::sleep(Duration::from_secs(2));
                                }
                            }

                            // Si finalement tous les matchs étaient déjà dans l'historique (faux positif)
                            if !has_new_match && !last_match.is_empty() {
                                continue;
                            }

                            // On sauvegarde le nouveau filet complet
                            let _ = save_state(tracker.match_state_filename(), match_msg);
                        }
                    }
                }

                log::info!("Envoi du nouveau classement...");
                if send_discord_alert(&client, &current_ladder).is_ok() {
                    let _ = save_state(tracker.state_filename(), &current_ladder);
                }
            }
        } else {
            log::warn!("Erreur lors de la récupération du classement.");
        }

        // Repos de l'équipage avant le prochain tour de garde (par ex: 10 minutes)
        thread::sleep(Duration::from_secs(600));
    }
}
