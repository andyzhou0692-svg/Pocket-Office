use pixtuoid_core::sprite::blit::blit_frame;
use pixtuoid_core::sprite::{Frame, Pixel, Rgb, RgbBuffer};

fn px(r: u8, g: u8, b: u8) -> Pixel {
    Some(Rgb { r, g, b })
}
fn t() -> Pixel {
    None
}

#[test]
fn blit_writes_opaque_pixels_and_skips_transparent() {
    let frame = Frame::from_pixels(2, 2, vec![px(10, 0, 0), t(), t(), px(0, 0, 30)]);
    let mut buf = RgbBuffer::filled(
        4,
        4,
        Rgb {
            r: 99,
            g: 99,
            b: 99,
        },
    );
    blit_frame(&frame, 1, 1, &mut buf);

    assert_eq!(buf.get(1, 1), Rgb { r: 10, g: 0, b: 0 });
    assert_eq!(
        buf.get(2, 1),
        Rgb {
            r: 99,
            g: 99,
            b: 99
        }
    );
    assert_eq!(
        buf.get(1, 2),
        Rgb {
            r: 99,
            g: 99,
            b: 99
        }
    );
    assert_eq!(buf.get(2, 2), Rgb { r: 0, g: 0, b: 30 });
    assert_eq!(
        buf.get(0, 0),
        Rgb {
            r: 99,
            g: 99,
            b: 99
        }
    );
}

#[test]
fn blit_ignores_out_of_bounds() {
    let frame = Frame::from_pixels(3, 3, vec![px(1, 1, 1); 9]);
    let mut buf = RgbBuffer::filled(2, 2, Rgb { r: 0, g: 0, b: 0 });
    blit_frame(&frame, 1, 1, &mut buf);
    assert_eq!(buf.get(1, 1), Rgb { r: 1, g: 1, b: 1 });
}

#[test]
fn x2_blit_uses_subcolumns_to_round_a_proven_sprite_corner() {
    let red = Rgb {
        r: 180,
        g: 40,
        b: 30,
    };
    let bg = Rgb {
        r: 10,
        g: 20,
        b: 30,
    };
    let frame = Frame::from_pixels(
        3,
        3,
        vec![
            t(),
            t(),
            t(),
            t(),
            Some(red),
            Some(red),
            t(),
            Some(red),
            Some(red),
        ],
    );
    let mut buf = RgbBuffer::filled_x2(3, 3, bg);

    blit_frame(&frame, 0, 0, &mut buf);

    assert_eq!(buf.physical_get(2, 1), bg);
    assert_eq!(buf.physical_get(3, 1), red);
}

#[test]
fn x2_blit_narrows_an_isolated_feature_to_one_subcolumn() {
    let skin = Rgb {
        r: 220,
        g: 170,
        b: 120,
    };
    let eye = Rgb {
        r: 25,
        g: 30,
        b: 35,
    };
    let frame = Frame::from_pixels(
        3,
        3,
        vec![
            Some(skin),
            Some(skin),
            Some(skin),
            Some(skin),
            Some(eye),
            Some(skin),
            Some(skin),
            Some(skin),
            Some(skin),
        ],
    );
    let mut buf = RgbBuffer::filled_x2(3, 3, skin);

    blit_frame(&frame, 0, 0, &mut buf);

    assert_eq!(buf.physical_get(2, 1), eye);
    assert_eq!(buf.physical_get(3, 1), skin);
}

#[test]
fn x2_blit_does_not_bevel_between_two_opaque_sprite_colours() {
    let skin = Rgb {
        r: 220,
        g: 170,
        b: 120,
    };
    let hair = Rgb {
        r: 35,
        g: 25,
        b: 20,
    };
    let frame = Frame::from_pixels(
        3,
        3,
        vec![
            Some(hair),
            Some(hair),
            Some(skin),
            Some(hair),
            Some(skin),
            Some(skin),
            Some(hair),
            Some(skin),
            Some(skin),
        ],
    );
    let mut dst = RgbBuffer::filled_x2(3, 3, Rgb { r: 1, g: 2, b: 3 });

    blit_frame(&frame, 0, 0, &mut dst);

    assert_eq!(dst.physical_get(2, 1), skin);
    assert_eq!(dst.physical_get(3, 1), skin);
}
