//! FastPotify's MIT-licensed classic artwork, adapted to SoundCloud orange.
//! The original archive and license live in assets/skins; geometry stays in
//! the standard Winamp format so user-supplied skins keep working too.

use super::{Bitmap, Sheet, Skin};
use egui::Color32;

pub fn load() -> anyhow::Result<Skin> {
    let mut skin = Skin::from_wsz(
        "Fastcloud",
        include_bytes!("../../assets/skins/fastpotify-base.wsz").to_vec(),
    )?;
    for bitmap in skin.sheets.values_mut() {
        for pixel in bitmap.rgba.chunks_exact_mut(4) {
            let [r, g, b] = orange([pixel[0], pixel[1], pixel[2]]);
            pixel[..3].copy_from_slice(&[r, g, b]);
        }
    }
    for color in &mut skin.vis_colors {
        *color = recolor(*color);
    }
    skin.playlist.normal = recolor(skin.playlist.normal);
    skin.playlist.current = recolor(skin.playlist.current);
    skin.playlist.selected_background = recolor(skin.playlist.selected_background);
    // The font is used both for the window title and the live marquee.
    let font = skin.sheet(Sheet::Text).cloned();
    if let (Some(font), Some(bar)) = (font, skin.sheets.get_mut(&Sheet::Titlebar)) {
        for (y, active, shade) in [
            (0, true, false),
            (15, false, false),
            (29, true, true),
            (42, false, true),
            (57, true, false),
            (72, false, false),
        ] {
            let x = if shade { 47 } else { 137 };
            let width = if shade { 50 } else { 56 };
            for py in y + 3..y + 11 {
                for px in x..x + width {
                    put(bar, px, py, [29, 33, 39]);
                }
            }
            let ink = if active {
                [242, 244, 246]
            } else {
                [110, 119, 132]
            };
            let start = if shade { x } else { 27 + (275 - 44) / 2 };
            for (i, letter) in "FASTCLOUD".chars().enumerate() {
                for gy in 0..6 {
                    for gx in 0..5 {
                        if glyph_lit(&font, letter, gx, gy).unwrap_or(false) {
                            put(bar, start + i as u32 * 5 + gx, y + 4 + gy, ink);
                        }
                    }
                }
            }
        }
    }
    Ok(skin)
}

fn put(bitmap: &mut Bitmap, x: u32, y: u32, rgb: [u8; 3]) {
    if x < bitmap.width && y < bitmap.height {
        let at = ((y * bitmap.width + x) * 4) as usize;
        bitmap.rgba[at..at + 3].copy_from_slice(&rgb);
    }
}

fn orange([r, g, b]: [u8; 3]) -> [u8; 3] {
    if g > r && g > b {
        [g, r.saturating_add((g - r) / 3), r / 3]
    } else {
        [r, g, b]
    }
}

fn recolor(color: Color32) -> Color32 {
    let [r, g, b] = orange([color.r(), color.g(), color.b()]);
    Color32::from_rgb(r, g, b)
}

/// Read the skin's actual 5×6 face. A blank fifth column keeps letters apart.
pub fn glyph_lit(font: &Bitmap, ch: char, x: u32, y: u32) -> Option<bool> {
    let ch = match ch {
        '–' | '—' => '-',
        ch => ch.to_ascii_uppercase(),
    };
    if ch == ' ' {
        return Some(false);
    }
    let rows = [
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ\"@",
        "0123456789….:()-'!_+\\/[]^&%,=$#",
        "ÅÖÄ?*",
    ];
    let (row, col) = rows
        .iter()
        .enumerate()
        .find_map(|(row, chars)| chars.chars().position(|c| c == ch).map(|col| (row, col)))?;
    let at = (((row as u32 * 6 + y) * font.width + col as u32 * 5 + x) * 4) as usize;
    let bg = ((font.width - 1) * 4) as usize;
    let pixel = font.rgba.get(at..at + 3)?;
    Some(pixel != font.rgba.get(bg..bg + 3)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_has_all_panels_and_readable_spaced_letters() {
        let skin = load().expect("bundled skin");
        for sheet in Sheet::ALL {
            assert!(skin.has(sheet), "{sheet:?}");
        }
        let font = skin.sheet(Sheet::Text).unwrap();
        assert!((0..6).all(|y| glyph_lit(font, 'A', 4, y) == Some(false)));
        assert!((0..6).any(|y| glyph_lit(font, 'A', 0, y) == Some(true)));
        assert!(skin.playlist.current.r() >= skin.playlist.current.g());
    }
}
