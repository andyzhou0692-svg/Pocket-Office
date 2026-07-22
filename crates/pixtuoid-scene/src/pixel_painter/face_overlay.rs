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

pub(super) fn paint_hires_front_details(
    buf: &mut pixtuoid_core::sprite::RgbBuffer,
    frame: &Frame,
    anchor: crate::layout::Point,
    anim_name: &str,
) {
    if buf.horizontal_scale() != 2
        || !FRONT_FACE_ANIMS.contains(&anim_name)
        || frame.width() < MIN_FACE_WIDTH
        || frame.height() < MIN_FACE_HEIGHT
    {
        return;
    }
    let Some(skin) = frame.get(6, 5).and_then(|pixel| *pixel) else {
        return;
    };
    let Some(eye) = frame.get(5, 4).and_then(|pixel| *pixel) else {
        return;
    };
    let shadow = frame.get(7, 6).and_then(|pixel| *pixel).unwrap_or(eye);
    let mouth = frame.get(8, 7).and_then(|pixel| *pixel).unwrap_or(shadow);
    if !has_pocket_office_face_geometry(frame, eye, shadow, mouth) {
        return;
    }
    let put = |buf: &mut pixtuoid_core::sprite::RgbBuffer,
               sub_x: u16,
               local_y: u16,
               color: pixtuoid_core::sprite::Rgb| {
        let x = anchor.x.saturating_mul(2).saturating_add(sub_x);
        let y = anchor.y.saturating_add(local_y);
        if x < buf.physical_width() && y < buf.height() {
            buf.physical_put(x, y, color);
        }
    };

    put(buf, 10, 4, skin);
    put(buf, 11, 4, eye);
    put(buf, 20, 4, eye);
    put(buf, 21, 4, skin);
    // Keep the original hairline intact. The first x2 pass painted three-pixel
    // eyebrow bars on this row; at terminal scale they read as a second pair
    // of eyes and made the face look vertically duplicated.
    put(buf, 14, 6, skin);
    put(buf, 15, 6, shadow);
    put(buf, 16, 6, skin);
    put(buf, 15, 7, skin);
    put(buf, 16, 7, mouth);
    put(buf, 17, 7, mouth);
    put(buf, 18, 7, skin);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixtuoid_core::sprite::{Rgb, RgbBuffer};

    #[test]
    fn high_resolution_face_uses_inner_eye_subcolumns() {
        let skin = Rgb {
            r: 220,
            g: 170,
            b: 120,
        };
        let eye = Rgb {
            r: 20,
            g: 25,
            b: 30,
        };
        let mut pixels = vec![Some(skin); 16 * 20];
        pixels[4 * 16 + 5] = Some(eye);
        pixels[4 * 16 + 10] = Some(eye);
        pixels[6 * 16 + 7] = Some(eye);
        pixels[6 * 16 + 9] = Some(eye);
        pixels[7 * 16 + 8] = Some(eye);
        pixels[7 * 16 + 9] = Some(eye);
        let frame = Frame::from_pixels(16, 20, pixels);
        let mut buf = RgbBuffer::filled_x2(16, 20, skin);

        paint_hires_front_details(
            &mut buf,
            &frame,
            crate::layout::Point { x: 0, y: 0 },
            "standing",
        );

        assert_eq!(buf.physical_get(10, 4), skin);
        assert_eq!(buf.physical_get(11, 4), eye);
        assert_eq!(buf.physical_get(20, 4), eye);
        assert_eq!(buf.physical_get(21, 4), skin);
    }

    #[test]
    fn high_resolution_face_does_not_add_base_eyes_to_a_shifted_profile() {
        let skin = Rgb {
            r: 220,
            g: 170,
            b: 120,
        };
        let eye = Rgb {
            r: 20,
            g: 25,
            b: 30,
        };
        let mut pixels = vec![Some(skin); 16 * 20];
        pixels[5 * 16 + 5] = Some(eye);
        pixels[5 * 16 + 10] = Some(eye);
        let frame = Frame::from_pixels(16, 20, pixels);
        let mut buf = RgbBuffer::filled_x2(16, 20, skin);

        paint_hires_front_details(
            &mut buf,
            &frame,
            crate::layout::Point { x: 0, y: 0 },
            "standing",
        );

        assert_eq!(buf.physical_get(11, 4), skin);
        assert_eq!(buf.physical_get(20, 4), skin);
    }
}
