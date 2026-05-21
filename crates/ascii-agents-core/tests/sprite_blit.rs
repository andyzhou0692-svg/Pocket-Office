use ascii_agents_core::sprite::blit::{blit_frame, half_block_cells, HalfCell};
use ascii_agents_core::sprite::{Frame, Pixel, Rgb, RgbBuffer};

fn px(r: u8, g: u8, b: u8) -> Pixel {
    Some(Rgb(r, g, b))
}
fn t() -> Pixel {
    None
}

#[test]
fn blit_writes_opaque_pixels_and_skips_transparent() {
    let frame = Frame {
        width: 2,
        height: 2,
        pixels: vec![px(10, 0, 0), t(), t(), px(0, 0, 30)],
    };
    let mut buf = RgbBuffer::filled(4, 4, Rgb(99, 99, 99));
    blit_frame(&frame, 1, 1, &mut buf);

    assert_eq!(buf.get(1, 1), Rgb(10, 0, 0));
    assert_eq!(buf.get(2, 1), Rgb(99, 99, 99));
    assert_eq!(buf.get(1, 2), Rgb(99, 99, 99));
    assert_eq!(buf.get(2, 2), Rgb(0, 0, 30));
    assert_eq!(buf.get(0, 0), Rgb(99, 99, 99));
}

#[test]
fn blit_ignores_out_of_bounds() {
    let frame = Frame {
        width: 3,
        height: 3,
        pixels: vec![px(1, 1, 1); 9],
    };
    let mut buf = RgbBuffer::filled(2, 2, Rgb(0, 0, 0));
    blit_frame(&frame, 1, 1, &mut buf);
    assert_eq!(buf.get(1, 1), Rgb(1, 1, 1));
}

#[test]
fn half_block_cells_pairs_rows() {
    let buf = RgbBuffer {
        width: 2,
        height: 4,
        pixels: vec![
            Rgb(1, 0, 0),
            Rgb(2, 0, 0),
            Rgb(3, 0, 0),
            Rgb(4, 0, 0),
            Rgb(5, 0, 0),
            Rgb(6, 0, 0),
            Rgb(7, 0, 0),
            Rgb(8, 0, 0),
        ],
    };
    let cells = half_block_cells(&buf);
    assert_eq!(cells.len(), 2);
    assert_eq!(cells[0].len(), 2);
    assert_eq!(
        cells[0][0],
        HalfCell {
            fg: Rgb(1, 0, 0),
            bg: Rgb(3, 0, 0)
        }
    );
    assert_eq!(
        cells[0][1],
        HalfCell {
            fg: Rgb(2, 0, 0),
            bg: Rgb(4, 0, 0)
        }
    );
    assert_eq!(
        cells[1][0],
        HalfCell {
            fg: Rgb(5, 0, 0),
            bg: Rgb(7, 0, 0)
        }
    );
    assert_eq!(
        cells[1][1],
        HalfCell {
            fg: Rgb(6, 0, 0),
            bg: Rgb(8, 0, 0)
        }
    );
}

#[test]
fn half_block_cells_pads_odd_height_with_repeated_row() {
    let buf = RgbBuffer {
        width: 1,
        height: 3,
        pixels: vec![Rgb(1, 0, 0), Rgb(2, 0, 0), Rgb(3, 0, 0)],
    };
    let cells = half_block_cells(&buf);
    assert_eq!(cells.len(), 2);
    assert_eq!(
        cells[0][0],
        HalfCell {
            fg: Rgb(1, 0, 0),
            bg: Rgb(2, 0, 0)
        }
    );
    assert_eq!(
        cells[1][0],
        HalfCell {
            fg: Rgb(3, 0, 0),
            bg: Rgb(3, 0, 0)
        }
    );
}
