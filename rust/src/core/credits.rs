//! In-game credits. Mirrors `docs/CREDITS.md` and `godot/music/CREDITS.md`; the tests
//! below fail when an entry here is missing from those files.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreditSection {
    Music,
    Art,
    Sound,
    Tech,
}

impl CreditSection {
    pub fn title(self) -> &'static str {
        match self {
            CreditSection::Music => "Music",
            CreditSection::Art => "Art and fonts",
            CreditSection::Sound => "Sound effects",
            CreditSection::Tech => "Built with",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Credit {
    pub section: CreditSection,
    pub title: &'static str,
    pub author: &'static str,
    pub license: &'static str,
    pub source: &'static str,
    /// Attribution text the license requires shown verbatim (empty if none).
    pub attribution: &'static str,
}

const fn credit(
    section: CreditSection,
    title: &'static str,
    author: &'static str,
    license: &'static str,
    source: &'static str,
) -> Credit {
    Credit {
        section,
        title,
        author,
        license,
        source,
        attribution: "",
    }
}

use CreditSection::{Art, Music, Sound, Tech};

pub const CREDITS: &[Credit] = &[
    Credit {
        attribution: "\"Ouroboros\" Kevin MacLeod (incompetech.com)\nLicensed under Creative Commons: By Attribution 4.0 License\nhttp://creativecommons.org/licenses/by/4.0/",
        ..credit(
            Music,
            "Ouroboros",
            "Kevin MacLeod",
            "CC BY 4.0",
            "incompetech.com",
        )
    },
    Credit {
        attribution: "\"Voxel Revolution\" Kevin MacLeod (incompetech.com)\nLicensed under Creative Commons: By Attribution 4.0 License\nhttp://creativecommons.org/licenses/by/4.0/",
        ..credit(
            Music,
            "Voxel Revolution",
            "Kevin MacLeod",
            "CC BY 4.0",
            "incompetech.com",
        )
    },
    credit(
        Music,
        "Wonders of the Earth",
        "Grand Project",
        "Pixabay Content License",
        "pixabay.com/music",
    ),
    credit(
        Music,
        "Celtic",
        "Alex Morgan",
        "Pixabay Content License",
        "pixabay.com/music",
    ),
    credit(
        Music,
        "Surf Rock",
        "Alex Morgan",
        "Pixabay Content License",
        "pixabay.com/music",
    ),
    credit(
        Music,
        "Las Huevas (en vivo)",
        "Banzai FC ft. Wos",
        "Used with permission",
        "live at Centro Cultural Konex, 2017",
    ),
    credit(
        Art,
        "Shape Characters",
        "Kenney",
        "CC0 1.0",
        "kenney.nl/assets/shape-characters",
    ),
    credit(
        Art,
        "Particle Pack",
        "Kenney",
        "CC0 1.0",
        "kenney.nl/assets/particle-pack",
    ),
    credit(
        Art,
        "Kenney Fonts",
        "Kenney",
        "CC0 1.0",
        "kenney.nl/assets/kenney-fonts",
    ),
    credit(
        Art,
        "UI Pack",
        "Kenney",
        "CC0 1.0",
        "kenney.nl/assets/ui-pack",
    ),
    credit(
        Art,
        "Input Prompts",
        "Kenney",
        "CC0 1.0",
        "kenney.nl/assets/input-prompts",
    ),
    credit(
        Art,
        "Simple Space",
        "Kenney",
        "CC0 1.0",
        "kenney.nl/assets/simple-space",
    ),
    Credit {
        attribution: "Icons from Game-icons.net by Lorc, Delapouite, Skoll and Sbed\nLicensed under Creative Commons: By Attribution 3.0 License\nhttp://creativecommons.org/licenses/by/3.0/",
        ..credit(
            Art,
            "Game-icons.net",
            "Lorc, Delapouite, Skoll and Sbed",
            "CC BY 3.0",
            "game-icons.net",
        )
    },
    credit(
        Sound,
        "Interface Sounds",
        "Kenney",
        "CC0 1.0",
        "kenney.nl/assets/interface-sounds",
    ),
    credit(
        Sound,
        "Digital Audio",
        "Kenney",
        "CC0 1.0",
        "kenney.nl/assets/digital-audio",
    ),
    credit(
        Sound,
        "Impact Sounds",
        "Kenney",
        "CC0 1.0",
        "kenney.nl/assets/impact-sounds",
    ),
    credit(
        Sound,
        "Sci-fi Sounds",
        "Kenney",
        "CC0 1.0",
        "kenney.nl/assets/sci-fi-sounds",
    ),
    credit(
        Tech,
        "Godot Engine",
        "Juan Linietsky, Ariel Manzur and contributors",
        "MIT",
        "godotengine.org/license",
    ),
    credit(
        Tech,
        "godot-rust",
        "godot-rust contributors",
        "MPL 2.0",
        "godot-rust.github.io",
    ),
];

pub const SECTION_ORDER: [CreditSection; 4] = [Music, Art, Sound, Tech];

#[cfg(test)]
mod tests {
    use super::*;

    const DOCS: &str = include_str!("../../../docs/CREDITS.md");
    const MUSIC_DOCS: &str = include_str!("../../../godot/music/CREDITS.md");

    #[test]
    fn every_asset_credit_is_documented() {
        let docs = format!("{DOCS}\n{MUSIC_DOCS}");
        for credit in CREDITS.iter().filter(|c| c.section != Tech) {
            assert!(docs.contains(credit.title), "{} missing", credit.title);
            assert!(docs.contains(credit.author), "{} missing", credit.author);
            assert!(docs.contains(credit.license), "{} missing", credit.license);
        }
    }

    #[test]
    fn cc_by_tracks_carry_their_attribution() {
        let mut cc_by = 0;
        for credit in CREDITS.iter().filter(|c| c.license.starts_with("CC BY")) {
            cc_by += 1;
            for line in credit.attribution.lines() {
                assert!(DOCS.contains(line), "{line:?} not in docs/CREDITS.md");
            }
            assert!(credit.attribution.contains(credit.title));
        }
        assert_eq!(cc_by, 3);
    }

    #[test]
    fn every_music_file_is_credited() {
        // Music table rows start with a backticked file or song id.
        for line in MUSIC_DOCS.lines().chain(DOCS.lines()) {
            let Some(rest) = line.strip_prefix("| `") else {
                continue;
            };
            let id = rest.split('`').next().unwrap_or("");
            if id.is_empty() || id.contains("song id") {
                continue;
            }
            let title = line.split('|').nth(2).unwrap_or("").trim();
            assert!(
                CREDITS
                    .iter()
                    .any(|c| c.section == Music && c.title == title),
                "{title} ({id}) not in the in-game credits"
            );
        }
    }
}
