//! Procedural detail pass for the shared 16x20 front-facing character face.

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
const MIN_FACE_WIDTH: u16 = 16;
const MIN_FACE_HEIGHT: u16 = 10;
const EYE_ACCENT_MIX: f32 = 0.18;
const EYE_POINTS: &[(u16, u16)] = &[(5, 4), (10, 4)];
const CLEAR_POINTS: &[(u16, u16)] = &[
    (5, 3),
    (10, 3),
    (4, 5),
    (6, 5),
    (9, 5),
    (11, 5),
    (7, 7),
    (9, 8),
];
const NOSE_POINT: (u16, u16) = (7, 6);
const MOUTH_POINT: (u16, u16) = (8, 7);

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
    frame.get(5, 4) == Some(&Some(eye))
        && frame.get(10, 4) == Some(&Some(eye))
        && frame.get(7, 6) == Some(&Some(shadow))
        && frame.get(9, 6) == Some(&Some(shadow))
        && frame.get(8, 7) == Some(&Some(mouth))
        && frame.get(9, 7) == Some(&Some(shadow))
}
