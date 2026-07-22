use crate::sprite::{Frame, RgbBuffer};

/// Blit a sprite frame into `dst` with top-left at `(dst_x, dst_y)`.
/// Transparent (None) pixels leave `dst` unchanged. Out-of-bounds pixels
/// are silently clipped.
pub fn blit_frame(frame: &Frame, dst_x: u16, dst_y: u16, dst: &mut RgbBuffer) {
    for fy in 0..frame.height {
        for fx in 0..frame.width {
            let i = (fy as usize) * (frame.width as usize) + (fx as usize);
            let Some(rgb) = frame.as_slice()[i] else {
                continue;
            };
            let x = dst_x.saturating_add(fx);
            let y = dst_y.saturating_add(fy);
            if x >= dst.width() || y >= dst.height() {
                continue;
            }
            if dst.horizontal_scale() == 1 {
                dst.put(x, y, rgb);
                continue;
            }

            let sample = |sx: i32, sy: i32| {
                if sx < 0 || sy < 0 || sx >= frame.width as i32 || sy >= frame.height as i32 {
                    return None;
                }
                frame.as_slice()[(sy as usize) * (frame.width as usize) + sx as usize]
            };
            let center = Some(rgb);
            let left = sample(fx as i32 - 1, fy as i32);
            let right = sample(fx as i32 + 1, fy as i32);
            let up = sample(fx as i32, fy as i32 - 1);
            let down = sample(fx as i32, fy as i32 + 1);
            // Only carve half-pixels at the transparent silhouette. Treating
            // two opaque palette neighbours as a diagonal blends hair into
            // skin, skin into clothing, and furniture panels into their
            // frames. That produced the visibly split faces and broken desk
            // edges in the first x2 production capture.
            let mut detail_left = if left.is_none() && left == up && left != down && up != right {
                left
            } else if left.is_none() && left == down && left != up && down != right {
                left
            } else {
                center
            };
            let mut detail_right = if right.is_none() && up == right && up != left && right != down
            {
                right
            } else if right.is_none() && down == right && down != left && right != up {
                right
            } else {
                center
            };
            if left == right && right == up && up == down && left != center {
                let surrounding = left;
                if fx < frame.width / 2 {
                    detail_left = surrounding;
                    detail_right = center;
                } else {
                    detail_left = center;
                    detail_right = surrounding;
                }
            }
            let physical_x = x.saturating_mul(2);
            if let Some(color) = detail_left {
                dst.physical_put(physical_x, y, color);
            }
            if let Some(color) = detail_right {
                dst.physical_put(physical_x + 1, y, color);
            }
        }
    }
}
