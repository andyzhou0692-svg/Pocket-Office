//! Procedural detail pass for the shared 12x16 front-facing character face.

use pixtuoid_core::sprite::{Frame, Palette};

use super::palette::blend_rgb;

const FRONT_FACE_ANIMS: &[&str] = &[
    "seated",
    "typing",
    "standing",
    "walking",
    "walking_coffee",
    "holding_coffee",
];
const MIN_FACE_WIDTH: u16 = 9;
const MIN_FACE_HEIGHT: u16 = 7;
const EYE_ACCENT_MIX: f32 = 0.18;
const EYE_POINTS: &[(u16, u16)] = &[(4, 3), (7, 3)];
const CLEAR_POINTS: &[(u16, u16)] = &[
    (4, 2),
    (7, 2),
    (3, 4),
    (5, 4),
    (7, 4),
    (8, 4),
    (5, 5),
    (7, 6),
];
const NOSE_POINT: (u16, u16) = (6, 4);
const MOUTH_POINT: (u16, u16) = (6, 5);

pub(super) fn apply_front_face_overlay(
    mut frame: Frame,
    palette: &Palette,
    anim_name: &str,
) -> Frame {
    if !FRONT_FACE_ANIMS.contains(&anim_name)
        || frame.width() < MIN_FACE_WIDTH
        || frame.height() < MIN_FACE_HEIGHT
    {
        return frame;
    }

    let (Some(skin), Some(shadow), Some(eye), Some(mouth), Some(accent)) = (
        palette.get('S').flatten(),
        palette.get('s').flatten(),
        palette.get('e').flatten(),
        palette.get('m').flatten(),
        palette.get('c').flatten(),
    ) else {
        return frame;
    };
    if !has_pocket_office_face_geometry(&frame, eye, shadow, mouth) {
        return frame;
    }
    let eye_accent = blend_rgb(eye, accent, EYE_ACCENT_MIX);

    for &(x, y) in EYE_POINTS {
        frame.set(x, y, Some(eye_accent));
    }
    for &(x, y) in CLEAR_POINTS {
        frame.set(x, y, Some(skin));
    }
    frame.set(NOSE_POINT.0, NOSE_POINT.1, Some(shadow));
    frame.set(MOUTH_POINT.0, MOUTH_POINT.1, Some(shadow));
    frame
}

fn has_pocket_office_face_geometry(
    frame: &Frame,
    eye: pixtuoid_core::Rgb,
    shadow: pixtuoid_core::Rgb,
    mouth: pixtuoid_core::Rgb,
) -> bool {
    frame.get(4, 3) == Some(&Some(eye))
        && frame.get(7, 3) == Some(&Some(eye))
        && frame.get(5, 4) == Some(&Some(shadow))
        && frame.get(7, 4) == Some(&Some(shadow))
        && frame.get(5, 5) == Some(&Some(mouth))
        && frame.get(6, 5) == Some(&Some(shadow))
}
