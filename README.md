# Nanterre Pirates - Rugby League Automation Suite 🏴‍☠️

A collection of Rust-based automation bots and an AI knowledge base (Hermes Agent) built for the **Nanterre Pirates** Rugby League club Discord server, following **Clean Architecture**, **Clean Code**, and **Gitflow** principles.

## Architecture

### Rust Crates
*   **`pirate-utils`:** Shared core library encapsulating pure domain models (`LadderRow`, `MatchResult`), Europe/Paris time helpers (`chrono-tz`), application orchestration (`LadderTracker` trait & polling loop), and infrastructure adapters (HTTP client, state persistence, Discord webhooks).
*   **`nrl-tracker`:** Fetches and publishes NRL match scores and updated standings.
*   **`super-league-tracker`:** Scrapes Sky Sports for Super League match results and live standings updates.
*   **`super-xiii-tracker`:** Monitors the French Super XIII championship on FFR13 (adaptive polling, autonomous Rust standings reconciliation, match results), detects imminent & active YouTube live broadcasts for all 11 clubs, and scans channels for weekday match replays.
*   **`live-match-monitor`:** Monitors Rugbyrama's live page for Super League broadcasts and sends Discord alerts.
*   **`rss-feed-notifier`:** Monitors the *Treize Mondial* RSS feed for new Rugby League articles.
*   **`weather-notifier` (Nami):** Runs daily at 19:00 Europe/Paris (`systemd` timer) to post weather forecasts for Nanterre training sessions and upcoming matches listed in [`matches.txt`](matches.txt) via Open-Meteo.

### Polly (Hermes Agent Knowledge Base)
*   **[`polly/`](polly/):** Markdown knowledge base (`SOUL.md`, `MEMORY.md`, and `skills/nanterre-pirates-faq/`) powering the **Polly** Discord assistant for youth players and parents (official FFR XIII age categories, U7–U13 & U15+ rules, 2026–2027 youth license documents, and club calendar).

### Deployment & Configuration
*   **[`deploy/`](deploy/):** Versioned `systemd` unit files (`.service` & `.timer`) and one-command VPS deployment script ([`deploy/deploy.sh`](deploy/deploy.sh)).
*   **[`.env.example`](.env.example):** Template of required environment variables (`DISCORD_*_WEBHOOK_URL`, `STATE_DIR`, etc.).

## Workflow, CI & Releases

*   **Gitflow & Pre-Commit Hooks:** See [**CONTRIBUTING.md**](CONTRIBUTING.md). Enable local hooks with:
    ```bash
    git config core.hooksPath .githooks
    ```
*   **CI (GitHub Actions):** Runs `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace`, and Gitflow branch checks on every PR.
*   **Automated Releases:** Managed by **Release Please** on `main` (generates `CHANGELOG.md` and semantic version tags `vX.Y.Z`). Merge `develop` → `main` using **Create a merge commit** to include all feature/fix/docs commits in the changelog.

## Tech Stack
*   **Language:** Rust (Workspace Edition 2021)
*   **AI Assistant:** Hermes Agent (Discord Gateway + Markdown Skills)
*   **Platform:** Ubuntu VPS (`systemd`) & Windows
*   **Notifications:** Discord Webhooks & Bot Gateway
