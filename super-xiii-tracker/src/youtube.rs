use crate::clubs::{Club, CLUBS};
use crate::state::AppState;
use log::{info, warn};
use pirate_utils::{extract_between, send_discord_alert};
use reqwest::blocking::Client;
use scraper::{Html, Selector};
use std::thread;
use std::time::Duration;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct LiveStreamDetails {
    pub video_id: String,
    pub title: String,
    pub url: String,
    pub is_live_now: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct YouTubeVideo {
    pub video_id: String,
    pub title: String,
    pub link: String,
    pub published: String,
}

/// Extracts the canonical video ID and title from a YouTube `/live` HTML page,
/// ignoring sidebar recommendations (`watchEndpoint`).
pub fn parse_live_page_html(response_text: &str, club: &Club) -> Option<LiveStreamDetails> {
    if response_text.is_empty()
        || (!response_text.contains("liveBroadcastDetails")
            && !response_text.contains("\"isLive\":true")
            && !response_text.contains("\"isUpcoming\":true")
            && !response_text
                .contains("<link rel=\"canonical\" href=\"https://www.youtube.com/watch?v="))
    {
        return None;
    }

    // Never use "watchEndpoint" as it matches recommended videos from other channels in the sidebar.
    let video_id = extract_between(
        response_text,
        "<link rel=\"canonical\" href=\"https://www.youtube.com/watch?v=",
        "\"",
    )
    .or_else(|| {
        extract_between(
            response_text,
            "<meta property=\"og:url\" content=\"https://www.youtube.com/watch?v=",
            "\"",
        )
    })
    .or_else(|| extract_between(response_text, "\"videoDetails\":{\"videoId\":\"", "\""))
    .or_else(|| extract_between(response_text, "\"externalVideoId\":\"", "\""))?;

    let title = extract_between(
        response_text,
        "<meta property=\"og:title\" content=\"",
        "\"",
    )
    .or_else(|| extract_between(response_text, "<title>", "</title>"))
    .map(|s| s.replace(" - YouTube", "").trim().to_string())
    .unwrap_or_else(|| format!("Match en direct de {}", club.name));

    let is_live_now =
        response_text.contains("\"isLiveNow\":true") || response_text.contains("\"isLive\":true");

    Some(LiveStreamDetails {
        video_id: video_id.to_string(),
        title,
        url: format!("https://www.youtube.com/watch?v={}", video_id),
        is_live_now,
    })
}

/// Probes specifically the club's YouTube channel for an active live stream or scheduled waiting room.
pub fn check_club_live_stream(
    client: &Client,
    club: &Club,
) -> Result<Option<LiveStreamDetails>, Box<dyn std::error::Error>> {
    let live_url = format!("https://www.youtube.com/{}/live", club.yt_handle);

    let response_text = match client
        .get(&live_url)
        .header("Accept-Language", "fr-FR,fr;q=0.9")
        .send()
    {
        Ok(res) => res.text().unwrap_or_default(),
        Err(e) => {
            warn!("Error accessing {}/live: {}", club.yt_handle, e);
            String::new()
        }
    };

    if let Some(details) = parse_live_page_html(&response_text, club) {
        return Ok(Some(details));
    }

    if let Ok(videos) = fetch_channel_videos(client, club.yt_channel_id) {
        if let Some(first) = videos.first() {
            if is_match_video(club, first) {
                return Ok(Some(LiveStreamDetails {
                    video_id: first.video_id.clone(),
                    title: first.title.clone(),
                    url: first.link.clone(),
                    is_live_now: true,
                }));
            }
        }
    }

    Ok(None)
}

/// Fetches recent videos for a specific YouTube channel using its public Atom RSS feed.
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
            if let Some(extracted) = extract_between(&html_str, "<yt:videoid>", "</yt:videoid>") {
                video_id = extracted.to_string();
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

/// Determines if a video is a home match stream/replay or broadcast.
pub fn is_match_video(host_club: &Club, video: &YouTubeVideo) -> bool {
    let title_lower = video.title.to_lowercase();

    let has_competition = title_lower.contains("super xiii")
        || title_lower.contains("super 13")
        || title_lower.contains("elite 1")
        || title_lower.contains("élite 1");

    let has_round = title_lower.contains("journée")
        || title_lower.contains("journee")
        || (1..=22).any(|round| {
            title_lower.contains(&format!("j{}", round))
                || title_lower.contains(&format!("j-{}", round))
                || title_lower.contains(&format!("j {}", round))
        });

    let has_match_keyword = title_lower.contains("match")
        || title_lower.contains("direct")
        || title_lower.contains("live")
        || title_lower.contains("diffusion")
        || title_lower.contains("replay");

    let mentions_opponent = CLUBS.iter().any(|c| {
        c.slug != host_club.slug
            && (title_lower.contains(&c.name.to_lowercase())
                || c.aliases.iter().any(|a| title_lower.contains(a)))
    });

    let has_vs = title_lower.contains(" v ")
        || title_lower.contains(" vs ")
        || title_lower.contains(" contre ")
        || title_lower.contains(" - ");

    if (has_competition || has_round) && (mentions_opponent || has_match_keyword || has_vs) {
        return true;
    }

    if mentions_opponent && (has_match_keyword || has_vs) {
        return true;
    }

    if has_competition && has_vs {
        return true;
    }

    false
}

/// Scans YouTube channels of all 11 Super XIII clubs for newly posted match broadcasts.
pub fn scan_super_xiii_youtube_channels(
    client: &Client,
    state: &mut AppState,
) -> Result<usize, Box<dyn std::error::Error>> {
    info!("🏴‍☠️ [YouTube Scanner] Scanning 11 Super XIII club channels for home match broadcasts...");
    let mut new_videos_count = 0;

    for club in CLUBS {
        info!(
            "📺 Checking channel for {} ({}) - {}",
            club.name, club.emoji, club.yt_handle
        );

        let videos = match fetch_channel_videos(client, club.yt_channel_id) {
            Ok(v) => v,
            Err(e) => {
                warn!("Error fetching YouTube feed for {}: {}", club.name, e);
                continue;
            }
        };

        for video in videos {
            if state.sent_youtube_videos.contains(&video.video_id) {
                continue;
            }

            if is_match_video(club, &video) {
                info!(
                    "🔥 NEW MATCH DETECTED for {}: \"{}\" ({})",
                    club.name, video.title, video.link
                );

                let message = format!(
                    "🏉 **DIFFUSION MATCH SUPER XIII** 📺 🏴‍☠️\n\nLe club à domicile **{}** {} a partagé son match :\n🎥 **{}**\n\n🔗 {}",
                    club.name, club.emoji, video.title, video.link
                );

                if let Err(e) = send_discord_alert(client, &message) {
                    warn!("Error sending YouTube alert to Discord: {}", e);
                } else {
                    info!("✅ Alert sent to Discord!");
                    new_videos_count += 1;
                    state.sent_youtube_videos.insert(video.video_id.clone());
                    thread::sleep(Duration::from_secs(2));
                }
            } else {
                state.sent_youtube_videos.insert(video.video_id.clone());
            }
        }

        thread::sleep(Duration::from_millis(500));
    }

    if let Err(e) = state.save() {
        warn!("Failed to save state after YouTube scan: {}", e);
    }

    info!(
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
        let carcassonne = &CLUBS[2];

        let v1 = YouTubeVideo {
            video_id: "3WgCfirniC4".to_string(),
            title: "🟡⚫️𝐒𝐔𝐏𝐄𝐑 𝐗𝐈𝐈𝐈 | Carcassonne XIII V Lézignan XIII".to_string(),
            link: "https://www.youtube.com/watch?v=3WgCfirniC4".to_string(),
            published: "2026-10-31".to_string(),
        };
        assert!(is_match_video(carcassonne, &v1));

        let v2 = YouTubeVideo {
            video_id: "u4vHiRsnygA".to_string(),
            title: "J1 SUPER XIII RC SAINT GAUDENS VS CARCASSONNE XIII".to_string(),
            link: "https://www.youtube.com/watch?v=u4vHiRsnygA".to_string(),
            published: "2026-10-24".to_string(),
        };
        assert!(is_match_video(carcassonne, &v2));

        let v3 = YouTubeVideo {
            video_id: "hNpa409_xG8".to_string(),
            title: "🎥 [Numéro 3] PARLONS CARCASSONNE XIII ! 🏉".to_string(),
            link: "https://www.youtube.com/watch?v=hNpa409_xG8".to_string(),
            published: "2026-10-28".to_string(),
        };
        assert!(!is_match_video(carcassonne, &v3));
    }

    #[test]
    fn test_parse_live_page_ignores_sidebar_watch_endpoint() {
        let carcassonne = &CLUBS[2];
        let mock_html = r#"
            <html>
            <head>
                <link rel="canonical" href="https://www.youtube.com/watch?v=3WgCfirniC4">
                <meta property="og:title" content="🟡⚫️𝐒𝐔𝐏𝐄𝐑 𝐗𝐈𝐈𝐈 | Carcassonne XIII V Lézignan XIII">
            </head>
            <body>
                <script>
                    var ytInitialData = {"sidebar":{"watchEndpoint":{"videoId":"LSGTtQc4FoM"}},"liveBroadcastDetails":{"isLiveNow":false}};
                </script>
            </body>
            </html>
        "#;

        let details = parse_live_page_html(mock_html, carcassonne).unwrap();
        assert_eq!(details.video_id, "3WgCfirniC4");
        assert_eq!(details.url, "https://www.youtube.com/watch?v=3WgCfirniC4");
        assert!(details.title.contains("Carcassonne XIII"));
    }
}
