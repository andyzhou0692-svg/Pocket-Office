use pixtuoid_core::sprite::Rgb;

use super::*;

const fn goldman_theme() -> Theme {
    let mut theme = super::normal::NORMAL_THEME;
    theme.name = GOLDMAN_THEME_NAME;

    theme.surface.wall = Rgb {
        r: 222,
        g: 220,
        b: 210,
    };
    theme.surface.wall_trim = Rgb {
        r: 180,
        g: 184,
        b: 184,
    };
    theme.surface.baseboard = Rgb {
        r: 64,
        g: 72,
        b: 82,
    };
    theme.surface.carpet_base = Rgb {
        r: 58,
        g: 78,
        b: 105,
    };
    theme.surface.carpet_light = Rgb {
        r: 72,
        g: 94,
        b: 123,
    };
    theme.surface.carpet_dark = Rgb {
        r: 45,
        g: 63,
        b: 88,
    };
    theme.surface.window_frame = Rgb {
        r: 28,
        g: 38,
        b: 48,
    };
    theme.surface.bg_fallback = Rgb {
        r: 36,
        g: 50,
        b: 66,
    };

    theme.office.room_wall_body = Rgb {
        r: 228,
        g: 226,
        b: 216,
    };
    theme.office.room_wall_trim_light = Rgb {
        r: 244,
        g: 241,
        b: 228,
    };
    theme.office.room_wall_trim_dark = Rgb {
        r: 148,
        g: 154,
        b: 158,
    };
    theme.office.cubicle_divider = Rgb {
        r: 186,
        g: 190,
        b: 188,
    };
    theme.office.runner_base = Rgb {
        r: 52,
        g: 69,
        b: 92,
    };
    theme.office.runner_stripe = Rgb {
        r: 68,
        g: 88,
        b: 116,
    };
    theme.office.runner_edge = Rgb {
        r: 34,
        g: 48,
        b: 68,
    };
    theme.office.neon_panel_bg = Rgb {
        r: 235,
        g: 232,
        b: 220,
    };
    theme.office.neon_frame_base = Rgb {
        r: 78,
        g: 110,
        b: 146,
    };
    theme.office.building_dark = Rgb {
        r: 45,
        g: 58,
        b: 74,
    };
    theme.office.building_light = Rgb {
        r: 92,
        g: 108,
        b: 125,
    };
    theme.office.city_lit_windows = [
        Rgb {
            r: 246,
            g: 222,
            b: 164,
        },
        Rgb {
            r: 178,
            g: 210,
            b: 238,
        },
        Rgb {
            r: 232,
            g: 236,
            b: 224,
        },
    ];
    theme.office.city_dark_window = Rgb {
        r: 40,
        g: 54,
        b: 70,
    };
    theme.office.clock_rim = Rgb {
        r: 96,
        g: 104,
        b: 112,
    };
    theme.office.clock_face = Rgb {
        r: 242,
        g: 240,
        b: 232,
    };
    theme.office.clock_hand = Rgb {
        r: 28,
        g: 34,
        b: 42,
    };
    theme.office.shadow = Rgb {
        r: 35,
        g: 43,
        b: 52,
    };

    theme.lighting.day_sky_a = Rgb {
        r: 128,
        g: 184,
        b: 224,
    };
    theme.lighting.day_sky_b = Rgb {
        r: 184,
        g: 218,
        b: 240,
    };
    theme.lighting.night_sky_a = Rgb {
        r: 18,
        g: 32,
        b: 58,
    };
    theme.lighting.night_sky_b = Rgb {
        r: 32,
        g: 54,
        b: 84,
    };
    theme.lighting.twilight_a = Rgb {
        r: 218,
        g: 138,
        b: 94,
    };
    theme.lighting.twilight_b = Rgb {
        r: 238,
        g: 186,
        b: 132,
    };
    theme.lighting.sun_spill = Rgb {
        r: 255,
        g: 238,
        b: 194,
    };
    theme.lighting.ceiling_pool = Rgb {
        r: 255,
        g: 250,
        b: 232,
    };
    theme.lighting.floor_lamp_halo = Rgb {
        r: 250,
        g: 224,
        b: 176,
    };
    theme.lighting.night_tint = Rgb {
        r: 24,
        g: 34,
        b: 54,
    };

    theme.furniture.wood_top = Rgb {
        r: 218,
        g: 190,
        b: 145,
    };
    theme.furniture.wood_trim = Rgb {
        r: 164,
        g: 132,
        b: 91,
    };
    theme.furniture.rug_field = Rgb {
        r: 49,
        g: 65,
        b: 88,
    };
    theme.furniture.rug_trim = Rgb {
        r: 31,
        g: 43,
        b: 60,
    };
    theme.furniture.rug_accent = Rgb {
        r: 79,
        g: 104,
        b: 137,
    };
    theme.furniture.magazine = Rgb {
        r: 236,
        g: 232,
        b: 218,
    };
    theme.furniture.magazine_trim = Rgb {
        r: 73,
        g: 115,
        b: 160,
    };
    theme.furniture.chair_seat = Rgb {
        r: 24,
        g: 29,
        b: 35,
    };
    theme.furniture.chair_trim = Rgb {
        r: 50,
        g: 57,
        b: 64,
    };
    theme.furniture.coffee_cup = Rgb {
        r: 236,
        g: 234,
        b: 226,
    };
    theme.furniture.coffee_cup_shadow = Rgb {
        r: 151,
        g: 157,
        b: 160,
    };

    theme.effects.monitor_frame_lit = Rgb {
        r: 72,
        g: 142,
        b: 207,
    };
    theme.effects.waiting_bubble = Rgb {
        r: 235,
        g: 239,
        b: 242,
    };
    theme.ui.neon_brand = Rgb {
        r: 67,
        g: 118,
        b: 169,
    };
    theme.ui.neon_star = Rgb {
        r: 218,
        g: 188,
        b: 112,
    };
    theme
}

pub static GOLDMAN: Theme = goldman_theme();
