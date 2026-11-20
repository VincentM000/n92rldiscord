# Guide de Contribution (CONTRIBUTING.md) 🏴‍☠️

Bienvenue sur le dépôt **Nanterre Pirates - Rugby League Automation Suite** !  
Ce document définit les règles d'architecture (**Clean Architecture**), de qualité de code (**Clean Code**), de tests, ainsi que les conventions Git (branches, commits et hooks `pre-commit`) à respecter pour contribuer au projet et éviter toute dérive du code.

---

## 1. Architecture du Projet (Clean Architecture)

Le workspace Cargo est organisé autour d'un socle commun (`pirate-utils`) et d'adaptateurs binaires spécialisés par compétition ou source de données :

```text
n92rldiscord/
├── pirate-utils/           # Socle partagé (Domain, Application Ports, Infrastructure)
│   └── src/
│       ├── domain.rs         # Entités pures (LadderRow, MatchResult) & formatage métier
│       ├── ladder_tracker.rs # Port (Trait LadderTracker) & Use Case d'orchestration
│       ├── state.rs          # Persistance d'état (texte et JSON, support de STATE_DIR)
│       ├── http.rs           # Client HTTP unifié (timeouts, User-Agent) & Jitter
│       └── discord.rs        # Adaptateur de notification Webhook Discord
├── nrl-tracker/            # Adaptateur NRL (JSON nrl.com -> LadderTracker)
├── super-league-tracker/   # Adaptateur Super League (HTML SkySports -> LadderTracker)
├── super-xiii-tracker/     # Adaptateur Super XIII (FFR13 + YouTube Live/Replay)
├── live-match-monitor/     # Moniteur de directs Rugbyrama
└── rss-feed-notifier/      # Veille RSS Treize Mondial
```

### Règles de séparation des responsabilités

1. **Couche Domaine (`pirate_utils::domain`)** :
   - Contient uniquement des structures de données métier ([`LadderRow`](pirate-utils/src/domain.rs), [`MatchResult`](pirate-utils/src/domain.rs)) et des fonctions **pures** (sans appels réseau, sans lecture/écriture disque, déterministes).
   - Tout formatage de classement ([`format_ladder`](pirate-utils/src/domain.rs)) ou de score de match ([`format_match_result`](pirate-utils/src/domain.rs)) doit passer par cette couche afin de garantir un affichage uniforme sur Discord.

2. **Couche Application / Use Cases (`pirate_utils::ladder_tracker`)** :
   - Définit le contrat [`LadderTracker`](pirate-utils/src/ladder_tracker.rs) et orchestre le cycle de vie via [`run_tracker_step`](pirate-utils/src/ladder_tracker.rs) et [`run_tracker_loop`](pirate-utils/src/ladder_tracker.rs).
   - Gère la comparaison des points totaux, la déduplication des états sauvegardés et l'envoi des notifications.

3. **Couche Infrastructure (`pirate_utils::{http, state, discord}`)** :
   - **Zéro duplication de client HTTP** : Utilisez systématiquement [`build_http_client`](pirate-utils/src/http.rs) au lieu d'instancier `reqwest::blocking::Client::new()` manuellement.
   - **Zéro duplication de gestion d'état** : Utilisez [`load_state`](pirate-utils/src/state.rs) / [`save_state`](pirate-utils/src/state.rs) (ou [`load_json_state`](pirate-utils/src/state.rs) / [`save_json_state`](pirate-utils/src/state.rs)) qui gèrent automatiquement la variable d'environnement `STATE_DIR` pour le déploiement Docker/VPS.
   - **Temporisation aléatoire** : Utilisez [`apply_jitter`](pirate-utils/src/http.rs) pour lisser les requêtes de scraping.

4. **Dépendances centralisées (`Cargo.toml` racine)** :
   - Toutes les versions de crates externes (`reqwest`, `serde`, `serde_json`, `chrono`, `scraper`, `log`, `env_logger`, `dotenvy`) sont déclarées une seule fois dans `[workspace.dependencies]` à la racine du projet.
   - Dans les sous-crates, utilisez toujours `{ workspace = true }`.

---

## 2. Principes Clean Code

- **Fonctions pures pour le parsing** : Séparez systématiquement la requête HTTP (`fetch_*`) de l'analyse syntaxique (`parse_*` ou `evaluate_*`). Une fonction de parsing doit prendre un `&str` (HTML/JSON/XML) en entrée et retourner un `Result<T, Box<dyn Error>>` ou une structure typée. Cela permet de la tester unitairement sans réseau.
- **Pas de constantes magiques dispersées** : Les constantes de temporisation, URLs et sélecteurs CSS doivent être déclarées clairement en haut du module en `UPPER_SNAKE_CASE`.
- **Journalisation structurée** : Utilisez les macros de la crate `log` (`info!`, `warn!`, `error!`, `debug!`) initialisées avec `env_logger::init()`. N'utilisez pas `println!` ou `eprintln!` dans le code applicatif.
- **Zéro warning toléré** : Le code doit compiler sans aucun avertissement `rustc` ni `clippy` (`-D warnings`).

---

## 3. Tests Unitaires & Formatage

Chaque nouvelle fonctionnalité ou correction de bug **doit** être accompagnée de tests unitaires dans un bloc `#[cfg(test)] mod tests`.

### Commandes de vérification locale

Avant tout commit, assurez-vous que ces trois commandes passent sans erreur :

```bash
# 1. Formater automatiquement tout le workspace
cargo fmt --all

# 2. Vérifier la qualité du code avec Clippy (aucun warning autorisé)
cargo clippy --workspace --all-targets -- -D warnings

# 3. Lancer l'intégralité des tests unitaires
cargo test --workspace
```

---

## 4. Conventions de Nommage Git

### 4.1 Nommage des Branches

Toutes les branches (à l'exception de `main`) doivent suivre le format suivant :

```text
<type>/<description-en-kebab-case>
```

**Types autorisés :**
- `feat` : Nouvelle fonctionnalité (ex: `feat/super-xiii-tracker`, `feat/youtube-live-alert`)
- `fix` : Correction de bug (ex: `fix/ffr13-score-parser`)
- `refactor` : Factorisation ou amélioration architecturale sans changement fonctionnel (ex: `refactor/clean-architecture`)
- `docs` : Ajout ou mise à jour de documentation (ex: `docs/contributing-guide`)
- `test` : Ajout ou amélioration de tests unitaires (ex: `test/nrl-json-fixtures`)
- `chore` : Maintenance, dépendances, configuration outillage (ex: `chore/update-cargo-deps`)
- `perf` : Optimisation de performance
- `ci` : Intégration continue / Docker / déploiement
- `hotfix` : Correction urgente en production
- `wip` : Travail en cours temporaire (à renommer ou squash avant merge)

**Règles :**
- Uniquement des lettres minuscules (`a-z`), chiffres (`0-9`) et tirets (`-`) après le `/`.
- Pas de majuscules, pas d'underscores (`_`), pas d'espaces ni d'accents.

### 4.2 Nommage des Commits (Conventional Commits)

Chaque message de commit doit respecter la spécification **[Conventional Commits](https://www.conventionalcommits.org/fr/)** :

```text
<type>(<scope optionnel>): <description courte à l'impératif>
```

**Types autorisés :**
`feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`, `build`, `style`, `revert`.

**Exemples valides :**
- `feat(super-xiii): add pre-match youtube live stream alert`
- `refactor(pirate-utils): mutualize LadderRow and MatchResult domain models`
- `fix(super-league): handle multi-line team names in sky sports scraper`
- `docs: add CONTRIBUTING.md and pre-commit hooks documentation`

**Exemples invalides (rejetés par le hook) :**
- `update code` *(pas de type)*
- `Feat: ajout super xiii` *(majuscule au type)*
- `wip` *(message trop court et type non autorisé pour un commit final)*

---

## 5. Système de Pre-Commit (Hooks Git)

Le dépôt inclut un système de hooks Git automatisés dans le dossier [`.githooks/`](.githooks/) ainsi qu'une configuration [`.pre-commit-config.yaml`](.pre-commit-config.yaml).

### Que vérifient les hooks automatiquement ?

1. **`pre-commit`** (avant chaque création de commit) :
   - Vérifie que le nom de la branche courante respecte la convention `<type>/<kebab-case>`.
   - Vérifie le formatage avec `cargo fmt --all -- --check`.
   - Vérifie les règles Clean Code avec `cargo clippy --workspace --all-targets -- -D warnings`.
   - Exécute toute la suite de tests unitaires avec `cargo test --workspace`.
2. **`commit-msg`** (lors de la validation du message) :
   - Vérifie que le message de commit respecte la norme **Conventional Commits**.
3. **`pre-push`** (avant chaque `git push`) :
   - Vérifie le nom de la branche et relance les tests unitaires.

### Installation (Activation des hooks)

#### Option A — Hooks Git natifs (Recommandé, sans outil externe)

Exécutez une seule fois à la racine du dépôt :

```bash
git config core.hooksPath .githooks
```

*(Sur Linux/macOS, assurez-vous que les scripts sont exécutables : `chmod +x .githooks/*`)*

#### Option B — Avec l'outil `pre-commit` (Python)

Si vous utilisez l'outil [`pre-commit`](https://pre-commit.com/) :

```bash
pre-commit install --hook-type pre-commit --hook-type commit-msg --hook-type pre-push
```

---

## 6. Workflow de Contribution Pas-à-Pas

1. **Partir d'une branche `main` à jour** :
   ```bash
   git checkout main
   git pull origin main
   ```
2. **Créer une branche nommée selon la convention** :
   ```bash
   git checkout -b feat/ma-nouvelle-fonctionnalite
   ```
3. **Développer en respectant la Clean Architecture** :
   - Mutualiser toute brique réutilisable dans `pirate-utils`.
   - Ajouter des tests unitaires pour toute logique de parsing ou de calcul.
4. **Formater et tester** :
   ```bash
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```
5. **Commiter en suivant Conventional Commits** :
   ```bash
   git add .
   git commit -m "feat(mon-module): add new feature description"
   ```
6. **Pousser et ouvrir une Pull Request vers `main`** :
   ```bash
   git push -u origin feat/ma-nouvelle-fonctionnalite
   ```
   > 💡 **Important** : Après le merge d'une Pull Request sur GitHub (notamment en *Squash and merge*), supprimez votre branche de travail et repartez toujours de `main` pour la fonctionnalité suivante afin d'éviter tout conflit d'historique.

