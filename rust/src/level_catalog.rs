//! The ordered, static list of levels: one song each, increasing difficulty.
//!
//! Tune a level's hazards here through its `pattern_pool` (kinds, weights, sections,
//! telegraph/duration/coverage) and its choreography through `phrases`;
//! `core::chart_gen` turns both into a chart. See `docs/spec/levels.md`.

use crate::core::analysis::SectionType;
use crate::core::chart::EventKind;
use crate::core::chart_gen::{
    Cue, CueAction, EnemyEntry, LevelSpec, Palette, PatternEntry, Phrase, PhraseEntry, Rgb,
};
use crate::util::dict_set;
use godot::prelude::*;
use serde::Deserialize;

/// Lyric cues of Las Huevas (see the file's `note`).
const LAS_HUEVAS_CUES: &str = include_str!("../../godot/music/las-huevas.cues.json");
/// Enemy kinds that come in from outside the arena (chasers).
const OUTSIDE_KINDS: [&str; 1] = ["cohete"];
/// Enemy kinds that only make sense next to others (they never arrive alone).
const SUPPORT_KINDS: [&str; 1] = ["hermanos"];

const STATIC_SHOOTER: &str = "res://scenes/static_shooter_enemy.tscn";
const SHOTGUN: &str = "res://scenes/shotgun_enemy.tscn";
const TURRET: &str = "res://scenes/turret_enemy.tscn";
const RUNNER: &str = "res://scenes/runner_enemy.tscn";
const MINE_LAYER: &str = "res://scenes/mine_layer_enemy.tscn";
const HOPPER: &str = "res://scenes/hopper_enemy.tscn";
const PULSER: &str = "res://scenes/pulser_enemy.tscn";
const BOUNCER: &str = "res://scenes/bouncer_enemy.tscn";
const SPLITTER: &str = "res://scenes/splitter_enemy.tscn";
const DASHER: &str = "res://scenes/dasher_enemy.tscn";
const CHAMELEON: &str = "res://scenes/chameleon_enemy.tscn";
const LANCER: &str = "res://scenes/lancer_enemy.tscn";
const WARDEN: &str = "res://scenes/warden_enemy.tscn";
// Las Huevas: one enemy per lyric reference.
const FOCO: &str = "res://scenes/foco_enemy.tscn";
const PAPA: &str = "res://scenes/papa_enemy.tscn";
const SABLE: &str = "res://scenes/sable_enemy.tscn";
const BIRRA: &str = "res://scenes/birra_enemy.tscn";
const CAJA: &str = "res://scenes/caja_enemy.tscn";
const CORAZON: &str = "res://scenes/corazon_enemy.tscn";
const FIERA: &str = "res://scenes/fiera_enemy.tscn";
const PASTILLA: &str = "res://scenes/pastilla_enemy.tscn";
const MAESTRO: &str = "res://scenes/maestro_enemy.tscn";
const GLOBO: &str = "res://scenes/globo_enemy.tscn";
const PELOTA: &str = "res://scenes/pelota_enemy.tscn";
const MANO_DE_DIOS: &str = "res://scenes/mano_de_dios_enemy.tscn";
const HUEVO: &str = "res://scenes/huevo_enemy.tscn";
const JERINGA: &str = "res://scenes/jeringa_enemy.tscn";
const OVEJA: &str = "res://scenes/oveja_enemy.tscn";
const ABEJA: &str = "res://scenes/abeja_enemy.tscn";
const HERMANOS: &str = "res://scenes/hermanos_enemy.tscn";
const COHETE: &str = "res://scenes/cohete_enemy.tscn";
const MICROFONO: &str = "res://scenes/microfono_enemy.tscn";
const GOTA: &str = "res://scenes/gota_enemy.tscn";
const LIMON: &str = "res://scenes/limon_enemy.tscn";

use SectionType::{Breakdown, Build, Intro, Main, Outro};

/// One enemy type as the gym lists it. `rule` is the one-line behavior summary from
/// `docs/spec/enemies.md`.
pub struct EnemyKind {
    pub id: &'static str,
    pub name: &'static str,
    pub scene: &'static str,
    pub sprite: &'static str,
    pub rule: &'static str,
}

const SPRITES: &str = "res://assets/kenney_simple-space/";
const ICONS: &str = "res://assets/game-icons/";

/// Every spawnable enemy type, in the order the levels introduce them. Pieces (the
/// Splitter's, the Gota's droplets, the Huevo's chicks) are left out: they only come
/// from their parent.
pub const ENEMY_KINDS: [EnemyKind; 34] = [
    EnemyKind {
        id: "static_shooter",
        name: "Static shooter",
        scene: STATIC_SHOOTER,
        sprite: "enemy_C.png",
        rule: "One aimed shot each bar",
    },
    EnemyKind {
        id: "turret",
        name: "Turret",
        scene: TURRET,
        sprite: "enemy_E.png",
        rule: "Four shots, cardinal then diagonal",
    },
    EnemyKind {
        id: "pulser",
        name: "Pulser",
        scene: PULSER,
        sprite: "enemy_D.png",
        rule: "Ring of shots with a turning gap",
    },
    EnemyKind {
        id: "hopper",
        name: "Hopper",
        scene: HOPPER,
        sprite: "enemy_B.png",
        rule: "Hops at you every 2 beats",
    },
    EnemyKind {
        id: "runner",
        name: "Runner",
        scene: RUNNER,
        sprite: "ship_E.png",
        rule: "Surges at a mismatched player each beat",
    },
    EnemyKind {
        id: "bouncer",
        name: "Bouncer",
        scene: BOUNCER,
        sprite: "meteor_squareDetailedLarge.png",
        rule: "One diagonal step per beat",
    },
    EnemyKind {
        id: "splitter",
        name: "Splitter",
        scene: SPLITTER,
        sprite: "enemy_A.png",
        rule: "Splits into two differently shielded pieces",
    },
    EnemyKind {
        id: "shotgun",
        name: "Shotgun",
        scene: SHOTGUN,
        sprite: "ship_sidesB.png",
        rule: "Chases; 3-shot fan on the off-bar",
    },
    EnemyKind {
        id: "dasher",
        name: "Dasher",
        scene: DASHER,
        sprite: "ship_G.png",
        rule: "Shows a lane, then dashes along it",
    },
    EnemyKind {
        id: "chameleon",
        name: "Chameleon",
        scene: CHAMELEON,
        sprite: "ship_J.png",
        rule: "Shield colors rotate every 2 bars",
    },
    EnemyKind {
        id: "mine_layer",
        name: "Mine layer",
        scene: MINE_LAYER,
        sprite: "ship_sidesA.png",
        rule: "Chases slowly, drops a mine each bar",
    },
    EnemyKind {
        id: "lancer",
        name: "Lancer",
        scene: LANCER,
        sprite: "ship_L.png",
        rule: "Aims a beam for 2 beats, then fires",
    },
    EnemyKind {
        id: "warden",
        name: "Warden",
        scene: WARDEN,
        sprite: "ship_sidesD.png",
        rule: "Wards nearby enemies in its color",
    },
    EnemyKind {
        id: "foco",
        name: "Foco",
        scene: FOCO,
        sprite: "game-icons/light_bulb.png",
        rule: "Flashes a light ring with two gaps each bar",
    },
    EnemyKind {
        id: "papa",
        name: "Papa",
        scene: PAPA,
        sprite: "game-icons/potato.png",
        rule: "Throws three potatoes every 2 beats",
    },
    EnemyKind {
        id: "sable",
        name: "Sable",
        scene: SABLE,
        sprite: "game-icons/light_sabers.png",
        rule: "Twin blades snap round on every beat",
    },
    EnemyKind {
        id: "birra",
        name: "Birra",
        scene: BIRRA,
        sprite: "game-icons/beer_bottle.png",
        rule: "Bounces and sprays foam four ways",
    },
    EnemyKind {
        id: "caja",
        name: "Caja",
        scene: CAJA,
        sprite: "game-icons/drum.png",
        rule: "Snare rings on beats 2 and 4",
    },
    EnemyKind {
        id: "corazon",
        name: "Corazón",
        scene: CORAZON,
        sprite: "game-icons/heart_beats.png",
        rule: "Chases; a heartbeat ring every 2 beats",
    },
    EnemyKind {
        id: "fiera",
        name: "Fiera",
        scene: FIERA,
        sprite: "game-icons/fangs.png",
        rule: "Lunges and bites every 2 beats",
    },
    EnemyKind {
        id: "pastilla",
        name: "Pastilla",
        scene: PASTILLA,
        sprite: "game-icons/pill.png",
        rule: "Shield swaps color every bar; aimed pills",
    },
    EnemyKind {
        id: "maestro",
        name: "Maestro",
        scene: MAESTRO,
        sprite: "game-icons/hood.png",
        rule: "Tiny and green: hops at you every beat",
    },
    EnemyKind {
        id: "globo",
        name: "Globo",
        scene: GLOBO,
        sprite: "game-icons/air_balloon.png",
        rule: "Circles the arena, dropping sandbags",
    },
    EnemyKind {
        id: "pelota",
        name: "Pelota",
        scene: PELOTA,
        sprite: "game-icons/soccer_ball.png",
        rule: "Dribbles a long diagonal step every beat",
    },
    EnemyKind {
        id: "mano_de_dios",
        name: "Mano de Dios",
        scene: MANO_DE_DIOS,
        sprite: "game-icons/hand_of_god.png",
        rule: "Falls from the sky every bar",
    },
    EnemyKind {
        id: "huevo",
        name: "Huevo",
        scene: HUEVO,
        sprite: "game-icons/egg.png",
        rule: "Hatches three chicks unless cracked first",
    },
    EnemyKind {
        id: "jeringa",
        name: "Jeringa",
        scene: JERINGA,
        sprite: "game-icons/syringe.png",
        rule: "Aims a lane, then injects along it",
    },
    EnemyKind {
        id: "oveja",
        name: "Oveja",
        scene: OVEJA,
        sprite: "game-icons/sheep.png",
        rule: "Slow, follows the flock to you",
    },
    EnemyKind {
        id: "abeja",
        name: "Abeja",
        scene: ABEJA,
        sprite: "game-icons/bee.png",
        rule: "Swarms fast; one hit pops it",
    },
    EnemyKind {
        id: "hermanos",
        name: "Hermanos",
        scene: HERMANOS,
        sprite: "game-icons/shaking_hands.png",
        rule: "Wards nearby enemies in its color",
    },
    EnemyKind {
        id: "cohete",
        name: "Cohete",
        scene: COHETE,
        sprite: "game-icons/rocket.png",
        rule: "Blasts in at a mismatched player",
    },
    EnemyKind {
        id: "microfono",
        name: "Micrófono",
        scene: MICROFONO,
        sprite: "game-icons/microphone.png",
        rule: "Ring of words with a turning gap",
    },
    EnemyKind {
        id: "gota",
        name: "Gota",
        scene: GOTA,
        sprite: "game-icons/drop.png",
        rule: "Splits into three droplets",
    },
    EnemyKind {
        id: "limon",
        name: "Limón",
        scene: LIMON,
        sprite: "game-icons/lemon.png",
        rule: "Bursts into juice unless squeezed first",
    },
];

impl EnemyKind {
    /// `sprite` is a Kenney simple-space file name, or `game-icons/<name>.png`.
    pub fn sprite_path(&self) -> String {
        match self.sprite.strip_prefix("game-icons/") {
            Some(icon) => format!("{ICONS}{icon}"),
            None => format!("{SPRITES}{}", self.sprite),
        }
    }
}

#[derive(Deserialize)]
struct CueSheet {
    cues: Vec<CueRecord>,
}

/// One cue as written in a `<song>.cues.json`: an `enemy` (kind id), a `hazard`
/// (event kind name) or neither (caption only).
#[derive(Deserialize)]
struct CueRecord {
    t: f64,
    #[serde(default)]
    caption: Option<String>,
    #[serde(default)]
    enemy: Option<String>,
    #[serde(default)]
    hazard: Option<String>,
    #[serde(default)]
    count: Option<u32>,
    #[serde(default)]
    life: f64,
    x: f32,
    y: f32,
    #[serde(default)]
    variant: u32,
    #[serde(default)]
    angle: f32,
}

/// Adds a cue sheet to `spec`: its cues, their captions, and an enemy pool entry per
/// enemy kind used (in order of first use). Hazards must be in the pattern pool.
pub fn apply_cue_sheet(spec: &mut LevelSpec, json: &str) -> Result<(), String> {
    let sheet: CueSheet =
        serde_json::from_str(json).map_err(|err| format!("invalid cue sheet: {err}"))?;
    for record in sheet.cues {
        let action = match (&record.enemy, &record.hazard) {
            (Some(id), None) => {
                let kind = ENEMY_KINDS
                    .iter()
                    .find(|kind| kind.id == id)
                    .ok_or_else(|| format!("unknown enemy '{id}' at {}s", record.t))?;
                let variant = match spec.enemy_pool.iter().position(|e| e.scene == kind.scene) {
                    Some(index) => index,
                    None => {
                        let mut entry =
                            EnemyEntry::new(kind.scene, OUTSIDE_KINDS.contains(&kind.id), 1.0);
                        if SUPPORT_KINDS.contains(&kind.id) {
                            entry = entry.supporting();
                        }
                        spec.enemy_pool.push(entry);
                        spec.enemy_pool.len() - 1
                    }
                };
                CueAction::Enemy {
                    variant: variant as u32,
                    count: record.count.unwrap_or(1),
                    life_beats: record.life,
                }
            }
            (None, Some(name)) => {
                let kind = EventKind::from_name(name)
                    .filter(|kind| spec.pattern(*kind).is_some())
                    .ok_or_else(|| {
                        format!("hazard '{name}' at {}s is not in the pool", record.t)
                    })?;
                CueAction::Hazard {
                    kind,
                    variant: record.variant,
                    angle: record.angle,
                }
            }
            (None, None) => CueAction::Caption,
            (Some(_), Some(_)) => {
                return Err(format!(
                    "cue at {}s has both an enemy and a hazard",
                    record.t
                ));
            }
        };
        let caption = record.caption.map(|text| {
            let index = spec.captions.iter().position(|c| *c == text);
            index.unwrap_or_else(|| {
                spec.captions.push(text);
                spec.captions.len() - 1
            }) as u32
        });
        spec.cues.push(Cue {
            time: record.t,
            action,
            caption,
            x: record.x.clamp(0.0, 1.0),
            y: record.y.clamp(0.0, 1.0),
        });
    }
    Ok(())
}

fn level(
    id: &str,
    title: &str,
    artist: &str,
    difficulty: u8,
    seed: u64,
    palette: Palette,
) -> LevelSpec {
    LevelSpec {
        id: id.to_string(),
        title: title.to_string(),
        artist: artist.to_string(),
        music_path: format!("res://music/{id}.ogg"),
        analysis_path: format!("res://music/{id}.analysis.json"),
        difficulty,
        seed,
        palette,
        pattern_pool: Vec::new(),
        enemy_pool: Vec::new(),
        phrases: Vec::new(),
        density: 1.0,
        tutorial: false,
        finale: false,
        cues: Vec::new(),
        captions: Vec::new(),
    }
}

/// All levels in play order. Level 1 is the gentle tutorial; the last ends with the
/// finale set piece. Each level has its own palette, signature phrases (weighted up in
/// `phrases`), hazard pool and enemy mix.
pub fn all_levels() -> Vec<LevelSpec> {
    // 1. Tutorial: pulses on the grid, a few lasers, gapped rings. Turrets and shooters.
    let mut wonders = level(
        "wonders-of-the-earth",
        "Wonders of the Earth",
        "Grand Project",
        1,
        0x5EED_0001,
        Palette {
            bg: Rgb::new(0.04, 0.07, 0.12),
            accent: Rgb::new(0.30, 0.85, 1.0),
            danger: Rgb::new(1.0, 0.25, 0.6),
        },
    );
    wonders.tutorial = true;
    wonders.density = 0.8;
    wonders.pattern_pool = vec![
        PatternEntry::new(EventKind::Pulse, 3.0),
        PatternEntry::new(EventKind::Laser, 1.0).in_sections(&[Build, Breakdown, Outro]),
        PatternEntry::new(EventKind::BulletRing, 0.6)
            .in_sections(&[Build])
            .count(8, 10),
    ];
    wonders.phrases = vec![
        PhraseEntry::new(Phrase::PulseGrid, 2.0),
        PhraseEntry::new(Phrase::Breather, 1.0),
        PhraseEntry::new(Phrase::LaserCallResponse, 1.0),
        PhraseEntry::new(Phrase::RingsOnKicks, 2.0),
    ];
    wonders.enemy_pool = vec![
        EnemyEntry::new(PULSER, false, 1.2).introduced(),
        EnemyEntry::new(HOPPER, false, 1.2).introduced(),
        EnemyEntry::new(STATIC_SHOOTER, false, 1.5),
        EnemyEntry::new(TURRET, false, 0.8),
    ];

    // 2. Voxel Revolution: blocky geometry. Spikes from the sides and walls with gaps.
    let mut voxel = level(
        "voxel-revolution",
        "Voxel Revolution",
        "Kevin MacLeod",
        2,
        0x5EED_0004,
        Palette {
            bg: Rgb::new(0.08, 0.05, 0.14),
            accent: Rgb::new(0.85, 1.0, 0.25),
            danger: Rgb::new(1.0, 0.2, 0.75),
        },
    );
    voxel.density = 1.05;
    voxel.pattern_pool = vec![
        PatternEntry::new(EventKind::Pulse, 2.0),
        PatternEntry::new(EventKind::Laser, 1.5),
        PatternEntry::new(EventKind::Spikes, 1.2).count(6, 8),
        PatternEntry::new(EventKind::Wall, 1.0).in_sections(&[Build, Main]),
        PatternEntry::new(EventKind::BulletRing, 0.6)
            .in_sections(&[Main])
            .count(8, 12),
    ];
    voxel.phrases = vec![
        PhraseEntry::new(Phrase::SpikeSides, 2.5),
        PhraseEntry::new(Phrase::SweepingWall, 2.0),
        PhraseEntry::new(Phrase::PulseGrid, 1.0),
        PhraseEntry::new(Phrase::LaserCallResponse, 1.0),
        PhraseEntry::new(Phrase::RingsOnKicks, 0.5),
        PhraseEntry::new(Phrase::Breather, 1.0),
    ];
    voxel.enemy_pool = vec![
        EnemyEntry::new(BOUNCER, false, 1.3).introduced(),
        EnemyEntry::new(SPLITTER, false, 1.0).introduced(),
        EnemyEntry::new(STATIC_SHOOTER, false, 1.0),
        EnemyEntry::new(RUNNER, true, 1.0),
        EnemyEntry::new(TURRET, false, 0.7),
        EnemyEntry::new(HOPPER, false, 0.8),
        EnemyEntry::new(PULSER, false, 0.7),
    ];

    // 3. Celtic: a reel. Rings on every kick and lasers trading sides, with sweeps.
    let mut celtic = level(
        "celtic",
        "Celtic",
        "Alex Morgan",
        3,
        0x5EED_0002,
        Palette {
            bg: Rgb::new(0.05, 0.10, 0.06),
            accent: Rgb::new(0.55, 1.0, 0.35),
            danger: Rgb::new(1.0, 0.3, 0.5),
        },
    );
    celtic.density = 0.9;
    celtic.pattern_pool = vec![
        PatternEntry::new(EventKind::Pulse, 2.0),
        PatternEntry::new(EventKind::Laser, 2.0),
        PatternEntry::new(EventKind::LaserSweep, 1.0).in_sections(&[Build, Main]),
        PatternEntry::new(EventKind::BulletRing, 1.5),
        PatternEntry::new(EventKind::Wall, 0.8).in_sections(&[Main]),
        PatternEntry::new(EventKind::Spikes, 1.0).in_sections(&[Main, Breakdown, Outro]),
        PatternEntry::new(EventKind::Spiral, 0.6).in_sections(&[Build]),
    ];
    celtic.phrases = vec![
        PhraseEntry::new(Phrase::RingsOnKicks, 2.5),
        PhraseEntry::new(Phrase::LaserCallResponse, 2.0),
        PhraseEntry::new(Phrase::SweepCross, 1.0),
        PhraseEntry::new(Phrase::SweepingWall, 0.8),
        PhraseEntry::new(Phrase::SpikeSides, 0.8),
        PhraseEntry::new(Phrase::SpiralRiser, 1.0),
        PhraseEntry::new(Phrase::PulseGrid, 1.0),
        PhraseEntry::new(Phrase::Breather, 1.0),
        PhraseEntry::new(Phrase::Scatter, 0.5),
    ];
    celtic.enemy_pool = vec![
        EnemyEntry::new(DASHER, false, 1.2).introduced(),
        EnemyEntry::new(CHAMELEON, false, 1.0).introduced(),
        EnemyEntry::new(STATIC_SHOOTER, false, 0.7),
        EnemyEntry::new(SHOTGUN, true, 0.9),
        EnemyEntry::new(RUNNER, true, 0.8),
        EnemyEntry::new(TURRET, false, 0.5),
        EnemyEntry::new(HOPPER, false, 0.8),
        EnemyEntry::new(BOUNCER, false, 0.8),
        EnemyEntry::new(SPLITTER, false, 0.6),
    ];

    // 4. Ouroboros: the serpent. Spirals, rotating sweeps and bombs; mine layers.
    let mut ouroboros = level(
        "ouroboros",
        "Ouroboros",
        "Kevin MacLeod",
        4,
        0x5EED_0005,
        Palette {
            bg: Rgb::new(0.03, 0.07, 0.09),
            accent: Rgb::new(1.0, 0.78, 0.25),
            danger: Rgb::new(1.0, 0.15, 0.45),
        },
    );
    ouroboros.density = 1.3;
    ouroboros.pattern_pool = vec![
        PatternEntry::new(EventKind::Pulse, 1.5),
        PatternEntry::new(EventKind::Laser, 1.5),
        PatternEntry::new(EventKind::LaserSweep, 1.5),
        PatternEntry::new(EventKind::Spiral, 1.5).in_sections(&[Build, Main]),
        PatternEntry::new(EventKind::BulletRing, 1.0),
        PatternEntry::new(EventKind::Bomb, 1.0),
        PatternEntry::new(EventKind::Spikes, 0.6),
        PatternEntry::new(EventKind::Wall, 0.6).in_sections(&[Main]),
    ];
    ouroboros.phrases = vec![
        PhraseEntry::new(Phrase::SpiralRiser, 2.5),
        PhraseEntry::new(Phrase::SweepCross, 2.0),
        PhraseEntry::new(Phrase::BombPairs, 1.5),
        PhraseEntry::new(Phrase::RingsOnKicks, 1.0),
        PhraseEntry::new(Phrase::LaserCallResponse, 1.0),
        PhraseEntry::new(Phrase::SweepingWall, 0.6),
        PhraseEntry::new(Phrase::SpikeSides, 0.6),
        PhraseEntry::new(Phrase::PulseGrid, 0.3),
        PhraseEntry::new(Phrase::Breather, 1.0),
        PhraseEntry::new(Phrase::Scatter, 0.5),
    ];
    ouroboros.enemy_pool = vec![
        EnemyEntry::new(LANCER, false, 1.2).introduced(),
        EnemyEntry::new(WARDEN, false, 0.8)
            .introduced()
            .supporting(),
        EnemyEntry::new(TURRET, false, 0.8),
        EnemyEntry::new(MINE_LAYER, true, 0.9),
        EnemyEntry::new(SHOTGUN, true, 0.8),
        EnemyEntry::new(RUNNER, true, 0.6),
        EnemyEntry::new(PULSER, false, 0.7),
        EnemyEntry::new(DASHER, false, 0.9),
        EnemyEntry::new(CHAMELEON, false, 0.6),
    ];

    // 5. Surf Rock (final): everything, aimed barrages on the snare, bomb pairs, and the
    // finale set piece before the outro.
    let mut surf = level(
        "surf-rock",
        "Surf Rock",
        "Alex Morgan",
        5,
        0x5EED_0003,
        Palette {
            bg: Rgb::new(0.12, 0.04, 0.08),
            accent: Rgb::new(1.0, 0.6, 0.2),
            danger: Rgb::new(1.0, 0.1, 0.42),
        },
    );
    surf.finale = true;
    surf.pattern_pool = vec![
        PatternEntry::new(EventKind::Pulse, 1.5),
        PatternEntry::new(EventKind::Laser, 2.0),
        PatternEntry::new(EventKind::LaserSweep, 1.5),
        PatternEntry::new(EventKind::BulletRing, 1.5),
        PatternEntry::new(EventKind::Spiral, 1.0).in_sections(&[Main, Build]),
        PatternEntry::new(EventKind::Wall, 1.0).in_sections(&[Main, Build, Outro]),
        PatternEntry::new(EventKind::Bomb, 0.8),
        PatternEntry::new(EventKind::Spikes, 1.0),
        PatternEntry::new(EventKind::Barrage, 1.0).in_sections(&[Main, Build]),
    ];
    surf.phrases = vec![
        PhraseEntry::new(Phrase::BarrageSnares, 2.5),
        PhraseEntry::new(Phrase::BombPairs, 1.5),
        PhraseEntry::new(Phrase::SweepingWall, 1.5),
        PhraseEntry::new(Phrase::RingsOnKicks, 1.2),
        PhraseEntry::new(Phrase::LaserCallResponse, 1.2),
        PhraseEntry::new(Phrase::SweepCross, 1.0),
        PhraseEntry::new(Phrase::SpikeSides, 1.0),
        PhraseEntry::new(Phrase::SpiralRiser, 1.0),
        PhraseEntry::new(Phrase::PulseGrid, 0.6),
        PhraseEntry::new(Phrase::Breather, 1.0),
        PhraseEntry::new(Phrase::Scatter, 0.6),
    ];
    surf.enemy_pool = vec![
        EnemyEntry::new(STATIC_SHOOTER, false, 0.6),
        EnemyEntry::new(SHOTGUN, true, 0.8),
        EnemyEntry::new(RUNNER, true, 0.8),
        EnemyEntry::new(TURRET, false, 0.6),
        EnemyEntry::new(MINE_LAYER, true, 0.6),
        EnemyEntry::new(HOPPER, false, 0.8),
        EnemyEntry::new(PULSER, false, 0.7),
        EnemyEntry::new(BOUNCER, false, 0.8),
        EnemyEntry::new(SPLITTER, false, 0.7),
        EnemyEntry::new(DASHER, false, 1.0),
        EnemyEntry::new(CHAMELEON, false, 0.8),
        EnemyEntry::new(LANCER, false, 1.0),
        EnemyEntry::new(WARDEN, false, 0.6).supporting(),
    ];

    // 6. Las Huevas (bonus): a live, improvised freestyle. Every enemy is a lyric
    // reference arriving on its word (`las-huevas.cues.json`); the band's jams between
    // the verses carry the hazards, and "se vienen los climas" opens the finale.
    let mut huevas = level(
        "las-huevas",
        "Las Huevas (en vivo)",
        "Banzai FC ft. Wos",
        5,
        0x5EED_0006,
        Palette {
            bg: Rgb::new(0.04, 0.06, 0.13),
            accent: Rgb::new(0.48, 0.76, 1.0),
            danger: Rgb::new(1.0, 0.25, 0.5),
        },
    );
    huevas.finale = true;
    huevas.density = 1.1;
    huevas.pattern_pool = vec![
        PatternEntry::new(EventKind::Pulse, 2.0),
        PatternEntry::new(EventKind::Laser, 1.5),
        PatternEntry::new(EventKind::Barrage, 1.5).in_sections(&[Main, Build]),
        PatternEntry::new(EventKind::BulletRing, 1.2),
        PatternEntry::new(EventKind::Spikes, 1.0),
        PatternEntry::new(EventKind::Wall, 0.8).in_sections(&[Main, Build, Outro]),
        PatternEntry::new(EventKind::LaserSweep, 0.8).in_sections(&[Main, Build]),
        PatternEntry::new(EventKind::Bomb, 0.6).in_sections(&[Main]),
    ];
    huevas.phrases = vec![
        PhraseEntry::new(Phrase::BarrageSnares, 2.5),
        PhraseEntry::new(Phrase::RingsOnKicks, 1.5),
        PhraseEntry::new(Phrase::PulseGrid, 1.2),
        PhraseEntry::new(Phrase::SpikeSides, 1.0),
        PhraseEntry::new(Phrase::LaserCallResponse, 1.0),
        PhraseEntry::new(Phrase::SweepingWall, 0.8),
        PhraseEntry::new(Phrase::BombPairs, 0.6),
        PhraseEntry::new(Phrase::Breather, 1.0),
    ];
    // Compiled in and checked by `las_huevas_cues_load`.
    apply_cue_sheet(&mut huevas, LAS_HUEVAS_CUES).expect("las-huevas cue sheet");

    let mut levels = vec![wonders, voxel, celtic, ouroboros, surf, huevas];
    // Intro sections stay pulse-only on every level so the first hits are readable.
    for spec in &mut levels {
        for entry in &mut spec.pattern_pool {
            if entry.sections.is_empty() && entry.kind != EventKind::Pulse {
                entry.sections = vec![Build, Main, Breakdown, Outro];
            }
        }
        if !spec.pattern_pool.iter().any(|e| e.allowed_in(Intro)) {
            spec.pattern_pool[0].sections.clear();
        }
    }
    levels
}

pub fn find_level(id: &str) -> Option<LevelSpec> {
    all_levels().into_iter().find(|spec| spec.id == id)
}

pub fn first_level_id() -> String {
    all_levels()
        .first()
        .map(|spec| spec.id.clone())
        .unwrap_or_default()
}

pub fn rgb_to_color(rgb: Rgb) -> Color {
    Color::from_rgb(rgb.r, rgb.g, rgb.b)
}

/// Godot dictionary view of a level. Keys: `id`, `index`, `title`, `artist`,
/// `music_path`, `analysis_path`, `difficulty`, `seed`, `bg_color`, `accent_color`,
/// `danger_color`, `density`, `tutorial`, `finale`, `pattern_pool` (kind names),
/// `phrases` (phrase names), `enemy_pool` (scene paths).
pub fn level_to_dictionary(spec: &LevelSpec, index: usize) -> VarDictionary {
    let mut dict = VarDictionary::new();
    let kinds: PackedStringArray = spec
        .pattern_pool
        .iter()
        .map(|entry| GString::from(entry.kind.name()))
        .collect();
    let phrases: PackedStringArray = spec
        .phrases
        .iter()
        .map(|entry| GString::from(entry.phrase.name()))
        .collect();
    let enemies: PackedStringArray = spec
        .enemy_pool
        .iter()
        .map(|entry| GString::from(&entry.scene))
        .collect();
    dict_set(&mut dict, "id", GString::from(&spec.id));
    dict_set(&mut dict, "index", index as i64);
    dict_set(&mut dict, "title", GString::from(&spec.title));
    dict_set(&mut dict, "artist", GString::from(&spec.artist));
    dict_set(&mut dict, "music_path", GString::from(&spec.music_path));
    dict_set(
        &mut dict,
        "analysis_path",
        GString::from(&spec.analysis_path),
    );
    dict_set(&mut dict, "difficulty", spec.difficulty as i64);
    dict_set(&mut dict, "seed", spec.seed as i64);
    dict_set(&mut dict, "bg_color", rgb_to_color(spec.palette.bg));
    dict_set(&mut dict, "accent_color", rgb_to_color(spec.palette.accent));
    dict_set(&mut dict, "danger_color", rgb_to_color(spec.palette.danger));
    dict_set(&mut dict, "density", spec.density as f64);
    dict_set(&mut dict, "tutorial", spec.tutorial);
    dict_set(&mut dict, "finale", spec.finale);
    dict_set(&mut dict, "pattern_pool", kinds);
    dict_set(&mut dict, "phrases", phrases);
    dict_set(&mut dict, "enemy_pool", enemies);
    dict
}

/// Static access to the level list from Godot: `LevelCatalog.count()`,
/// `LevelCatalog.get_level(i)`, `LevelCatalog.find_level(id)`, `LevelCatalog.list()`.
#[derive(GodotClass)]
#[class(init, base = Object)]
pub struct LevelCatalog {
    base: Base<Object>,
}

#[godot_api]
impl LevelCatalog {
    #[func]
    pub fn count() -> i64 {
        all_levels().len() as i64
    }

    /// Level dictionary at `index` (empty if out of range).
    #[func]
    pub fn get_level(index: i64) -> VarDictionary {
        usize::try_from(index)
            .ok()
            .and_then(|i| all_levels().get(i).map(|spec| level_to_dictionary(spec, i)))
            .unwrap_or_default()
    }

    /// Level dictionary by id (empty if unknown).
    #[func]
    pub fn find_level(id: GString) -> VarDictionary {
        let id = id.to_string();
        all_levels()
            .iter()
            .enumerate()
            .find(|(_, spec)| spec.id == id)
            .map(|(i, spec)| level_to_dictionary(spec, i))
            .unwrap_or_default()
    }

    /// Index of a level id, or -1.
    #[func]
    pub fn index_of(id: GString) -> i64 {
        let id = id.to_string();
        all_levels()
            .iter()
            .position(|spec| spec.id == id)
            .map_or(-1, |i| i as i64)
    }

    #[func]
    pub fn list() -> Array<VarDictionary> {
        all_levels()
            .iter()
            .enumerate()
            .map(|(i, spec)| level_to_dictionary(spec, i))
            .collect()
    }

    /// Spawnable enemy types for the gym. Keys: `id`, `name`, `scene`, `sprite`, `rule`.
    #[func]
    pub fn enemy_kinds() -> Array<VarDictionary> {
        ENEMY_KINDS
            .iter()
            .map(|kind| {
                let mut dict = VarDictionary::new();
                dict_set(&mut dict, "id", GString::from(kind.id));
                dict_set(&mut dict, "name", GString::from(kind.name));
                dict_set(&mut dict, "scene", GString::from(kind.scene));
                dict_set(&mut dict, "sprite", GString::from(&kind.sprite_path()));
                dict_set(&mut dict, "rule", GString::from(kind.rule));
                dict
            })
            .collect()
    }

    #[func(rename = first_level_id)]
    pub fn first_level_id_gd() -> GString {
        GString::from(&first_level_id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::analysis::SongAnalysis;
    use crate::core::chart::{Chart, ChartEvent};
    use crate::core::chart_gen::{
        Phrase, coverage_cap, generate_chart, max_active_hazards, phrase_plan,
    };
    use crate::core::mode::DifficultyMode;
    use crate::core::test_support::REAL_ANALYSES;
    use std::collections::{HashMap, HashSet};

    fn analysis(id: &str) -> SongAnalysis {
        let (_, json) = REAL_ANALYSES.iter().find(|(name, _)| *name == id).unwrap();
        SongAnalysis::from_json(json).unwrap()
    }

    fn chart(spec: &LevelSpec) -> Chart {
        generate_chart(&analysis(&spec.id), spec, spec.seed)
    }

    #[test]
    fn levels_are_distinct_and_ordered() {
        let levels = all_levels();
        assert_eq!(levels.len(), 6);
        let ids: Vec<&str> = levels.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "wonders-of-the-earth",
                "voxel-revolution",
                "celtic",
                "ouroboros",
                "surf-rock",
                "las-huevas"
            ]
        );
        // The campaign ramps 1 to 5 and ends on the finale; the scripted bonus level
        // after it plays at the top difficulty with its own finale.
        let (campaign, bonus) = levels.split_at(5);
        assert!(
            campaign
                .windows(2)
                .all(|w| w[0].difficulty < w[1].difficulty)
        );
        assert!(levels[0].tutorial);
        assert!(campaign.last().unwrap().finale);
        assert_eq!(campaign.iter().filter(|l| l.finale).count(), 1);
        assert!(
            bonus
                .iter()
                .all(|l| l.difficulty == 5 && !l.cues.is_empty())
        );
        assert!(campaign.iter().all(|l| l.cues.is_empty()));
        let unique: HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(unique.len(), levels.len());
        let accents: HashSet<String> = levels
            .iter()
            .map(|l| format!("{:?}", l.palette.accent))
            .collect();
        assert_eq!(accents.len(), levels.len());
        for spec in &levels {
            assert!(!spec.pattern_pool.is_empty() && !spec.enemy_pool.is_empty());
            assert!(!spec.phrases.is_empty(), "{}", spec.id);
            assert!(spec.pattern_pool.iter().any(|e| e.allowed_in(Intro)));
            assert!(REAL_ANALYSES.iter().any(|(id, _)| *id == spec.id));
        }
        assert!(levels[0].pattern_pool.len() < levels[4].pattern_pool.len());
        // The final level uses every hazard kind.
        let kinds: HashSet<EventKind> = levels[4].pattern_pool.iter().map(|e| e.kind).collect();
        assert_eq!(kinds.len(), EventKind::HAZARDS.len());
    }

    /// Hazard pressure: summed `coverage x active seconds` per minute of song. Counts
    /// alone undersell levels built on fewer, longer hazards (spirals, sweeps).
    fn pressure(spec: &LevelSpec, chart: &Chart) -> f64 {
        let timing = chart.timing();
        let load: f64 = chart
            .events
            .iter()
            .filter(|e| e.kind.is_hazard())
            .map(|e| {
                let coverage = spec.pattern(e.kind).unwrap().coverage as f64;
                coverage * timing.beats_to_duration(e.params.duration_beats)
            })
            .sum();
        load / (chart.duration_seconds / 60.0)
    }

    /// Peak pressure: the busiest `PEAK_WINDOW_BARS` stretch, as coverage-seconds per
    /// minute. Ranks levels by their hardest passage, independent of long quiet intros.
    fn peak_pressure(spec: &LevelSpec, chart: &Chart) -> f64 {
        const PEAK_WINDOW_BARS: f64 = 8.0;
        let timing = chart.timing();
        let window = PEAK_WINDOW_BARS * 4.0;
        let last = chart.events.iter().map(|e| e.beat).fold(0.0, f64::max);
        let mut peak: f64 = 0.0;
        let mut start = 0.0;
        while start <= last {
            let load: f64 = chart
                .events
                .iter()
                .filter(|e| e.kind.is_hazard() && e.beat >= start && e.beat < start + window)
                .map(|e| {
                    let coverage = spec.pattern(e.kind).unwrap().coverage as f64;
                    coverage * timing.beats_to_duration(e.params.duration_beats)
                })
                .sum();
            peak = peak.max(load / (timing.beats_to_duration(window) / 60.0));
            start += 4.0;
        }
        peak
    }

    #[test]
    fn every_level_generates_a_playable_chart() {
        let mut pressures = Vec::new();
        for spec in all_levels() {
            let chart = chart(&spec);
            let hazards = chart.events.iter().filter(|e| e.kind.is_hazard()).count();
            assert!(hazards > 10, "{}: {hazards} hazards", spec.id);
            assert!(chart.count_kind(EventKind::SpawnEnemy) > 0, "{}", spec.id);
            for event in chart.events.iter().filter(|e| e.kind.is_hazard()) {
                assert!(spec.pattern(event.kind).is_some(), "{}: {event:?}", spec.id);
            }
            if spec.cues.is_empty() {
                pressures.push(peak_pressure(&spec, &chart));
            }
        }
        // Hazard pressure ramps through the campaign (scripted levels lean on enemies).
        assert!(pressures.windows(2).all(|w| w[0] < w[1]), "{pressures:?}");
    }

    /// Tuning aid: `cargo test level_stats -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn level_stats() {
        for spec in all_levels() {
            let chart = chart(&spec);
            let plan = phrase_plan(&analysis(&spec.id), &spec, spec.seed);
            let mut kinds: Vec<(EventKind, usize)> = EventKind::ALL
                .iter()
                .filter(|k| k.is_hazard() || **k == EventKind::SpawnEnemy)
                .map(|k| (*k, chart.count_kind(*k)))
                .filter(|(_, n)| *n > 0)
                .collect();
            kinds.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
            let hazards = chart.events.iter().filter(|e| e.kind.is_hazard()).count();
            println!(
                "{}: {hazards} hazards, {:.1}/min, pressure {:.2}, peak {:.2}",
                spec.id,
                hazards as f64 / (chart.duration_seconds / 60.0),
                pressure(&spec, &chart),
                peak_pressure(&spec, &chart)
            );
            println!("  kinds: {kinds:?}");
            let phrases: Vec<String> = plan
                .iter()
                .map(|(s, e, p)| format!("{}-{}:{}", s / 4, e / 4, p.name()))
                .collect();
            println!("  phrases: {}", phrases.join(" "));
        }
    }

    #[test]
    fn levels_play_their_signature_phrases() {
        for spec in all_levels() {
            let plan = phrase_plan(&analysis(&spec.id), &spec, spec.seed);
            let mut counts: HashMap<Phrase, usize> = HashMap::new();
            for (_, _, phrase) in &plan {
                *counts.entry(*phrase).or_default() += 1;
            }
            let signature = spec
                .phrases
                .iter()
                .max_by(|a, b| a.weight.total_cmp(&b.weight))
                .unwrap()
                .phrase;
            assert!(
                counts.get(&signature).copied().unwrap_or(0) >= 2,
                "{}: signature {signature:?} in {counts:?}",
                spec.id
            );
            let distinct = counts.len();
            assert!(distinct >= 3, "{}: only {distinct} phrases", spec.id);
        }
    }

    /// Never more than `max_active_hazards` at any beat, and coverage stays under the
    /// cap, for every level in every difficulty mode.
    #[test]
    fn every_level_keeps_a_safe_path_in_every_mode() {
        for spec in all_levels() {
            for mode in [
                DifficultyMode::Casual,
                DifficultyMode::Normal,
                DifficultyMode::Hardcore,
            ] {
                let mut tuned = spec.clone();
                tuned.density *= mode.density_scale();
                let chart = chart(&tuned);
                let hazards: Vec<&ChartEvent> =
                    chart.events.iter().filter(|e| e.kind.is_hazard()).collect();
                for probe in &hazards {
                    let active: Vec<&&ChartEvent> = hazards
                        .iter()
                        .filter(|e| e.beat <= probe.beat && e.end_beat() > probe.beat)
                        .collect();
                    let coverage: f32 = active
                        .iter()
                        .map(|e| spec.pattern(e.kind).unwrap().coverage)
                        .sum();
                    assert!(active.len() <= max_active_hazards(spec.difficulty));
                    assert!(coverage <= coverage_cap(spec.difficulty) + 1e-5);
                }
            }
        }
    }

    #[test]
    fn find_level_by_id() {
        assert_eq!(find_level("celtic").unwrap().difficulty, 3);
        assert_eq!(find_level("ouroboros").unwrap().difficulty, 4);
        assert!(find_level("nope").is_none());
    }

    #[test]
    fn enemy_kinds_cover_every_pool_and_exist_on_disk() {
        let godot_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../godot");
        let on_disk = |res: &str| godot_dir.join(res.trim_start_matches("res://")).is_file();
        for kind in &ENEMY_KINDS {
            assert!(on_disk(kind.scene), "missing scene {}", kind.scene);
            assert!(
                on_disk(&kind.sprite_path()),
                "missing sprite {}",
                kind.sprite
            );
            assert!(kind.scene.ends_with(&format!("/{}_enemy.tscn", kind.id)));
        }
        for spec in all_levels() {
            for entry in &spec.enemy_pool {
                assert!(
                    ENEMY_KINDS.iter().any(|kind| kind.scene == entry.scene),
                    "{} not in ENEMY_KINDS",
                    entry.scene
                );
            }
        }
    }

    #[test]
    fn las_huevas_cues_load() {
        let spec = find_level("las-huevas").unwrap();
        let enemies = spec
            .cues
            .iter()
            .filter(|c| matches!(c.action, CueAction::Enemy { .. }))
            .count();
        assert!(enemies >= 35, "{enemies} enemy cues");
        // Every lyric enemy kind is used, each once in the pool.
        let lyric_kinds = ENEMY_KINDS
            .iter()
            .filter(|k| k.sprite.starts_with("game-icons/"));
        for kind in lyric_kinds {
            assert!(
                spec.enemy_pool.iter().any(|e| e.scene == kind.scene),
                "{} has no cue",
                kind.id
            );
        }
        let scenes: HashSet<&str> = spec.enemy_pool.iter().map(|e| e.scene.as_str()).collect();
        assert_eq!(scenes.len(), spec.enemy_pool.len());
        assert!(
            spec.cues
                .iter()
                .all(|c| c.caption.is_none_or(|i| (i as usize) < spec.captions.len()))
        );
    }

    #[test]
    fn cue_sheet_errors_name_the_cue() {
        let mut spec = find_level("celtic").unwrap();
        let bad_enemy = r#"{"cues": [{"t": 3.5, "enemy": "nope", "x": 0.5, "y": 0.5}]}"#;
        assert!(
            apply_cue_sheet(&mut spec, bad_enemy)
                .unwrap_err()
                .contains("3.5")
        );
        let bad_hazard = r#"{"cues": [{"t": 4.0, "hazard": "Barrage", "x": 0.5, "y": 0.5}]}"#;
        assert!(
            apply_cue_sheet(&mut spec, bad_hazard)
                .unwrap_err()
                .contains("Barrage")
        );
        assert!(apply_cue_sheet(&mut spec, "{").is_err());
    }

    /// Scripted enemies spawn on their words (tempo map included), with their caption,
    /// group size and lifetime; generated enemies keep clear of them.
    #[test]
    fn las_huevas_enemies_arrive_on_their_words() {
        let spec = find_level("las-huevas").unwrap();
        let chart = chart(&spec);
        let timing = chart.timing();
        assert!(timing.beat_times.is_some());
        let spawns: Vec<&ChartEvent> = chart
            .events
            .iter()
            .filter(|e| e.kind == EventKind::SpawnEnemy)
            .collect();
        for cue in &spec.cues {
            let CueAction::Enemy {
                variant,
                count,
                life_beats,
            } = cue.action
            else {
                continue;
            };
            let spawn = spawns
                .iter()
                .find(|e| {
                    e.params.variant == variant
                        && (timing.beat_to_seconds(e.beat) - cue.time).abs() < 0.4
                })
                .unwrap_or_else(|| panic!("no spawn for cue at {}s", cue.time));
            assert_eq!(spawn.params.count, count);
            assert_eq!(spawn.params.duration_beats, life_beats);
            if let Some(caption) = cue.caption {
                assert!(chart.events.iter().any(|e| e.kind == EventKind::Caption
                    && e.params.variant == caption
                    && (e.beat - spawn.beat).abs() < 1e-9));
            }
        }
        // Generated arrivals (no lifetime) stay out of the verses.
        let cue_beats: Vec<f64> = spawns
            .iter()
            .filter(|e| e.params.duration_beats > 0.0)
            .map(|e| e.beat)
            .collect();
        let generated: Vec<&&ChartEvent> = spawns
            .iter()
            .filter(|e| e.params.duration_beats == 0.0)
            .collect();
        assert!(!generated.is_empty(), "the jams bring enemies too");
        for event in generated {
            let nearest = cue_beats
                .iter()
                .map(|b| (b - event.beat).abs())
                .fold(f64::INFINITY, f64::min);
            assert!(nearest >= 4.0, "generated enemy {nearest} beats from a cue");
        }
        // Lyric hazards are committed first and never thinned.
        let hazard_cues = spec
            .cues
            .iter()
            .filter(|c| matches!(c.action, CueAction::Hazard { .. }))
            .count();
        let placed = spec
            .cues
            .iter()
            .filter(|c| match c.action {
                CueAction::Hazard { kind, .. } => chart.events.iter().any(|e| {
                    e.kind == kind && (timing.beat_to_seconds(e.beat) - c.time).abs() < 0.4
                }),
                _ => false,
            })
            .count();
        assert_eq!(placed, hazard_cues);
    }
}
