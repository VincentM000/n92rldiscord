mod clubs;
mod ffr13;
mod state;
mod youtube;

use chrono::{Datelike, Local, Weekday};
use ffr13::{
    calculate_adaptive_sleep, format_match_result_message, parse_all_matches,
    parse_ranking_table, FFR13_SUPER_XIII_URL,
};
use pirate_utils::send_discord_alert;
use reqwest::blocking::Client;
use state::AppState;
use std::env;
use std::thread;
use std::time::Duration;
use youtube::{fetch_channel_videos, scan_super_xiii_youtube_channels};

fn main() {
    println!("🏴‍☠️ ================================================================");
    println!("🏴‍☠️ Nanterre Pirates - Super XIII Live Tracker & YouTube Monitor 🏴‍☠️");
    println!("🏴‍☠️ ================================================================\n");

    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) NanterrePirates-SuperXIII/1.0")
        .timeout(Duration::from_secs(20))
        .build()
        .expect("Failed to build HTTP client");

    let (mut state, is_first_run) = AppState::load();

    // Parse CLI arguments
    let args: Vec<String> = env::args().collect();
    let force_yt = args.iter().any(|a| a == "--scan-yt");
    let run_once = args.iter().any(|a| a == "--check-once" || a == "--now");
    let show_status = args.iter().any(|a| a == "--status");

    if show_status {
        println!("📊 État actuel du tracker Super XIII :");
        println!("  - Matchs terminés enregistrés : {}", state.completed_matches.len());
        println!("  - Vidéos YouTube enregistrées : {}", state.sent_youtube_videos.len());
        println!("  - Dernier scan YouTube : {:?}", state.last_yt_scan_date);
        return;
    }

    if is_first_run {
        println!("⚓ Premier lancement détecté ! Établissement de la base de référence pour éviter le spam...");
        initialize_baseline(&client, &mut state);
    }

    if force_yt {
        println!("⚡ Lancement forcé du scan YouTube...");
        if let Err(e) = scan_super_xiii_youtube_channels(&client, &mut state) {
            eprintln!("⚠️ Erreur lors du scan YouTube : {}", e);
        }
        if run_once {
            return;
        }
    }

    // Main Loop
    loop {
        // 1. Check YouTube (Monday to Friday, once per day)
        check_daily_youtube(&client, &mut state);

        // 2. Check FFR13 for match results and ladder updates
        let next_sleep = match check_ffr13_competition(&client, &mut state) {
            Ok(duration) => duration,
            Err(e) => {
                eprintln!("⚠️ Erreur lors de la vérification FFR13 : {}. Reprise dans 10 min.", e);
                Duration::from_secs(600)
            }
        };

        if run_once {
            println!("🏁 Mode exécution unique terminé.");
            break;
        }

        println!(
            "💤 Radar en veille... prochaine vigie dans {} secondes ({} minutes)...\n",
            next_sleep.as_secs(),
            next_sleep.as_secs() / 60
        );
        thread::sleep(next_sleep);
    }
}

/// Baseline initialization on first start to avoid alerting historic matches/videos
fn initialize_baseline(client: &Client, state: &mut AppState) {
    // 1. Baseline FFR13
    match client.get(FFR13_SUPER_XIII_URL).send().and_then(|r| r.text()) {
        Ok(html) => {
            let (_, ladder_msg) = parse_ranking_table(&html);
            state.last_ladder = ladder_msg;

            let matches = parse_all_matches(&html);
            let mut count = 0;
            for m in matches {
                if m.is_finished() {
                    state.completed_matches.insert(m.id);
                    count += 1;
                }
            }
            println!("  ✅ {} matchs passés enregistrés en référence.", count);
        }
        Err(e) => {
            eprintln!("  ⚠️ Impossible de récupérer la page FFR13 pour la baseline : {}", e);
        }
    }

    // 2. Baseline YouTube (mark existing videos as seen)
    println!("  📺 Enregistrement des vidéos YouTube actuelles pour éviter le spam initial...");
    for club in clubs::CLUBS {
        if let Ok(videos) = fetch_channel_videos(client, club.yt_channel_id) {
            for v in videos {
                state.sent_youtube_videos.insert(v.video_id);
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
    println!("  ✅ {} vidéos YouTube enregistrées.", state.sent_youtube_videos.len());

    let today_str = Local::now().format("%Y-%m-%d").to_string();
    state.last_yt_scan_date = Some(today_str);

    if let Err(e) = state.save() {
        eprintln!("⚠️ Erreur lors de la sauvegarde de la baseline : {}", e);
    }
    println!("⚓ Base de référence établie avec succès !\n");
}

/// Checks YouTube once per day, strictly from Monday to Friday
fn check_daily_youtube(client: &Client, state: &mut AppState) {
    let now = Local::now();
    let weekday = now.weekday();
    let is_weekday = weekday != Weekday::Sat && weekday != Weekday::Sun;
    let today_str = now.format("%Y-%m-%d").to_string();

    if !is_weekday {
        return;
    }

    if state.last_yt_scan_date.as_deref() == Some(&today_str) {
        return;
    }

    println!(
        "📅 Scan YouTube quotidien ({}) : recherche des matchs diffusés par les clubs...",
        today_str
    );

    match scan_super_xiii_youtube_channels(client, state) {
        Ok(_) => {
            state.last_yt_scan_date = Some(today_str);
            let _ = state.save();
        }
        Err(e) => {
            eprintln!("⚠️ Erreur lors du scan quotidien YouTube : {}", e);
        }
    }
}

/// Scrapes FFR13, sends match alerts, updates the standings, and returns the adaptive sleep time
fn check_ffr13_competition(
    client: &Client,
    state: &mut AppState,
) -> Result<Duration, Box<dyn std::error::Error>> {
    println!("📡 Vigie FFR13 : consultation de la page Super XIII...");
    let response = client.get(FFR13_SUPER_XIII_URL).send()?.text()?;

    // 1. Parse official ranking table
    let (_rows, current_ladder) = parse_ranking_table(&response);

    // 2. Parse all rounds and matches
    let matches = parse_all_matches(&response);

    // 3. Find newly completed matches
    let mut newly_finished = Vec::new();
    for m in &matches {
        if m.is_finished() && !state.completed_matches.contains(&m.id) {
            newly_finished.push(m.clone());
        }
    }

    // 4. Alert newly finished matches
    for m in newly_finished {
        let result_msg = format_match_result_message(&m);
        println!("🏁 Nouveau résultat détecté ! Envoi à Discord :\n{}", result_msg);

        if let Err(e) = send_discord_alert(client, &result_msg) {
            eprintln!("⚠️ Échec de l'envoi du score à Discord : {}", e);
        }

        // Wait 2 seconds before sending the ladder to guarantee Discord message order
        println!("⏳ Pause de 2 secondes avant l'envoi du classement mis à jour...");
        thread::sleep(Duration::from_secs(2));

        if !current_ladder.is_empty() {
            println!("🏆 Envoi du classement mis à jour à Discord...");
            if let Err(e) = send_discord_alert(client, &current_ladder) {
                eprintln!("⚠️ Échec de l'envoi du classement : {}", e);
            }
            state.last_ladder = current_ladder.clone();
        }

        state.completed_matches.insert(m.id);
        let _ = state.save();
    }

    // 5. If ladder changed independently (e.g. bonus point recalculation or sanctions)
    if !current_ladder.is_empty() && current_ladder != state.last_ladder {
        if !state.last_ladder.is_empty() {
            println!("📊 Évolution du classement détectée hors fin de match ! Envoi à Discord...");
            if let Err(e) = send_discord_alert(client, &current_ladder) {
                eprintln!("⚠️ Échec de l'envoi du classement : {}", e);
            }
        }
        state.last_ladder = current_ladder;
        let _ = state.save();
    }

    // 6. Calculate adaptive sleeping time using the matches calendar
    let sleep_duration = calculate_adaptive_sleep(&matches);
    Ok(sleep_duration)
}
