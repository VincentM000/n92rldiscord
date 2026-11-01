mod clubs;
mod ffr13;
mod state;
mod youtube;

use chrono::{Datelike, Weekday};
use ffr13::{
    calculate_adaptive_sleep, format_live_started_alert_message, format_match_result_message,
    format_prematch_alert_message, get_matches_in_prematch_window, paris_now, parse_all_matches,
    parse_ranking_table, FFR13_SUPER_XIII_URL,
};
use log::{info, warn};
use pirate_utils::{apply_jitter, build_http_client, send_discord_alert};
use reqwest::blocking::Client;
use state::AppState;
use std::env;
use std::thread;
use std::time::Duration;
use youtube::{fetch_channel_videos, scan_super_xiii_youtube_channels};

fn main() {
    env_logger::init();
    info!("🏴‍☠️ Nanterre Pirates - Super XIII Live Tracker & YouTube Monitor started.");

    let client = build_http_client();
    let (mut state, is_first_run) = AppState::load();

    let args: Vec<String> = env::args().collect();
    let force_yt = args.iter().any(|a| a == "--scan-yt");
    let run_once = args.iter().any(|a| a == "--check-once" || a == "--now");
    let show_status = args.iter().any(|a| a == "--status");

    if show_status {
        println!("📊 État actuel du tracker Super XIII :");
        println!(
            "  - Matchs terminés enregistrés : {}",
            state.completed_matches.len()
        );
        println!(
            "  - Alertes avant-match envoyées : {}",
            state.alerted_pre_matches.len()
        );
        println!(
            "  - Vidéos YouTube enregistrées : {}",
            state.sent_youtube_videos.len()
        );
        println!("  - Dernier scan YouTube : {:?}", state.last_yt_scan_date);
        return;
    }

    if is_first_run {
        info!("⚓ Premier lancement détecté ! Établissement de la base de référence...");
        initialize_baseline(&client, &mut state);
    }

    if force_yt {
        info!("⚡ Lancement forcé du scan YouTube...");
        if let Err(e) = scan_super_xiii_youtube_channels(&client, &mut state) {
            warn!("Erreur lors du scan YouTube : {}", e);
        }
        if run_once {
            return;
        }
    }

    loop {
        check_daily_youtube(&client, &mut state);

        let next_sleep = match check_ffr13_competition(&client, &mut state) {
            Ok(duration) => duration,
            Err(e) => {
                warn!(
                    "Erreur lors de la vérification FFR13 : {}. Reprise dans 10 min.",
                    e
                );
                apply_jitter(600)
            }
        };

        if run_once {
            info!("🏁 Mode exécution unique terminé.");
            break;
        }

        info!(
            "💤 Radar en veille... prochaine vigie dans {} secondes ({} minutes)...",
            next_sleep.as_secs(),
            next_sleep.as_secs() / 60
        );
        thread::sleep(next_sleep);
    }
}

fn initialize_baseline(client: &Client, state: &mut AppState) {
    match client
        .get(FFR13_SUPER_XIII_URL)
        .send()
        .and_then(|r| r.text())
    {
        Ok(html) => {
            let (_, ladder_msg) = parse_ranking_table(&html);
            state.last_ladder = ladder_msg;

            let matches = parse_all_matches(&html);
            let mut count = 0;
            for m in matches {
                if m.is_finished() {
                    state.completed_matches.insert(m.id);
                    state.alerted_pre_matches.insert(m.id);
                    state.alerted_live_matches.insert(m.id);
                    count += 1;
                }
            }
            info!("✅ {} matchs passés enregistrés en référence.", count);
        }
        Err(e) => {
            warn!(
                "Impossible de récupérer la page FFR13 pour la baseline : {}",
                e
            );
        }
    }

    info!("📺 Enregistrement des vidéos YouTube actuelles pour éviter le spam initial...");
    for club in clubs::CLUBS {
        if let Ok(videos) = fetch_channel_videos(client, club.yt_channel_id) {
            for v in videos {
                state.sent_youtube_videos.insert(v.video_id);
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
    info!(
        "✅ {} vidéos YouTube enregistrées.",
        state.sent_youtube_videos.len()
    );

    let today_str = paris_now().format("%Y-%m-%d").to_string();
    state.last_yt_scan_date = Some(today_str);

    if let Err(e) = state.save() {
        warn!("Erreur lors de la sauvegarde de la baseline : {}", e);
    }
    info!("⚓ Base de référence établie avec succès !");
}

fn check_daily_youtube(client: &Client, state: &mut AppState) {
    let now = paris_now();
    let weekday = now.weekday();
    let is_weekday = weekday != Weekday::Sat && weekday != Weekday::Sun;
    let today_str = now.format("%Y-%m-%d").to_string();

    if !is_weekday || state.last_yt_scan_date.as_deref() == Some(&today_str) {
        return;
    }

    info!(
        "📅 Scan YouTube quotidien ({}) : recherche des matchs diffusés par les clubs...",
        today_str
    );

    match scan_super_xiii_youtube_channels(client, state) {
        Ok(_) => {
            state.last_yt_scan_date = Some(today_str);
            let _ = state.save();
        }
        Err(e) => {
            warn!("Erreur lors du scan quotidien YouTube : {}", e);
        }
    }
}

fn check_ffr13_competition(
    client: &Client,
    state: &mut AppState,
) -> Result<Duration, Box<dyn std::error::Error>> {
    info!("📡 Vigie FFR13 : consultation de la page Super XIII...");
    let response = client.get(FFR13_SUPER_XIII_URL).send()?.text()?;

    let (_rows, current_ladder) = parse_ranking_table(&response);
    let matches = parse_all_matches(&response);

    if state.completed_matches.is_empty() {
        for m in &matches {
            if m.is_finished() {
                state.completed_matches.insert(m.id);
                state.alerted_pre_matches.insert(m.id);
                state.alerted_live_matches.insert(m.id);
            }
        }
        if !state.completed_matches.is_empty() {
            info!(
                "⚓ Initialisation silencieuse de {} matchs déjà terminés (anti-spam).",
                state.completed_matches.len()
            );
            let _ = state.save();
        }
    }

    check_upcoming_prematch_streams(client, state, &matches);

    let mut newly_finished = Vec::new();
    for m in &matches {
        if m.is_finished() && !state.completed_matches.contains(&m.id) {
            newly_finished.push(m.clone());
        } else if m.has_score() && !m.is_finished() && !state.completed_matches.contains(&m.id) {
            info!(
                "⏳ Score intermédiaire détecté pour {} vs {} ({:?}-{:?}) avant la fin réglementaire (1h50) — publication différée.",
                m.home_team, m.away_team, m.score_home, m.score_away
            );
        }
    }

    let ladder_has_changed = !current_ladder.is_empty()
        && !state.last_ladder.is_empty()
        && current_ladder != state.last_ladder;

    if !newly_finished.is_empty() {
        if !ladder_has_changed && !state.last_ladder.is_empty() {
            info!(
                "🛡️ Garde-fou actif : {} match(s) terminé(s) détecté(s), mais le classement FFR13 n'a pas encore été mis à jour. Attente du prochain cycle...",
                newly_finished.len()
            );
        } else {
            for m in newly_finished {
                let result_msg = format_match_result_message(&m);
                info!(
                    "🏁 Nouveau résultat final validé (avec évolution du classement) ! Envoi à Discord :\n{}",
                    result_msg
                );

                if let Err(e) = send_discord_alert(client, &result_msg) {
                    warn!("Échec de l'envoi du score à Discord : {}", e);
                }

                thread::sleep(Duration::from_secs(2));

                if !current_ladder.is_empty() {
                    info!("🏆 Envoi du classement mis à jour à Discord...");
                    if let Err(e) = send_discord_alert(client, &current_ladder) {
                        warn!("Échec de l'envoi du classement : {}", e);
                    }
                    state.last_ladder = current_ladder.clone();
                }

                state.completed_matches.insert(m.id);
                let _ = state.save();
            }
        }
    } else if !current_ladder.is_empty() && state.last_ladder.is_empty() {
        state.last_ladder = current_ladder;
        let _ = state.save();
    }

    let sleep_duration = calculate_adaptive_sleep(&matches);
    Ok(sleep_duration)
}

fn check_upcoming_prematch_streams(
    client: &Client,
    state: &mut AppState,
    matches: &[ffr13::FfrMatch],
) {
    let prematch_matches = get_matches_in_prematch_window(matches);
    if prematch_matches.is_empty() {
        return;
    }

    info!(
        "⚡ [Radar Avant-match] Fenêtre active : {} match(s) approchant du coup d'envoi.",
        prematch_matches.len()
    );

    for m in prematch_matches {
        if state.alerted_live_matches.contains(&m.id) {
            continue;
        }

        if let Some(host_club) = clubs::find_club_by_name(&m.home_team) {
            info!(
                "🔍 Vérification de la chaîne de {} ({}) pour le direct...",
                host_club.name, host_club.yt_handle
            );

            match youtube::check_club_live_stream(client, host_club) {
                Ok(Some(live)) => {
                    if live.is_live_now {
                        info!(
                            "🔴 COUP D'ENVOI / DIRECT ACTIF DÉTECTÉ pour {} : \"{}\" ({})",
                            host_club.name, live.title, live.url
                        );

                        let alert_msg =
                            format_live_started_alert_message(&m, &live.title, &live.url);

                        if let Err(e) = send_discord_alert(client, &alert_msg) {
                            warn!("Erreur lors de l'envoi de l'alerte direct actif : {}", e);
                        } else {
                            info!("✅ Alerte 'LE MATCH EST EN DIRECT' envoyée sur Discord !");
                            state.alerted_pre_matches.insert(m.id);
                            state.alerted_live_matches.insert(m.id);
                            state.sent_youtube_videos.insert(live.video_id);
                            let _ = state.save();
                            thread::sleep(Duration::from_secs(2));
                        }
                    } else if !state.alerted_pre_matches.contains(&m.id) {
                        info!(
                            "🚨 SALLE D'ATTENTE DÉTECTÉE pour {} : \"{}\" ({})",
                            host_club.name, live.title, live.url
                        );

                        let alert_msg = format_prematch_alert_message(&m, &live.title, &live.url);

                        if let Err(e) = send_discord_alert(client, &alert_msg) {
                            warn!("Erreur lors de l'envoi de l'alerte avant-match : {}", e);
                        } else {
                            info!("✅ Alerte coup d'envoi imminent envoyée sur Discord !");
                            state.alerted_pre_matches.insert(m.id);
                            state.sent_youtube_videos.insert(live.video_id);
                            let _ = state.save();
                            thread::sleep(Duration::from_secs(2));
                        }
                    } else {
                        info!(
                            "⏳ Salle d'attente déjà notifiée pour {}, en attente du passage en direct...",
                            host_club.name
                        );
                    }
                }
                Ok(None) => {
                    info!(
                        "⏳ Aucun flux en direct actif pour le moment sur {}.",
                        host_club.yt_handle
                    );
                }
                Err(e) => {
                    warn!("Erreur vérification live pour {} : {}", host_club.name, e);
                }
            }
        }
    }
}
