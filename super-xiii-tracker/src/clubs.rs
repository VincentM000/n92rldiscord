/// Information on each Super XIII club
#[derive(Debug, Clone)]
pub struct Club {
    pub slug: &'static str,
    pub name: &'static str,
    pub emoji: &'static str,
    pub yt_channel_id: &'static str,
    pub yt_handle: &'static str,
    pub aliases: &'static [&'static str],
}

pub const CLUBS: &[Club] = &[
    Club {
        slug: "avignon",
        name: "SO Avignon XIII",
        emoji: "🦬",
        yt_channel_id: "UCGZpP72LJztiTvH1j0H1Z5A",
        yt_handle: "@SOAVIGNONXIIIOFFICIEL",
        aliases: &["avignon", "bisons", "soa", "so avignon", "so avignon xiii"],
    },
    Club {
        slug: "villefranche",
        name: "VILLEFRANCHE XIII AVEYRON",
        emoji: "🐺",
        yt_channel_id: "UCHRWsImIi78zE0tMRz7RqLQ",
        yt_handle: "@lesloupsvillefrancheXIII",
        aliases: &[
            "villefranche",
            "loups",
            "aveyron",
            "villefranche xiii",
            "villefranche xiii aveyron",
        ],
    },
    Club {
        slug: "carcassonne",
        name: "Carcassonne XIII",
        emoji: "🐤",
        yt_channel_id: "UCnobtSahekk-hGlSF3BYzyA",
        yt_handle: "@carcassonnexiii",
        aliases: &[
            "carcassonne",
            "canaris",
            "asc xiii",
            "carcassonne xiii",
            "as carcassonne",
        ],
    },
    Club {
        slug: "albi",
        name: "Albi RL XIII",
        emoji: "🐯",
        yt_channel_id: "UCxiOpJg24t0oCaPIFkI4ZuA",
        yt_handle: "@albirugbyleaguexiii",
        aliases: &["albi", "tigres", "albi rl", "albi rl xiii", "albi rugby league"],
    },
    Club {
        slug: "limoux",
        name: "GRIZZLIES - XIII LIMOUXIN",
        emoji: "🐻",
        yt_channel_id: "UCYY0p_oc5WWz8nnDo5BRVaw",
        yt_handle: "@xiiilimouxintv",
        aliases: &[
            "limoux",
            "grizzlies",
            "xiii limouxin",
            "limouxin",
            "grizzlies - xiii limouxin",
        ],
    },
    Club {
        slug: "pia",
        name: "Pia XIII Baroudeur",
        emoji: "🤠",
        yt_channel_id: "UC7koH1Fm6L2VWq94MnBoi3w",
        yt_handle: "@piaxiiibaroudeur4527",
        aliases: &[
            "pia",
            "baroudeur",
            "baroudeurs",
            "pia xiii",
            "pia xiii baroudeur",
            "le xiii baroudeur de pia",
        ],
    },
    Club {
        slug: "st_esteve",
        name: "Saint-Estève XIII Catalan",
        emoji: "🐲",
        yt_channel_id: "UC14TrQMsQ0MULklLyROPtMw",
        yt_handle: "@Saint-EsteveXIIICatalan",
        aliases: &[
            "saint-esteve",
            "saint esteve",
            "st esteve",
            "baby dracs",
            "saint-estève",
            "saint-estève xiii catalan",
            "st esteve xiii catalan",
        ],
    },
    Club {
        slug: "toulouse",
        name: "Toulouse Olympique Elite",
        emoji: "✈️",
        yt_channel_id: "UC5F4t_nCF4etiUXci7MeFig",
        yt_handle: "@associationtoulouseolympique13",
        aliases: &[
            "toulouse",
            "to xiii",
            "to elite",
            "toulouse olympique",
            "toulouse olympique elite",
        ],
    },
    Club {
        slug: "lezignan",
        name: "FC Lézignan XIII",
        emoji: "🟢",
        yt_channel_id: "UCt8LFVbNfzwKayxxSwnfsWg",
        yt_handle: "@fclXIII",
        aliases: &[
            "lezignan",
            "lézignan",
            "fcl",
            "meuniers",
            "fc lezignan",
            "fc lézignan",
            "fc lézignan xiii",
        ],
    },
    Club {
        slug: "villeneuve",
        name: "Villeneuve XIII RL",
        emoji: "🐆",
        yt_channel_id: "UCojVILqgRL5i6uVavVY3lPg",
        yt_handle: "@leopardsdaquitaine",
        aliases: &[
            "villeneuve",
            "leopards",
            "léopards",
            "villeneuve xiii",
            "villeneuve xiii rl",
        ],
    },
    Club {
        slug: "st_gaudens",
        name: "RC ST GAUDENS",
        emoji: "🐾",
        yt_channel_id: "UCXSDndFO086gQNqfaADisiA",
        yt_handle: "@racingclubstgaudenscomming8478",
        aliases: &[
            "saint-gaudens",
            "saint gaudens",
            "st gaudens",
            "ours",
            "rc saint-gaudens",
            "rc saint gaudens",
            "rc st gaudens",
            "rc st gaudens xiii",
        ],
    },
];

/// Match a raw team name to the canonical Club entry
pub fn find_club_by_name(raw_name: &str) -> Option<&'static Club> {
    let lower = raw_name.trim().to_lowercase();
    for club in CLUBS {
        if club.name.to_lowercase() == lower {
            return Some(club);
        }
        for alias in club.aliases {
            if lower.contains(alias) {
                return Some(club);
            }
        }
    }
    None
}

/// Returns the formatted team name and its emoji
pub fn get_club_display(raw_name: &str) -> (String, &'static str) {
    if let Some(club) = find_club_by_name(raw_name) {
        (club.name.to_string(), club.emoji)
    } else {
        (raw_name.trim().to_string(), "🏉")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_11_clubs_lookup() {
        let expected_clubs = [
            ("SO Avignon XIII", "🦬"),
            ("VILLEFRANCHE XIII AVEYRON", "🐺"),
            ("Carcassonne XIII", "🐤"),
            ("Albi RL XIII", "🐯"),
            ("GRIZZLIES - XIII LIMOUXIN", "🐻"),
            ("Pia XIII Baroudeur", "🤠"),
            ("Saint-Estève XIII Catalan", "🐲"),
            ("Toulouse Olympique Elite", "✈️"),
            ("FC Lézignan XIII", "🟢"),
            ("Villeneuve XIII RL", "🐆"),
            ("RC ST GAUDENS", "🐾"),
        ];

        for (name, expected_emoji) in expected_clubs {
            let (canonical, emoji) = get_club_display(name);
            assert_eq!(emoji, expected_emoji, "Emoji mismatch for {}", name);
            assert!(!canonical.is_empty());
        }
    }

    #[test]
    fn test_aliases_and_case_insensitivity() {
        let (name1, emoji1) = get_club_display("les bisons");
        assert_eq!(emoji1, "🦬");
        assert_eq!(name1, "SO Avignon XIII");

        let (name2, emoji2) = get_club_display("LE XIII BAROUDEUR DE PIA");
        assert_eq!(emoji2, "🤠");
        assert_eq!(name2, "Pia XIII Baroudeur");

        let (_name3, emoji3) = get_club_display("st esteve");
        assert_eq!(emoji3, "🐲");

        let (name4, emoji4) = get_club_display("Equipe Inconnue");
        assert_eq!(emoji4, "🏉");
        assert_eq!(name4, "Equipe Inconnue");
    }
}
