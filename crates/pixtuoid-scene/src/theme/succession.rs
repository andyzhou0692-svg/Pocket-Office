use pixtuoid_core::sprite::Rgb;

use super::*;

const fn succession_theme() -> Theme {
    let mut theme = super::normal::NORMAL_THEME;
    theme.name = SUCCESSION_THEME_NAME;

    theme.surface.wall = Rgb {
        r: 224,
        g: 219,
        b: 204,
    };
    theme.surface.wall_trim = Rgb {
        r: 164,
        g: 150,
        b: 126,
    };
    theme.surface.baseboard = Rgb {
        r: 40,
        g: 39,
        b: 38,
    };
    theme.surface.carpet_base = Rgb {
        r: 72,
        g: 67,
        b: 61,
    };
    theme.surface.carpet_light = Rgb {
        r: 91,
        g: 84,
        b: 75,
    };
    theme.surface.carpet_dark = Rgb {
        r: 53,
        g: 50,
        b: 47,
    };
    theme.surface.window_frame = Rgb {
        r: 27,
        g: 30,
        b: 31,
    };
    theme.surface.bg_fallback = Rgb {
        r: 39,
        g: 42,
        b: 43,
    };

    theme.office.room_wall_body = Rgb {
        r: 234,
        g: 229,
        b: 214,
    };
    theme.office.room_wall_trim_light = Rgb {
        r: 248,
        g: 243,
        b: 229,
    };
    theme.office.room_wall_trim_dark = Rgb {
        r: 143,
        g: 132,
        b: 113,
    };
    theme.office.cubicle_divider = Rgb {
        r: 132,
        g: 124,
        b: 112,
    };
    theme.office.runner_base = Rgb {
        r: 45,
        g: 43,
        b: 40,
    };
    theme.office.runner_stripe = Rgb {
        r: 115,
        g: 89,
        b: 57,
    };
    theme.office.runner_edge = Rgb {
        r: 25,
        g: 25,
        b: 24,
    };
    theme.office.neon_panel_bg = Rgb {
        r: 35,
        g: 35,
        b: 34,
    };
    theme.office.neon_frame_base = Rgb {
        r: 184,
        g: 143,
        b: 83,
    };
    theme.office.building_dark = Rgb {
        r: 50,
        g: 58,
        b: 64,
    };
    theme.office.building_light = Rgb {
        r: 103,
        g: 112,
        b: 116,
    };
    theme.office.city_lit_windows = [
        Rgb {
            r: 247,
            g: 222,
            b: 170,
        },
        Rgb {
            r: 224,
            g: 202,
            b: 151,
        },
        Rgb {
            r: 189,
            g: 209,
            b: 214,
        },
    ];
    theme.office.city_dark_window = Rgb {
        r: 42,
        g: 49,
        b: 54,
    };
    theme.office.shadow = Rgb {
        r: 34,
        g: 31,
        b: 27,
    };

    theme.lighting.day_sky_a = Rgb {
        r: 129,
        g: 169,
        b: 194,
    };
    theme.lighting.day_sky_b = Rgb {
        r: 190,
        g: 211,
        b: 218,
    };
    theme.lighting.night_sky_a = Rgb {
        r: 19,
        g: 30,
        b: 43,
    };
    theme.lighting.night_sky_b = Rgb {
        r: 39,
        g: 51,
        b: 64,
    };
    theme.lighting.twilight_a = Rgb {
        r: 194,
        g: 112,
        b: 75,
    };
    theme.lighting.twilight_b = Rgb {
        r: 229,
        g: 166,
        b: 108,
    };
    theme.lighting.sun_spill = Rgb {
        r: 249,
        g: 226,
        b: 181,
    };
    theme.lighting.ceiling_pool = Rgb {
        r: 255,
        g: 246,
        b: 223,
    };
    theme.lighting.floor_lamp_halo = Rgb {
        r: 226,
        g: 193,
        b: 139,
    };
    theme.lighting.night_tint = Rgb {
        r: 25,
        g: 31,
        b: 39,
    };

    theme.furniture.wood_top = Rgb {
        r: 119,
        g: 83,
        b: 52,
    };
    theme.furniture.wood_trim = Rgb {
        r: 72,
        g: 49,
        b: 35,
    };
    theme.furniture.rug_field = Rgb {
        r: 52,
        g: 50,
        b: 47,
    };
    theme.furniture.rug_trim = Rgb {
        r: 28,
        g: 28,
        b: 27,
    };
    theme.furniture.rug_accent = Rgb {
        r: 172,
        g: 130,
        b: 73,
    };
    theme.furniture.chair_seat = Rgb {
        r: 55,
        g: 42,
        b: 34,
    };
    theme.furniture.chair_trim = Rgb {
        r: 30,
        g: 27,
        b: 25,
    };

    theme.ui.neon_brand = Rgb {
        r: 206,
        g: 167,
        b: 104,
    };
    theme.ui.neon_star = Rgb {
        r: 243,
        g: 218,
        b: 166,
    };

    theme
}

pub static SUCCESSION: Theme = succession_theme();
