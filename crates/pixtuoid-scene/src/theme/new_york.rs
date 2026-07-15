use pixtuoid_core::sprite::Rgb;

use super::*;

const fn new_york_theme() -> Theme {
    let mut theme = super::normal::NORMAL_THEME;
    theme.name = NEW_YORK_THEME_NAME;

    theme.surface.wall = Rgb {
        r: 190,
        g: 185,
        b: 172,
    };
    theme.surface.wall_trim = Rgb {
        r: 108,
        g: 113,
        b: 116,
    };
    theme.surface.baseboard = Rgb {
        r: 48,
        g: 52,
        b: 55,
    };
    theme.surface.carpet_base = Rgb {
        r: 78,
        g: 94,
        b: 109,
    };
    theme.surface.carpet_light = Rgb {
        r: 96,
        g: 112,
        b: 127,
    };
    theme.surface.carpet_dark = Rgb {
        r: 58,
        g: 72,
        b: 85,
    };
    theme.surface.window_frame = Rgb {
        r: 31,
        g: 38,
        b: 44,
    };
    theme.surface.bg_fallback = Rgb {
        r: 41,
        g: 50,
        b: 59,
    };

    theme.office.room_wall_body = Rgb {
        r: 201,
        g: 195,
        b: 180,
    };
    theme.office.room_wall_trim_light = Rgb {
        r: 226,
        g: 220,
        b: 204,
    };
    theme.office.room_wall_trim_dark = Rgb {
        r: 105,
        g: 107,
        b: 105,
    };
    theme.office.cubicle_divider = Rgb {
        r: 105,
        g: 117,
        b: 126,
    };
    theme.office.runner_base = Rgb {
        r: 59,
        g: 75,
        b: 89,
    };
    theme.office.runner_stripe = Rgb {
        r: 193,
        g: 149,
        b: 55,
    };
    theme.office.runner_edge = Rgb {
        r: 37,
        g: 47,
        b: 57,
    };
    theme.office.neon_panel_bg = Rgb {
        r: 38,
        g: 45,
        b: 52,
    };
    theme.office.neon_frame_base = Rgb {
        r: 211,
        g: 165,
        b: 65,
    };
    theme.office.building_dark = Rgb {
        r: 44,
        g: 52,
        b: 59,
    };
    theme.office.building_light = Rgb {
        r: 93,
        g: 105,
        b: 114,
    };
    theme.office.city_lit_windows = [
        Rgb {
            r: 249,
            g: 213,
            b: 126,
        },
        Rgb {
            r: 177,
            g: 209,
            b: 229,
        },
        Rgb {
            r: 238,
            g: 228,
            b: 198,
        },
    ];
    theme.office.city_dark_window = Rgb {
        r: 36,
        g: 44,
        b: 51,
    };

    theme.lighting.day_sky_a = Rgb {
        r: 111,
        g: 157,
        b: 193,
    };
    theme.lighting.day_sky_b = Rgb {
        r: 173,
        g: 204,
        b: 224,
    };
    theme.lighting.night_sky_a = Rgb {
        r: 17,
        g: 29,
        b: 50,
    };
    theme.lighting.night_sky_b = Rgb {
        r: 31,
        g: 51,
        b: 76,
    };
    theme.lighting.twilight_a = Rgb {
        r: 213,
        g: 115,
        b: 74,
    };
    theme.lighting.twilight_b = Rgb {
        r: 239,
        g: 170,
        b: 105,
    };
    theme.lighting.sun_spill = Rgb {
        r: 252,
        g: 225,
        b: 169,
    };
    theme.lighting.ceiling_pool = Rgb {
        r: 246,
        g: 238,
        b: 215,
    };
    theme.lighting.floor_lamp_halo = Rgb {
        r: 224,
        g: 188,
        b: 116,
    };
    theme.lighting.night_tint = Rgb {
        r: 24,
        g: 35,
        b: 49,
    };

    theme.furniture.wood_top = Rgb {
        r: 129,
        g: 91,
        b: 59,
    };
    theme.furniture.wood_trim = Rgb {
        r: 78,
        g: 55,
        b: 40,
    };
    theme.furniture.rug_field = Rgb {
        r: 70,
        g: 82,
        b: 91,
    };
    theme.furniture.rug_trim = Rgb {
        r: 41,
        g: 50,
        b: 58,
    };
    theme.furniture.rug_accent = Rgb {
        r: 196,
        g: 148,
        b: 48,
    };
    theme.ui.neon_brand = Rgb {
        r: 221,
        g: 174,
        b: 77,
    };
    theme.ui.neon_star = Rgb {
        r: 246,
        g: 217,
        b: 150,
    };

    theme
}

pub static NEW_YORK: Theme = new_york_theme();
