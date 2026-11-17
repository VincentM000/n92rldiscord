# Nanterre Pirates - Rugby League Automation Suite 🏴‍☠️

A collection of Rust-based tools designed to automate notifications and data tracking for our club's Discord server, built following **Clean Architecture** and **Clean Code** principles.

## Crate Architecture

*   **`pirate-utils`:** Shared core library encapsulating pure domain models (`LadderRow`, `MatchResult`), application orchestration (`LadderTracker` trait & polling loop), and infrastructure adapters (HTTP client, state persistence, Discord webhooks).
*   **`nrl-tracker`:** Automatically fetches and updates current NRL match scores and standings.
*   **`super-league-tracker`:** Scrapes Sky Sports for the latest Super League results and live standings updates.
*   **`super-xiii-tracker`:** Monitors the French Super XIII championship on FFR13 (adaptive polling, match results, standings updates), detects imminent live broadcasts on YouTube (`[H-15m, H+10m]`), and scans the YouTube channels of all 11 clubs from Monday to Friday for home match replays.
*   **`live-match-monitor`:** Monitors Rugbyrama's live page for Super League games and sends Discord alerts.
*   **`rss-feed-notifier`:** Monitors the *Treize Mondial* RSS feed and notifies the Discord server whenever a new article is published.

## Contributing & Pre-Commit Hooks

Please read [**CONTRIBUTING.md**](CONTRIBUTING.md) before submitting changes. It details our Clean Architecture layers, coding standards, unit testing rules, and branch/commit naming conventions.

To enable the automated Git hooks (`pre-commit`, `commit-msg`, `pre-push`):

```bash
git config core.hooksPath .githooks
```

## Tech Stack
*   **Language:** Rust (Workspace Edition 2021)
*   **Platform:** VPS (Linux / Docker) & Windows
*   **Notification:** Discord Webhooks
