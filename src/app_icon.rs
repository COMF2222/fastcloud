//! Fastcloud's application icon, rendered at any square size.

pub const ICON_SIZE: u32 = 64;

/// Render the sunset tile and its white waveform-cloud mark as straight RGBA.
pub fn rgba(size: u32) -> Vec<u8> {
    if size == 0 {
        return Vec::new();
    }

    let side = size as usize;
    let pixel = 1.0 / size as f32;
    let mut rgba = Vec::with_capacity(side * side * 4);

    for y in 0..side {
        for x in 0..side {
            let px = (x as f32 + 0.5) / size as f32 - 0.5;
            let py = (y as f32 + 0.5) / size as f32 - 0.5;
            let tile_coverage = coverage(tile_sdf(px, py), pixel);

            let gradient = ((px + py + 1.0) * 0.5).clamp(0.0, 1.0);
            let base = [
                lerp(0xFF, 0xF0, gradient),
                lerp(0x7A, 0x2D, gradient),
                lerp(0x18, 0x58, gradient),
            ];
            let mark_coverage = coverage(mark_sdf(px, py), pixel) * tile_coverage;
            let colour = [
                lerp(base[0], 0xFF, mark_coverage),
                lerp(base[1], 0xFF, mark_coverage),
                lerp(base[2], 0xFF, mark_coverage),
            ];

            rgba.extend_from_slice(&[
                colour[0],
                colour[1],
                colour[2],
                (tile_coverage * 255.0).round() as u8,
            ]);
        }
    }

    rgba
}

fn coverage(distance: f32, pixel: f32) -> f32 {
    (0.5 - distance / pixel).clamp(0.0, 1.0)
}

fn lerp(from: u8, to: u8, amount: f32) -> u8 {
    (from as f32 + (to as f32 - from as f32) * amount).round() as u8
}

fn tile_sdf(x: f32, y: f32) -> f32 {
    rounded_box_sdf(x, y, 0.46, 0.46, 0.13)
}

fn mark_sdf(x: f32, y: f32) -> f32 {
    bars_sdf(x, y).min(cloud_sdf(x, y))
}

fn cloud_sdf(x: f32, y: f32) -> f32 {
    const CIRCLES: [(f32, f32, f32); 3] = [
        (0.05, 0.035, 0.105),
        (0.17, -0.045, 0.145),
        (0.30, 0.035, 0.105),
    ];
    let mut cloud = rounded_box_sdf(x - 0.18, y - 0.09, 0.19, 0.065, 0.025);
    for (cx, cy, radius) in CIRCLES {
        cloud = cloud.min(((x - cx).powi(2) + (y - cy).powi(2)).sqrt() - radius);
    }
    cloud
}

fn bars_sdf(x: f32, y: f32) -> f32 {
    const BARS: [(f32, f32); 4] = [(-0.27, 0.07), (-0.20, 0.11), (-0.13, 0.15), (-0.06, 0.19)];
    const HALF_WIDTH: f32 = 0.025;
    const CENTRE_Y: f32 = 0.02;

    let mut bars = f32::MAX;
    for (centre_x, half_height) in BARS {
        bars = bars.min(rounded_box_sdf(
            x - centre_x,
            y - CENTRE_Y,
            HALF_WIDTH,
            half_height,
            HALF_WIDTH,
        ));
    }
    bars
}

fn rounded_box_sdf(x: f32, y: f32, half_width: f32, half_height: f32, radius: f32) -> f32 {
    let dx = x.abs() - half_width + radius;
    let dy = y.abs() - half_height + radius;
    let outside = (dx.max(0.0).powi(2) + dy.max(0.0).powi(2)).sqrt();
    outside + dx.max(dy).min(0.0) - radius
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel_at(rgba: &[u8], size: u32, x: u32, y: u32) -> &[u8] {
        let start = ((y * size + x) * 4) as usize;
        &rgba[start..start + 4]
    }

    #[test]
    fn icon_renderer_returns_exact_rgba_size() {
        for size in [16, 24, 32, 48, 64, 128, 256] {
            assert_eq!(rgba(size).len(), (size * size * 4) as usize);
        }
    }

    #[test]
    fn icon_has_transparent_corners_and_an_opaque_tile() {
        let pixels = rgba(ICON_SIZE);
        assert_eq!(pixel_at(&pixels, ICON_SIZE, 0, 0)[3], 0);
        assert_eq!(pixel_at(&pixels, ICON_SIZE, 63, 63)[3], 0);
        assert_eq!(pixel_at(&pixels, ICON_SIZE, 32, 32)[3], 255);
    }

    #[test]
    fn icon_keeps_the_mark_white_and_the_background_colourful() {
        let pixels = rgba(ICON_SIZE);
        let mark = pixel_at(&pixels, ICON_SIZE, 43, 29);
        assert!(mark[..3].iter().all(|channel| *channel > 245));

        let top_left = pixel_at(&pixels, ICON_SIZE, 12, 12);
        let bottom_right = pixel_at(&pixels, ICON_SIZE, 52, 52);
        assert!(top_left[1] > bottom_right[1]);
        assert!(top_left[2] < bottom_right[2]);
    }

    #[test]
    fn icon_edges_are_antialiased_at_tray_size() {
        let pixels = rgba(16);
        let soft_edge_pixels = pixels
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| (1..=254).contains(&pixel[3]))
            .count();
        assert!(
            soft_edge_pixels >= 8,
            "only {soft_edge_pixels} soft edge pixels"
        );
    }
}
