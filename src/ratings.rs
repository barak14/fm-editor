//! Position rating calculator and PA extrapolation.
//! Weights are FM-community-style placeholders — refine after testing.

use crate::player::PlayerRecord;

struct PosWeights {
    pos_key: &'static str,
    label: &'static str,
    weights: &'static [(&'static str, u16)],
}

/// Position weight tables.  Weight scale: 3 = key, 2 = important, 1 = supporting.
static POSITIONS: &[PosWeights] = &[
    PosWeights {
        pos_key: "pos_gk", label: "GK",
        weights: &[
            ("handling", 3), ("aerial_reach", 3), ("reflexes", 3),
            ("command_of_area", 2), ("communication", 2), ("one_on_ones", 2),
            ("positioning", 2), ("concentration", 2),
            ("kicking", 1), ("throwing", 1), ("rushing_out", 1), ("agility", 1),
        ],
    },
    PosWeights {
        pos_key: "pos_dc", label: "DC",
        weights: &[
            ("heading", 3), ("marking", 3), ("tackling", 3), ("positioning", 3),
            ("concentration", 2), ("strength", 2), ("jumping_reach", 2),
            ("bravery", 2), ("anticipation", 2),
            ("composure", 1), ("pace", 1), ("decisions", 1),
        ],
    },
    PosWeights {
        pos_key: "pos_dl", label: "DL",
        weights: &[
            ("tackling", 3), ("marking", 2), ("positioning", 2), ("crossing", 2),
            ("stamina", 2), ("pace", 2), ("work_rate", 2),
            ("acceleration", 1), ("teamwork", 1), ("anticipation", 1),
            ("passing", 1), ("concentration", 1),
        ],
    },
    PosWeights {
        pos_key: "pos_dr", label: "DR",
        weights: &[
            ("tackling", 3), ("marking", 2), ("positioning", 2), ("crossing", 2),
            ("stamina", 2), ("pace", 2), ("work_rate", 2),
            ("acceleration", 1), ("teamwork", 1), ("anticipation", 1),
            ("passing", 1), ("concentration", 1),
        ],
    },
    PosWeights {
        pos_key: "pos_sw", label: "SW",
        weights: &[
            ("marking", 3), ("tackling", 3), ("positioning", 3),
            ("passing", 2), ("composure", 2), ("anticipation", 2),
            ("decisions", 2), ("concentration", 2),
            ("first_touch", 1), ("strength", 1), ("pace", 1), ("heading", 1),
        ],
    },
    PosWeights {
        pos_key: "pos_dm", label: "DM",
        weights: &[
            ("tackling", 3), ("positioning", 3),
            ("marking", 2), ("passing", 2), ("stamina", 2), ("work_rate", 2),
            ("teamwork", 2), ("concentration", 2), ("anticipation", 2),
            ("strength", 1), ("composure", 1), ("decisions", 1),
        ],
    },
    PosWeights {
        pos_key: "pos_mc", label: "MC",
        weights: &[
            ("passing", 3), ("decisions", 3),
            ("teamwork", 2), ("vision", 2), ("stamina", 2), ("technique", 2),
            ("first_touch", 2), ("composure", 2),
            ("work_rate", 1), ("anticipation", 1), ("off_the_ball", 1),
            ("concentration", 1),
        ],
    },
    PosWeights {
        pos_key: "pos_ml", label: "ML",
        weights: &[
            ("crossing", 3),
            ("dribbling", 2), ("pace", 2), ("stamina", 2), ("passing", 2),
            ("technique", 2), ("acceleration", 2), ("work_rate", 2),
            ("off_the_ball", 1), ("teamwork", 1), ("decisions", 1), ("flair", 1),
        ],
    },
    PosWeights {
        pos_key: "pos_mr", label: "MR",
        weights: &[
            ("crossing", 3),
            ("dribbling", 2), ("pace", 2), ("stamina", 2), ("passing", 2),
            ("technique", 2), ("acceleration", 2), ("work_rate", 2),
            ("off_the_ball", 1), ("teamwork", 1), ("decisions", 1), ("flair", 1),
        ],
    },
    PosWeights {
        pos_key: "pos_aml", label: "AML",
        weights: &[
            ("dribbling", 3),
            ("crossing", 2), ("pace", 2), ("acceleration", 2), ("technique", 2),
            ("finishing", 2), ("flair", 2), ("off_the_ball", 2),
            ("first_touch", 1), ("agility", 1), ("composure", 1), ("decisions", 1),
        ],
    },
    PosWeights {
        pos_key: "pos_amr", label: "AMR",
        weights: &[
            ("dribbling", 3),
            ("crossing", 2), ("pace", 2), ("acceleration", 2), ("technique", 2),
            ("finishing", 2), ("flair", 2), ("off_the_ball", 2),
            ("first_touch", 1), ("agility", 1), ("composure", 1), ("decisions", 1),
        ],
    },
    PosWeights {
        pos_key: "pos_amc", label: "AMC",
        weights: &[
            ("passing", 3), ("vision", 3),
            ("decisions", 2), ("technique", 2), ("first_touch", 2),
            ("composure", 2), ("dribbling", 2), ("off_the_ball", 2),
            ("flair", 1), ("anticipation", 1), ("long_shots", 1), ("agility", 1),
        ],
    },
    PosWeights {
        pos_key: "pos_st", label: "ST",
        weights: &[
            ("finishing", 3),
            ("composure", 2), ("off_the_ball", 2), ("first_touch", 2), ("heading", 2),
            ("anticipation", 1), ("decisions", 1), ("pace", 1),
            ("strength", 1), ("technique", 1), ("dribbling", 1), ("acceleration", 1),
        ],
    },
    PosWeights {
        pos_key: "pos_wbl", label: "WBL",
        weights: &[
            ("crossing", 3), ("stamina", 3),
            ("pace", 2), ("tackling", 2), ("acceleration", 2), ("work_rate", 2),
            ("dribbling", 1), ("passing", 1), ("technique", 1),
            ("marking", 1), ("positioning", 1), ("teamwork", 1),
        ],
    },
    PosWeights {
        pos_key: "pos_wbr", label: "WBR",
        weights: &[
            ("crossing", 3), ("stamina", 3),
            ("pace", 2), ("tackling", 2), ("acceleration", 2), ("work_rate", 2),
            ("dribbling", 1), ("passing", 1), ("technique", 1),
            ("marking", 1), ("positioning", 1), ("teamwork", 1),
        ],
    },
];

/// Calculate a 0-100 rating for every position. Returns (label, pos_key, familiarity, rating).
pub fn all_position_ratings(player: &PlayerRecord) -> Vec<(&'static str, &'static str, u8, u8)> {
    POSITIONS
        .iter()
        .map(|pw| {
            let fam = player.field_u8(pw.pos_key).unwrap_or(1);
            let rating = calc_rating(player, pw);
            (pw.label, pw.pos_key, fam, rating)
        })
        .collect()
}

/// Strict, unrounded rating for lineup selection. Missing or invalid attributes
/// exclude a position instead of silently substituting weak/default values.
pub fn checked_position_rating(
    pos_key: &str,
    mut attribute: impl FnMut(&str) -> Option<u8>,
) -> Option<f64> {
    let weights = POSITIONS.iter().find(|p| p.pos_key == pos_key)?;
    let mut total = 0u32;
    let mut weight_sum = 0u32;
    for &(key, weight) in weights.weights {
        let value = attribute(key).filter(|v| (1..=20).contains(v))?;
        total += u32::from(value) * u32::from(weight);
        weight_sum += u32::from(weight);
    }
    Some(f64::from(total) * 5.0 / f64::from(weight_sum))
}

fn calc_rating(player: &PlayerRecord, pw: &PosWeights) -> u8 {
    let mut total: u32 = 0;
    let mut max_total: u32 = 0;
    for &(key, weight) in pw.weights {
        let val = player.field_u8(key).unwrap_or(1) as u32;
        total += val * weight as u32;
        max_total += 20 * weight as u32;
    }
    if max_total == 0 {
        return 0;
    }
    ((total * 100) / max_total) as u8
}

// ---------------------------------------------------------------------------
// PA extrapolation (Genie Scout-style)
// ---------------------------------------------------------------------------
//
// Naive `current * PA / CA` scales every attribute uniformly, which doesn't
// match how FM actually grows players. Character mentals (Aggression,
// Determination, Work Rate) and most hidden stay frozen, while technical +
// "playing mental" attributes grow hardest and physical grows moderately.
// Calibrated against Franco Mastantuono (AMC/AMR, CA 140 → PA 190).

#[derive(Clone, Copy)]
enum Tier {
    /// Technical, set pieces, playing-mental attributes. Grow hardest.
    Main,
    /// Pace, acceleration, strength, etc. Moderate growth.
    Physical,
    /// Consistency, Versatility. Small growth.
    HiddenLight,
    /// Character mentals + most hidden. No growth.
    Frozen,
}

const COEFF_MAIN: f32 = 0.60;
const COEFF_PHYSICAL: f32 = 0.35;
const COEFF_HIDDEN_LIGHT: f32 = 0.15;

/// Outfield profile: all GK attrs frozen, outfield technicals in Main.
const TIERS_OUTFIELD: &[(&str, Tier)] = &[
    // Technical
    ("crossing", Tier::Main), ("dribbling", Tier::Main), ("finishing", Tier::Main),
    ("heading", Tier::Main),  ("long_shots", Tier::Main), ("marking", Tier::Main),
    ("passing", Tier::Main),  ("tackling", Tier::Main),  ("technique", Tier::Main),
    ("first_touch", Tier::Main),
    // Set Pieces
    ("corners", Tier::Main), ("free_kicks", Tier::Main), ("long_throws", Tier::Main),
    ("penalty_taking", Tier::Main),
    // Mental (playing)
    ("anticipation", Tier::Main), ("composure", Tier::Main), ("concentration", Tier::Main),
    ("decisions", Tier::Main),    ("flair", Tier::Main),    ("off_the_ball", Tier::Main),
    ("positioning", Tier::Main),  ("vision", Tier::Main),
    // Mental (character) — frozen
    ("aggression", Tier::Frozen),  ("bravery", Tier::Frozen),     ("determination", Tier::Frozen),
    ("leadership", Tier::Frozen),  ("teamwork", Tier::Frozen),    ("work_rate", Tier::Frozen),
    // Physical
    ("acceleration", Tier::Physical), ("agility", Tier::Physical), ("balance", Tier::Physical),
    ("jumping_reach", Tier::Physical),("natural_fitness", Tier::Physical),
    ("pace", Tier::Physical),         ("stamina", Tier::Physical), ("strength", Tier::Physical),
    // Goalkeeping — frozen for outfielders
    ("handling", Tier::Frozen), ("aerial_reach", Tier::Frozen),
    ("command_of_area", Tier::Frozen), ("communication", Tier::Frozen),
    ("kicking", Tier::Frozen), ("throwing", Tier::Frozen),
    ("one_on_ones", Tier::Frozen), ("reflexes", Tier::Frozen),
    ("eccentricity", Tier::Frozen), ("rushing_out", Tier::Frozen),
    ("punching", Tier::Frozen),
    // Hidden
    ("consistency", Tier::HiddenLight), ("p_versatility", Tier::HiddenLight),
    ("important_matches", Tier::Frozen), ("injury_proneness", Tier::Frozen),
    ("p_dirtiness", Tier::Frozen),
];

/// GK profile: goalkeeping attrs Main, outfield-only technicals Frozen.
const TIERS_GK: &[(&str, Tier)] = &[
    // Goalkeeping — Main
    ("handling", Tier::Main), ("aerial_reach", Tier::Main),
    ("command_of_area", Tier::Main), ("communication", Tier::Main),
    ("kicking", Tier::Main), ("throwing", Tier::Main),
    ("one_on_ones", Tier::Main), ("reflexes", Tier::Main),
    ("eccentricity", Tier::Main), ("rushing_out", Tier::Main),
    ("punching", Tier::Main),
    // Outfield-specific technicals — frozen
    ("crossing", Tier::Frozen), ("dribbling", Tier::Frozen), ("finishing", Tier::Frozen),
    ("long_shots", Tier::Frozen), ("tackling", Tier::Frozen),
    ("corners", Tier::Frozen), ("free_kicks", Tier::Frozen), ("long_throws", Tier::Frozen),
    ("penalty_taking", Tier::Frozen),
    // Shared technicals still relevant to GKs
    ("heading", Tier::Main), ("marking", Tier::Main),
    ("passing", Tier::Main),  ("technique", Tier::Main), ("first_touch", Tier::Main),
    // Mental playing — Main (anticipation/concentration/positioning especially)
    ("anticipation", Tier::Main), ("composure", Tier::Main), ("concentration", Tier::Main),
    ("decisions", Tier::Main),    ("flair", Tier::Frozen),  ("off_the_ball", Tier::Frozen),
    ("positioning", Tier::Main),  ("vision", Tier::Main),
    // Mental character
    ("aggression", Tier::Frozen),  ("bravery", Tier::Main),       ("determination", Tier::Frozen),
    ("leadership", Tier::Frozen),  ("teamwork", Tier::Frozen),    ("work_rate", Tier::Frozen),
    // Physical
    ("acceleration", Tier::Physical), ("agility", Tier::Physical), ("balance", Tier::Physical),
    ("jumping_reach", Tier::Physical),("natural_fitness", Tier::Physical),
    ("pace", Tier::Physical),         ("stamina", Tier::Physical), ("strength", Tier::Physical),
    // Hidden
    ("consistency", Tier::HiddenLight), ("p_versatility", Tier::HiddenLight),
    ("important_matches", Tier::Frozen), ("injury_proneness", Tier::Frozen),
    ("p_dirtiness", Tier::Frozen),
];

fn pick_tier_table(player: &PlayerRecord) -> &'static [(&'static str, Tier)] {
    let gk_fam = player.field_u8("pos_gk").unwrap_or(1);
    if gk_fam >= 17 { TIERS_GK } else { TIERS_OUTFIELD }
}

/// Compute the Genie Scout-style extrapolation of every growing attribute.
/// Returns (field_key, current_displayed, projected_displayed) tuples for
/// attributes that would grow. Pure read — never touches `player.raw`
/// mutably, never writes to memory.
///
/// Formula per attribute:
///     new = round(cur + (20 - cur) * tier_coeff * (PA - CA) / 50)
/// clamped to 1..=20. `tier_coeff` is 0 for frozen attributes, so they
/// don't move regardless of the CA→PA gap.
pub fn extrapolate_player(player: &PlayerRecord) -> Vec<(&'static str, u8, u8)> {
    let ca = player.field_u8("p_ca").unwrap_or(0);
    let pa = player.field_u8("p_pa").unwrap_or(0);
    if pa <= ca || ca == 0 {
        return Vec::new();
    }
    let delta = (pa - ca) as f32;
    let tiers = pick_tier_table(player);

    tiers
        .iter()
        .filter_map(|(key, tier)| {
            let field = player.build.schemas().field(key)?;
            let loc = field.offset.as_ref()?;
            let off = loc.offset() as usize;
            if off >= player.raw.len() {
                return None;
            }
            let cur = loc.decode_u8(player.raw[off]);
            if !(1..=20).contains(&cur) {
                return None;
            }
            let coeff = match tier {
                Tier::Main => COEFF_MAIN,
                Tier::Physical => COEFF_PHYSICAL,
                Tier::HiddenLight => COEFF_HIDDEN_LIGHT,
                Tier::Frozen => 0.0,
            };
            let gap = 20.0 - cur as f32;
            let growth = gap * coeff * delta / 50.0;
            let projected = ((cur as f32 + growth).round() as i32).clamp(1, 20) as u8;
            if projected > cur {
                Some((*key, cur, projected))
            } else {
                None
            }
        })
        .collect()
}
