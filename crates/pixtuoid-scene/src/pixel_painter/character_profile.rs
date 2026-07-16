//! Stable 200West character identities layered onto the shared 16x20 animation grid.
//!
//! Recurring resident names select fixed profiles. Unnamed real agents reuse the
//! same seven-profile pool deterministically from `AgentId`, so rendering stays
//! local and token-free while active-agent population remains uncapped.

use pixtuoid_core::sprite::{Frame, Palette, Rgb};
use pixtuoid_core::AgentSlot;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum GenderPresentation {
    Masculine,
    Feminine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum CharacterProfile {
    Tom,
    Tristan,
    Alex,
    Vivian,
    Amy,
    Jess,
    Maya,
}

impl CharacterProfile {
    pub(super) fn gender(self) -> GenderPresentation {
        match self {
            Self::Tom | Self::Tristan | Self::Alex => GenderPresentation::Masculine,
            Self::Vivian | Self::Amy | Self::Jess | Self::Maya => GenderPresentation::Feminine,
        }
    }

    pub(super) fn colors(self) -> (Rgb, Rgb) {
        match self {
            Self::Tom => (
                Rgb {
                    r: 20,
                    g: 13,
                    b: 10,
                },
                Rgb {
                    r: 226,
                    g: 174,
                    b: 132,
                },
            ),
            Self::Tristan => (
                Rgb {
                    r: 173,
                    g: 132,
                    b: 63,
                },
                Rgb {
                    r: 239,
                    g: 193,
                    b: 151,
                },
            ),
            Self::Alex => (
                Rgb {
                    r: 15,
                    g: 17,
                    b: 22,
                },
                Rgb {
                    r: 195,
                    g: 139,
                    b: 94,
                },
            ),
            Self::Vivian => (
                Rgb {
                    r: 12,
                    g: 10,
                    b: 14,
                },
                Rgb {
                    r: 234,
                    g: 177,
                    b: 139,
                },
            ),
            Self::Amy => (
                Rgb {
                    r: 112,
                    g: 53,
                    b: 31,
                },
                Rgb {
                    r: 239,
                    g: 191,
                    b: 153,
                },
            ),
            Self::Jess => (
                Rgb {
                    r: 45,
                    g: 27,
                    b: 20,
                },
                Rgb {
                    r: 217,
                    g: 158,
                    b: 119,
                },
            ),
            Self::Maya => (
                Rgb {
                    r: 24,
                    g: 19,
                    b: 18,
                },
                Rgb {
                    r: 181,
                    g: 119,
                    b: 78,
                },
            ),
        }
    }

    pub(super) fn suit_index(self) -> usize {
        match self {
            Self::Tom | Self::Vivian => 0,
            Self::Tristan | Self::Amy => 1,
            Self::Alex | Self::Jess => 2,
            Self::Maya => 3,
        }
    }
}

pub(super) fn profile_for(agent: &AgentSlot) -> CharacterProfile {
    let session = agent.session_id.to_ascii_lowercase();
    let first_name = agent
        .label
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_ascii_lowercase();
    match session.as_str() {
        "tom" => CharacterProfile::Tom,
        "tristan-pembroke" => CharacterProfile::Tristan,
        "alex" => CharacterProfile::Alex,
        "vivian" => CharacterProfile::Vivian,
        "amy" => CharacterProfile::Amy,
        "jess" => CharacterProfile::Jess,
        "maya" => CharacterProfile::Maya,
        _ => match first_name.as_str() {
            "tom" => CharacterProfile::Tom,
            "tristan" => CharacterProfile::Tristan,
            "alex" => CharacterProfile::Alex,
            "vivian" => CharacterProfile::Vivian,
            "amy" => CharacterProfile::Amy,
            "jess" => CharacterProfile::Jess,
            "maya" => CharacterProfile::Maya,
            _ => {
                const POOL: [CharacterProfile; 7] = [
                    CharacterProfile::Tom,
                    CharacterProfile::Vivian,
                    CharacterProfile::Tristan,
                    CharacterProfile::Amy,
                    CharacterProfile::Alex,
                    CharacterProfile::Jess,
                    CharacterProfile::Maya,
                ];
                POOL[agent.agent_id.raw() as usize % POOL.len()]
            }
        },
    }
}

const FRONT_ANIMS: &[&str] = &[
    "seated",
    "typing",
    "standing",
    "walking",
    "walking_coffee",
    "holding_coffee",
];
const BACK_ANIMS: &[&str] = &["walking_back", "back_couch"];

pub(super) fn apply_200west_profile(
    mut frame: Frame,
    palette: &Palette,
    agent: &AgentSlot,
    anim_name: &str,
) -> Frame {
    if frame.width() != 16 || frame.height() != 20 {
        return frame;
    }
    let Some(hair) = palette.get('H').flatten() else {
        return frame;
    };
    let profile = profile_for(agent);
    if FRONT_ANIMS.contains(&anim_name) {
        apply_front_silhouette(&mut frame, profile, hair, palette);
    } else if BACK_ANIMS.contains(&anim_name) {
        apply_back_silhouette(&mut frame, profile, hair);
    }
    frame
}

fn apply_front_silhouette(
    frame: &mut Frame,
    profile: CharacterProfile,
    hair: Rgb,
    palette: &Palette,
) {
    let skin = palette.get('S').flatten().unwrap_or(hair);
    let shadow = palette.get('s').flatten().unwrap_or(hair);
    let eye = palette.get('e').flatten().unwrap_or(hair);
    let mouth = palette.get('m').flatten().unwrap_or(shadow);
    let jacket = palette.get('B').flatten().unwrap_or(hair);
    let shirt = palette.get('w').flatten().unwrap_or(skin);

    // Strip the base sprite's oversized outer cheek columns for every 200West
    // profile. Feminine faces are then framed by hair and taper to a six-pixel
    // chin; masculine faces keep a wider jaw but lose the old square corners.
    clear(frame, &[(2, 3), (13, 3), (2, 4), (13, 4), (2, 5), (13, 5)]);
    if profile.gender() == GenderPresentation::Feminine {
        paint_vertical(frame, 3, 3, 8, hair);
        paint_vertical(frame, 12, 3, 8, hair);
        paint(frame, &[(3, 9), (4, 9), (11, 9), (12, 9)], hair);
    } else {
        clear(frame, &[(3, 9), (12, 9)]);
    }

    match profile {
        CharacterProfile::Tom => {
            clear(frame, &[(10, 0), (12, 3)]);
            paint(frame, &[(4, 0), (4, 1), (4, 2)], hair);
            paint(frame, &[(4, 4), (11, 4)], eye);
            paint(frame, &[(7, 7)], shadow);
            paint(frame, &[(2, 11), (13, 11)], jacket);
        }
        CharacterProfile::Tristan => {
            paint(frame, &[(10, 0), (11, 0), (11, 1), (12, 1)], hair);
            clear(frame, &[(3, 2), (3, 3), (12, 3), (3, 8), (12, 8)]);
            paint(frame, &[(5, 4), (10, 4)], eye);
            paint(frame, &[(8, 7)], shadow);
            paint(frame, &[(3, 10), (12, 10)], jacket);
        }
        CharacterProfile::Alex => {
            clear(frame, &[(5, 0), (10, 0), (4, 1), (11, 1)]);
            paint(frame, &[(5, 1), (10, 1), (4, 2), (11, 2)], hair);
            clear(frame, &[(3, 8), (12, 8)]);
            paint(frame, &[(5, 4), (10, 4)], eye);
            paint(frame, &[(8, 7)], shadow);
            paint(frame, &[(3, 11), (12, 11)], jacket);
        }
        CharacterProfile::Vivian => {
            paint_vertical(frame, 3, 3, 11, hair);
            paint_vertical(frame, 12, 3, 11, hair);
            clear(frame, &[(3, 14), (12, 14)]);
            paint(frame, &[(5, 4), (10, 4)], eye);
            paint(frame, &[(8, 7)], mouth);
            paint(frame, &[(6, 11), (9, 11)], shirt);
        }
        CharacterProfile::Amy => {
            paint_vertical(frame, 3, 3, 9, hair);
            paint_vertical(frame, 12, 3, 9, hair);
            paint(frame, &[(4, 9), (11, 9), (4, 10), (11, 10)], hair);
            clear(frame, &[(3, 14)]);
            paint(frame, &[(5, 4), (10, 4)], eye);
            paint(frame, &[(8, 7)], mouth);
            paint(frame, &[(7, 11), (8, 11)], shirt);
        }
        CharacterProfile::Jess => {
            paint(frame, &[(3, 0), (4, 0), (12, 2), (13, 2)], hair);
            paint_vertical(frame, 13, 4, 8, hair);
            paint_vertical(frame, 14, 6, 8, hair);
            clear(frame, &[(12, 14)]);
            paint(frame, &[(5, 4), (10, 4)], eye);
            paint(frame, &[(8, 7)], mouth);
            paint(frame, &[(6, 11), (9, 11)], shirt);
        }
        CharacterProfile::Maya => {
            paint_vertical(frame, 3, 3, 12, hair);
            paint_vertical(frame, 12, 3, 12, hair);
            paint(frame, &[(2, 6), (2, 8), (13, 7), (13, 9), (13, 11)], hair);
            clear(frame, &[(3, 14), (12, 14)]);
            paint(frame, &[(5, 4), (10, 4)], eye);
            paint(frame, &[(8, 7)], mouth);
            paint(frame, &[(7, 11), (8, 11)], shirt);
        }
    }

    if profile.gender() == GenderPresentation::Feminine {
        fitted_waist(frame, jacket);
    }

    // Keep every face clean at terminal scale after the profile-specific shape pass.
    paint(frame, &[(6, 5), (9, 5), (7, 7), (9, 8)], skin);
    paint(frame, &[(7, 6)], shadow);
}

fn apply_back_silhouette(frame: &mut Frame, profile: CharacterProfile, hair: Rgb) {
    clear(frame, &[(2, 3), (13, 3), (2, 4), (13, 4)]);
    match profile {
        CharacterProfile::Tom => {
            clear(frame, &[(10, 0)]);
            paint(frame, &[(4, 0)], hair);
        }
        CharacterProfile::Tristan => {
            paint(frame, &[(11, 0), (12, 1)], hair);
            clear(frame, &[(3, 3)]);
        }
        CharacterProfile::Alex => {
            clear(frame, &[(5, 0), (10, 0), (4, 1), (11, 1)]);
        }
        CharacterProfile::Vivian => {
            paint_vertical(frame, 3, 3, 11, hair);
            paint_vertical(frame, 12, 3, 11, hair);
        }
        CharacterProfile::Amy => {
            paint_vertical(frame, 3, 3, 9, hair);
            paint_vertical(frame, 12, 3, 9, hair);
            paint(frame, &[(4, 9), (11, 9)], hair);
        }
        CharacterProfile::Jess => {
            paint_vertical(frame, 13, 4, 8, hair);
            paint_vertical(frame, 14, 6, 8, hair);
        }
        CharacterProfile::Maya => {
            paint_vertical(frame, 3, 3, 12, hair);
            paint_vertical(frame, 12, 3, 12, hair);
            paint(frame, &[(2, 6), (2, 8), (13, 7), (13, 9), (13, 11)], hair);
        }
    }
}

fn fitted_waist(frame: &mut Frame, jacket: Rgb) {
    clear(frame, &[(3, 14), (12, 14)]);
    paint(frame, &[(4, 14), (11, 14)], jacket);
}

fn paint(frame: &mut Frame, points: &[(u16, u16)], color: Rgb) {
    for &(x, y) in points {
        frame.set(x, y, Some(color));
    }
}

fn paint_vertical(frame: &mut Frame, x: u16, y0: u16, y1: u16, color: Rgb) {
    for y in y0..=y1 {
        frame.set(x, y, Some(color));
    }
}

fn clear(frame: &mut Frame, points: &[(u16, u16)]) {
    for &(x, y) in points {
        frame.set(x, y, None);
    }
}
