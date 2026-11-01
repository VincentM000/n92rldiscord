use crate::clubs::{Club, CLUBS};
use crate::state::AppState;
use pirate_utils::send_discord_alert;
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::thread;
use std::time::Duration;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct YouTubeVideo {
    pub video_id: String,
    pub title: String,
    pub link: String,
    pub published: String,
}

/// Fetch recent videos for a specific YouTube channel using its public Atom RSS feed
pub fn fetch_channel_videos(
    client: &Client,
    channel_id: &str,
) -> Result<Vec<YouTubeVideo>, Box<dyn std::error::Error>> {
    let url = format!(
        "https://www.youtube.com/feeds/videos.xml?channel_id={}",
        channel_id
    );

    let response = client.get(&url).send()?.text()?;
    let document = Html::parse_document(&response);

    let entry_selector = Selector::parse("entry").unwrap();
    let title_selector = Selector::parse("title").unwrap();
    let link_selector = Selector::parse("link[rel='alternate']").unwrap();

    let mut videos = Vec::new();

    for entry in document.select(&entry_selector) {
        let title = entry
            .select(&title_selector)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        let link = entry
            .select(&link_selector)
            .next()
            .and_then(|e| e.value().attr("href"))
            .unwrap_or_default()
            .to_string();

        let mut video_id = String::new();
        if let Some(pos) = link.find("v=") {
            video_id = link[pos + 2..].to_string();
        } else {
            let html_str = entry.html();
            if let Some(pos) = html_str.find("<yt:videoid>") {
                let rest = &html_str[pos + 12..];
                if let Some(end) = rest.find("</yt:videoid>") {
                    video_id = rest[..end].to_string();
                }
            }
        }

        if !video_id.is_empty() {
            let final_link = if link.is_empty() {
                format!("https://www.youtube.com/watch?v={}", video_id)
            } else {
                link
            };

            videos.push(YouTubeVideo {
                video_id,
                title,
                link: final_link,
                published: String::new(),
            });
        }
    }

    Ok(videos)
}

/// Determines if a video is a home match stream/replay or broadcast
pub fn is_match_video(host_club: &Club, video: &YouTubeVideo) -> bool {
    let title_lower = video.title.to_lowercase();

    // 1. Direct competition names
    let has_competition = title_lower.contains("super xiii")
        || title_lower.contains("super 13")
        || title_lower.contains("elite 1")
        || title_lower.contains("élite 1");

    // 2. Round indicators (j1, j2, ..., journee, journée)
    let has_round = title_lower.contains("journée")
        || title_lower.contains("journee")
        || (1..=22).any(|round| {
            title_lower.contains(&format!("j{}", round))
                || title_lower.contains(&format!("j-{}", round))
                || title_lower.contains(&format!("j {}", round))
        });

    // 3. Match indicators
    let has_match_keyword = title_lower.contains("match")
        || title_lower.contains("direct")
        || title_lower.contains("live")
        || title_lower.contains("diffusion")
        || title_lower.contains("replay");

    // 4. Opponent detection (does the title mention any other club in Super XIII?)
    let mentions_opponent = CLUBS.iter().any(|c| {
        c.slug != host_club.slug
            && (title_lower.contains(&c.name.to_lowercase())
                || c.aliases.iter().any(|a| title_lower.contains(a)))
    });

    let has_vs = title_lower.contains(" v ")
        || title_lower.contains(" vs ")
        || title_lower.contains(" contre ")
        || title_lower.contains(" - ");

    // It's a match broadcast if:
    // (Competition or Round) AND (Opponent or Match keyword or "vs")
    // OR (Opponent AND (Match keyword or "vs"))
    if (has_competition || has_round) && (mentions_opponent || has_match_keyword || has_vs) {
        return true;
    }

    if mentions_opponent && (has_match_keyword || has_vs) {
        return true;
    }

    // Explicit Super XIII match stream title (like Carcassonne: "SUPER XIII | Carcassonne XIII V Lézignan XIII")
    if has_competition && has_vs {
        return true;
    }

    false
}

/// Scans YouTube channels of all 11 Super XIII clubs for newly posted match broadcasts
pub fn scan_super_xiii_youtube_channels(
    client: &Client,
    state: &mut AppState,
) -> Result<usize, Box<dyn std::error::Error>> {
    println!("🏴‍☠️ [YouTube Scanner] Scanning 11 Super XIII club channels for home match broadcasts...");
    let mut new_videos_count = 0;

    for club in CLUBS {
        println!(
            "  📺 Checking channel for {} ({}) - {}",
            club.name, club.emoji, club.yt_handle
        );

        let videos = match fetch_channel_videos(client, club.yt_channel_id) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "    ⚠️ Error fetching YouTube feed for {}: {}",
                    club.name, e
                );
                continue;
            }
        };

        for video in videos {
            if state.sent_youtube_videos.contains(&video.video_id) {
                continue;
            }

            if is_match_video(club, &video) {
                println!(
                    "    🔥 NEW MATCH DETECTED for {}: \"{}\" ({})",
                    club.name, video.title, video.link
                );

                let message = format!(
                    "🏉 **DIFFUSION MATCH SUPER XIII** 📺 🏴‍☠️\n\nLe club à domicile **{}** {} a partagé son match :\n🎥 **{}**\n\n🔗 {}",
                    club.name, club.emoji, video.title, video.link
                );

                if let Err(e) = send_discord_alert(client, &message) {
                    eprintln!("    ⚠️ Error sending YouTube alert to Discord: {}", e);
                } else {
                    println!("    ✅ Alert sent to Discord!");
                    new_videos_count += 1;
                    state.sent_youtube_videos.insert(video.video_id.clone());
                    // Brief pause to maintain good etiquette with Discord webhooks
                    thread::sleep(Duration::from_secs(2));
                }
            } else {
                // Not a match stream (e.g. talk show, interview), remember it so we don't re-evaluate
                state.sent_youtube_videos.insert(video.video_id.clone());
            }
        }

        // Small delay between checking YouTube feeds
        thread::sleep(Duration::from_millis(500));
    }

    if let Err(e) = state.save() {
        eprintln!("⚠️ Failed to save state after YouTube scan: {}", e);
    }

    println!(
        "🏴‍☠️ [YouTube Scanner] Scan completed! {} new match stream(s) alerted.",
        new_videos_count
    );

    Ok(new_videos_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_match_video_detection() {
        let carcassonne = &CLUBS[2]; // Carcassonne

        // Live stream / Replay
        let v1 = YouTubeVideo {
            video_id: "3WgCfirniC4".to_string(),
            title: "🟡⚫️𝐒𝐔𝐏𝐄𝐑 𝐗𝐈𝐈𝐈 | Carcassonne XIII V Lézignan XIII".to_string(),
            link: "https://www.youtube.com/watch?v=3WgCfirniC4".to_string(),
            published: "2026-10-31".to_string(),
        };
        assert!(is_match_video(carcassonne, &v1));

        // J1 match against Saint-Gaudens
        let v2 = YouTubeVideo {
            video_id: "u4vHiRsnygA".to_string(),
            title: "J1 SUPER XIII RC SAINT GAUDENS VS CARCASSONNE XIII".to_string(),
            link: "https://www.youtube.com/watch?v=u4vHiRsnygA".to_string(),
            published: "2026-10-24".to_string(),
        };
        assert!(is_match_video(carcassonne, &v2));

        // Talk show / interview (NOT a match)
        let v3 = YouTubeVideo {
            video_id: "hNpa409_xG8".to_string(),
            title: "🎥 [Numéro 3] PARLONS CARCASSONNE XIII ! 🏉".to_string(),
            link: "https://www.youtube.com/watch?v=hNpa409_xG8".to_string(),
            published: "2026-10-28".to_string(),
        };
        assert!(!is_match_video(carcassonne, &v3));
    }
}
