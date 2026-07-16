//! 2dPig-derived component layers applied to the shared 16x20 character frame.
//!
//! The head is rebuilt from stable hair and face components. Outfit layers
//! recolor the current pose footprint, so typing, walking and seating remain
//! driven by the authored animation frames.

use pixtuoid_core::sprite::{Frame, Palette, Rgb};

use super::character_profile::{ComponentLook, FaceComponent, HairComponent, OutfitComponent};

const BURGUNDY_DRESS: Rgb = Rgb {
    r: 129,
    g: 56,
    b: 77,
};
const SKIRT_BLUE: Rgb = Rgb {
    r: 69,
    g: 111,
    b: 159,
};
const BLUSH: Rgb = Rgb {
    r: 217,
    g: 126,
    b: 124,
};
const GLASSES_BLACK: Rgb = Rgb {
    r: 18,
    g: 18,
    b: 20,
};

pub(super) fn apply_2dpig_front(
    frame: &mut Frame,
    palette: &Palette,
    look: ComponentLook,
    anim_name: &str,
) {
    let Some(hair) = color(palette, 'H') else {
        return;
    };
    let Some(skin) = color(palette, 'S') else {
        return;
    };
    let shadow = color(palette, 's').unwrap_or(hair);
    let outline = color(palette, 'n').unwrap_or(hair);
    let eye = color(palette, 'e').unwrap_or(outline);
    let mouth = color(palette, 'm').unwrap_or(shadow);

    rebuild_front_head(frame, skin);
    paint_outfit(frame, palette, look.outfit, anim_name);
    paint_front_hair(frame, look.hair, hair);
    paint_face(frame, look.face, skin, shadow, eye, mouth);
}

pub(super) fn apply_2dpig_back(
    frame: &mut Frame,
    palette: &Palette,
    look: ComponentLook,
    anim_name: &str,
) {
    let Some(hair) = color(palette, 'H') else {
        return;
    };
    clear_rect(frame, 2, 0, 14, 10);
    paint_outfit(frame, palette, look.outfit, anim_name);
    match look.hair {
        HairComponent::ExecutiveBob => {
            line(frame, 5, 10, 0, hair);
            line(frame, 4, 11, 1, hair);
            line(frame, 3, 12, 2, hair);
            rect(frame, 3, 3, 12, 9, hair);
            line(frame, 4, 11, 10, hair);
        }
        HairComponent::Ponytail => {
            line(frame, 5, 10, 0, hair);
            line(frame, 4, 11, 1, hair);
            line(frame, 3, 12, 2, hair);
            rect(frame, 3, 3, 12, 8, hair);
            rect(frame, 12, 5, 14, 9, hair);
            point(frame, 13, 10, hair);
        }
        HairComponent::LongHair => {
            line(frame, 5, 10, 0, hair);
            line(frame, 4, 11, 1, hair);
            line(frame, 3, 12, 2, hair);
            rect(frame, 3, 3, 12, 10, hair);
            paint_vertical(frame, 2, 7, 14, hair);
            paint_vertical(frame, 13, 7, 14, hair);
            paint_vertical(frame, 3, 10, 15, hair);
            paint_vertical(frame, 12, 10, 15, hair);
        }
    }
}

fn rebuild_front_head(frame: &mut Frame, skin: Rgb) {
    clear_rect(frame, 2, 0, 14, 10);
    rect(frame, 4, 3, 11, 6, skin);
    line(frame, 5, 10, 7, skin);
    line(frame, 6, 9, 8, skin);
    line(frame, 7, 8, 9, skin);
    point(frame, 3, 5, skin);
    point(frame, 12, 5, skin);
    point(frame, 3, 6, skin);
    point(frame, 12, 6, skin);
}

fn paint_front_hair(frame: &mut Frame, component: HairComponent, hair: Rgb) {
    match component {
        HairComponent::ExecutiveBob => {
            line(frame, 5, 10, 0, hair);
            line(frame, 4, 11, 1, hair);
            line(frame, 3, 12, 2, hair);
            paint_vertical(frame, 3, 3, 10, hair);
            paint_vertical(frame, 12, 3, 10, hair);
            point(frame, 4, 8, hair);
            point(frame, 11, 8, hair);
            point(frame, 4, 9, hair);
            point(frame, 11, 9, hair);
        }
        HairComponent::Ponytail => {
            line(frame, 5, 10, 0, hair);
            line(frame, 4, 11, 1, hair);
            line(frame, 3, 12, 2, hair);
            paint_vertical(frame, 3, 3, 8, hair);
            paint_vertical(frame, 12, 3, 7, hair);
            line(frame, 3, 7, 3, hair);
            line(frame, 3, 5, 4, hair);
            point(frame, 11, 4, hair);
            point(frame, 12, 4, hair);
            rect(frame, 13, 6, 14, 9, hair);
            point(frame, 13, 10, hair);
        }
        HairComponent::LongHair => {
            line(frame, 5, 10, 0, hair);
            line(frame, 4, 11, 1, hair);
            line(frame, 3, 12, 2, hair);
            paint_vertical(frame, 3, 3, 15, hair);
            paint_vertical(frame, 12, 3, 15, hair);
            paint_vertical(frame, 2, 7, 14, hair);
            paint_vertical(frame, 13, 7, 14, hair);
            point(frame, 4, 8, hair);
            point(frame, 11, 8, hair);
        }
    }
}

fn paint_face(
    frame: &mut Frame,
    component: FaceComponent,
    skin: Rgb,
    shadow: Rgb,
    eye: Rgb,
    mouth: Rgb,
) {
    match component {
        FaceComponent::SoftMakeup => {
            point(frame, 5, 4, shadow);
            point(frame, 10, 4, shadow);
            point(frame, 5, 5, eye);
            point(frame, 10, 5, eye);
            point(frame, 4, 6, BLUSH);
            point(frame, 11, 6, BLUSH);
            point(frame, 7, 6, shadow);
            point(frame, 7, 8, mouth);
            point(frame, 8, 8, mouth);
        }
        FaceComponent::Glasses => {
            line(frame, 2, 6, 4, GLASSES_BLACK);
            line(frame, 9, 13, 4, GLASSES_BLACK);
            for (x, y) in [
                (3, 5),
                (6, 5),
                (7, 5),
                (8, 5),
                (9, 5),
                (12, 5),
                (3, 6),
                (6, 6),
                (9, 6),
                (12, 6),
            ] {
                point(frame, x, y, GLASSES_BLACK);
            }
            line(frame, 4, 5, 7, GLASSES_BLACK);
            line(frame, 10, 11, 7, GLASSES_BLACK);
            point(frame, 4, 6, eye);
            point(frame, 11, 6, eye);
            point(frame, 7, 7, shadow);
            point(frame, 7, 8, mouth);
            point(frame, 8, 8, mouth);
            point(frame, 7, 9, skin);
            point(frame, 8, 9, skin);
        }
    }
}

fn paint_outfit(frame: &mut Frame, palette: &Palette, component: OutfitComponent, anim_name: &str) {
    let jacket = color(palette, 'B').unwrap_or(BURGUNDY_DRESS);
    let ivory = color(palette, 'w').unwrap_or(Rgb {
        r: 242,
        g: 238,
        b: 229,
    });
    let outline = color(palette, 'n').unwrap_or(jacket);
    let walking = anim_name.starts_with("walking");

    match component {
        OutfitComponent::NavySuit => {
            recolor_opaque(frame, 3, 11, 12, 16, jacket);
            rect(frame, 7, 11, 8, 13, ivory);
            point(frame, 7, 13, outline);
            point(frame, 8, 13, outline);
        }
        OutfitComponent::IvorySkirt => {
            recolor_opaque(frame, 3, 11, 12, 14, ivory);
            recolor_opaque(frame, 3, 15, 12, 16, SKIRT_BLUE);
            if !walking {
                line(frame, 4, 11, 16, SKIRT_BLUE);
            }
            point(frame, 7, 11, BURGUNDY_DRESS);
            point(frame, 8, 11, BURGUNDY_DRESS);
        }
        OutfitComponent::BurgundyDress => {
            recolor_opaque(frame, 3, 11, 12, 16, BURGUNDY_DRESS);
            if !walking {
                line(frame, 3, 12, 16, BURGUNDY_DRESS);
            }
            point(frame, 7, 11, ivory);
            point(frame, 8, 11, ivory);
        }
    }
}

fn color(palette: &Palette, role: char) -> Option<Rgb> {
    palette.get(role).flatten()
}

fn point(frame: &mut Frame, x: u16, y: u16, color: Rgb) {
    frame.set(x, y, Some(color));
}

fn line(frame: &mut Frame, x0: u16, x1: u16, y: u16, color: Rgb) {
    for x in x0..=x1 {
        point(frame, x, y, color);
    }
}

fn paint_vertical(frame: &mut Frame, x: u16, y0: u16, y1: u16, color: Rgb) {
    for y in y0..=y1 {
        point(frame, x, y, color);
    }
}

fn rect(frame: &mut Frame, x0: u16, y0: u16, x1: u16, y1: u16, color: Rgb) {
    for y in y0..=y1 {
        line(frame, x0, x1, y, color);
    }
}

fn clear_rect(frame: &mut Frame, x0: u16, y0: u16, x1: u16, y1: u16) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            frame.set(x, y, None);
        }
    }
}

fn recolor_opaque(frame: &mut Frame, x0: u16, y0: u16, x1: u16, y1: u16, color: Rgb) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            if matches!(frame.get(x, y), Some(Some(_))) {
                point(frame, x, y, color);
            }
        }
    }
}
