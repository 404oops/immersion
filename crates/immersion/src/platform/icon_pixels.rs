//! Pixel helpers shared by the platform icon renderers. OS document icons
//! carry different amounts of built-in padding (a fruit-shaped icon floats
//! in its canvas, a page-shaped one fills it), so each icon is trimmed to
//! its visible pixels and re-centred on a square with the same small margin
//! before it reaches the list. Self-contained (std and `image` only) so the
//! Windows renderer can be compile-checked from a scratch crate.

use std::io::Cursor;

/// Alpha below this counts as background: it excludes the faint drop
/// shadows some icons paint around their artwork.
const VISIBLE_ALPHA: u8 = 28;
/// Breathing room around the artwork, as a fraction of its larger side.
const MARGIN_FRACTION: f32 = 0.06;

/// Crops straight-alpha RGBA pixels to their visible bounding box and places
/// that on a square canvas with a uniform margin. Returns the input unchanged
/// when nothing is visible.
pub fn trim_to_content(width: u32, height: u32, rgba: &[u8]) -> (u32, u32, Vec<u8>) {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 || rgba.len() < w * h * 4 {
        return (width, height, rgba.to_vec());
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (w, h, 0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            if rgba[(y * w + x) * 4 + 3] >= VISIBLE_ALPHA {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    if min_x > max_x || min_y > max_y {
        return (width, height, rgba.to_vec());
    }
    let box_w = max_x - min_x + 1;
    let box_h = max_y - min_y + 1;
    let side = box_w.max(box_h);
    let margin = ((side as f32) * MARGIN_FRACTION).ceil() as usize;
    let out_side = side + 2 * margin;
    let dst_x0 = margin + (side - box_w) / 2;
    let dst_y0 = margin + (side - box_h) / 2;

    let mut out = vec![0u8; out_side * out_side * 4];
    for y in 0..box_h {
        let src = ((min_y + y) * w + min_x) * 4;
        let dst = ((dst_y0 + y) * out_side + dst_x0) * 4;
        out[dst..dst + box_w * 4].copy_from_slice(&rgba[src..src + box_w * 4]);
    }
    (out_side as u32, out_side as u32, out)
}

/// PNG bytes of straight-alpha RGBA pixels.
pub fn encode_png(width: u32, height: u32, rgba: Vec<u8>) -> Option<Vec<u8>> {
    let image = image::RgbaImage::from_raw(width, height, rgba)?;
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

/// Trims, squares and encodes in one go.
pub fn trimmed_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let (w, h, pixels) = trim_to_content(width, height, rgba);
    encode_png(w, h, pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(w: usize, h: usize, visible: &[(usize, usize)]) -> Vec<u8> {
        let mut px = vec![0u8; w * h * 4];
        for &(x, y) in visible {
            let i = (y * w + x) * 4;
            px[i..i + 4].copy_from_slice(&[200, 100, 50, 255]);
        }
        px
    }

    #[test]
    fn crops_to_visible_pixels_on_a_square_with_margin() {
        // A 2x3 block floating in a 10x10 canvas.
        let visible: Vec<(usize, usize)> =
            (2..4).flat_map(|x| (5..8).map(move |y| (x, y))).collect();
        let (w, h, out) = trim_to_content(10, 10, &canvas(10, 10, &visible));
        // Larger side 3, margin ceil(0.18) = 1 -> 5x5.
        assert_eq!((w, h), (5, 5));
        let alpha = |x: usize, y: usize| out[(y * 5 + x) * 4 + 3];
        // Block centred horizontally in the 3-wide square: columns 1..=2.
        assert_eq!(alpha(1, 1), 255);
        assert_eq!(alpha(2, 3), 255);
        assert_eq!(alpha(0, 1), 0);
        assert_eq!(alpha(3, 1), 0);
        assert_eq!(alpha(1, 0), 0);
        assert_eq!(alpha(1, 4), 0);
    }

    #[test]
    fn faint_shadow_does_not_count_as_content() {
        let mut px = canvas(6, 6, &[(2, 2)]);
        px[(5 * 6 + 5) * 4 + 3] = 10; // a shadow pixel in the far corner
        let (w, h, _) = trim_to_content(6, 6, &px);
        assert_eq!((w, h), (3, 3));
    }

    #[test]
    fn fully_transparent_icon_is_returned_unchanged() {
        let px = vec![0u8; 4 * 4 * 4];
        let (w, h, out) = trim_to_content(4, 4, &px);
        assert_eq!((w, h, out.len()), (4, 4, 64));
    }
}
