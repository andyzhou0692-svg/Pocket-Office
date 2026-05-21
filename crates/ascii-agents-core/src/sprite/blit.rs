use crate::sprite::{Frame, Rgb, RgbBuffer};

/// Blit a sprite frame into `dst` with top-left at `(dst_x, dst_y)`.
/// Transparent (None) pixels leave `dst` unchanged. Out-of-bounds pixels
/// are silently clipped.
pub fn blit_frame(frame: &Frame, dst_x: u16, dst_y: u16, dst: &mut RgbBuffer) {
    for fy in 0..frame.height {
        for fx in 0..frame.width {
            let i = (fy as usize) * (frame.width as usize) + (fx as usize);
            let Some(rgb) = frame.pixels[i] else {
                continue;
            };
            let x = dst_x.saturating_add(fx);
            let y = dst_y.saturating_add(fy);
            if x >= dst.width || y >= dst.height {
                continue;
            }
            dst.put(x, y, rgb);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HalfCell {
    pub fg: Rgb,
    pub bg: Rgb,
}

/// Convert an RGB buffer into a 2D grid of half-block cells.
/// Each row pair becomes one cell row: `fg` = upper pixel, `bg` = lower pixel.
/// Odd-height buffers pad the last cell by duplicating the final row into `bg`.
pub fn half_block_cells(buf: &RgbBuffer) -> Vec<Vec<HalfCell>> {
    let w = buf.width as usize;
    let h = buf.height as usize;
    if h == 0 || w == 0 {
        return Vec::new();
    }
    let cell_rows = (h + 1) / 2;
    let mut out: Vec<Vec<HalfCell>> = Vec::with_capacity(cell_rows);
    for cy in 0..cell_rows {
        let py_top = cy * 2;
        let py_bot = (py_top + 1).min(h - 1);
        let mut row = Vec::with_capacity(w);
        for x in 0..w {
            let fg = buf.pixels[py_top * w + x];
            let bg = buf.pixels[py_bot * w + x];
            row.push(HalfCell { fg, bg });
        }
        out.push(row);
    }
    out
}
