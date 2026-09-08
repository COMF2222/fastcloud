//! Application fonts.
//!
//! ## What ships, and why not SoundCloud's own face
//!
//! [Inter](https://rsms.me/inter/) is the interface font: it is a close
//! neighbour of the grotesque soundcloud.com uses, and it is under the SIL
//! Open Font License, so it can be redistributed with an MIT project.
//!
//! soundcloud.com itself sets `--font-main: "Söhne"` — a commercial face from
//! Klim Type Foundry, which cannot be bundled here. A user who owns it can
//! point Fastcloud at the file
//! ([`crate::config::Settings::interface_font`]), and it then leads the chain;
//! see [`custom_face`].
//!
//! Behind the interface font come the bundled egui faces (Ubuntu, the
//! monochrome emoji-icon-font, NotoEmoji) and one system face per script Inter
//! does not cover — memory-mapped, see [`crate::system_fonts`].
//!
//! ## Weights
//!
//! Weights are real font families, not `RichText::strong()`: egui's `strong()`
//! only brightens the colour, so a UI built on it renders every label at
//! weight 400. Use [`crate::ui::theme::Type`] (or `medium`/`bold`) to pick a
//! weight; they name the families registered here.
//!
//! ## Faces that claim a character and draw nothing
//!
//! epaint picks the first face in a family that *claims* a char — its
//! `resolve_face` consults the charmap only. A face whose cmap maps `…` to an
//! empty outline therefore renders nothing instead of falling through, and `…`
//! is what epaint appends to every truncated label. [`strip_blank_glyphs`]
//! unmaps those codepoints in memory before the face is installed, which is
//! what makes an arbitrary user-supplied font safe to put first.

use eframe::egui;

pub const INTER_LATIN_400: &[u8] = include_bytes!("../assets/fonts/inter-latin-400.ttf");
pub const INTER_CYRILLIC_400: &[u8] = include_bytes!("../assets/fonts/inter-cyrillic-400.ttf");
pub const INTER_LATIN_500: &[u8] = include_bytes!("../assets/fonts/inter-latin-500.ttf");
pub const INTER_CYRILLIC_500: &[u8] = include_bytes!("../assets/fonts/inter-cyrillic-500.ttf");
pub const INTER_LATIN_700: &[u8] = include_bytes!("../assets/fonts/inter-latin-700.ttf");
pub const INTER_CYRILLIC_700: &[u8] = include_bytes!("../assets/fonts/inter-cyrillic-700.ttf");

/// Named families for the heavier weights (see module docs).
pub const FAMILY_MEDIUM: &str = "inter-medium";
pub const FAMILY_BOLD: &str = "inter-bold";

/// The name a user-supplied interface face is registered under.
const CUSTOM: &str = "interface";

/// Largest font file accepted from settings. Real interface faces are well
/// under a megabyte; a pan-CJK collection would be pointless here and slow to
/// parse on every start.
const MAX_CUSTOM_FONT_BYTES: u64 = 8 * 1024 * 1024;

/// Every non-ASCII glyph used by the UI. Covered by the test below
/// through renderable (non-bitmap) fonts.
#[cfg(test)]
pub const ICON_CHARS: &[char] = &[
    '▶', '⏸', '⏮', '⏭', '🔀', '🔁', '🔊', '🔈', '♪', '♥', '♡', '☁', '🔔', '👤', '+', '🗑', '💬',
    '👥', '☰', '⬇',
];

/// Basic Latin + Cyrillic samples the UI must render (track titles, names),
/// plus the punctuation epaint and the views lean on.
#[cfg(test)]
pub const TEXT_SAMPLE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789АБВГДЕЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯабвгдежзийклмнопрстуфхцчшщъыьэюяЁё—–…«»·";

/// Read and prepare a user-supplied interface face.
///
/// `None` when there is no such setting, the file cannot be read, it is
/// implausibly large, or it is not a font epaint could parse — a bad path must
/// not stop the app from starting, it just falls back to Inter.
pub fn custom_face(path: &std::path::Path) -> Option<Vec<u8>> {
    let size = std::fs::metadata(path)
        .map_err(|e| log::warn!("interface font {}: {e}", path.display()))
        .ok()?
        .len();
    if size == 0 || size > MAX_CUSTOM_FONT_BYTES {
        log::warn!(
            "interface font {} is {size} bytes; ignoring it",
            path.display()
        );
        return None;
    }
    let bytes = std::fs::read(path)
        .map_err(|e| log::warn!("interface font {}: {e}", path.display()))
        .ok()?;
    if skrifa::FontRef::new(&bytes).is_err() {
        log::warn!("interface font {} is invalid; using Inter", path.display());
        return None;
    }
    // Blank glyphs first: an arbitrary face may well claim `…` and draw
    // nothing, and it is about to lead the family (see the module docs).
    Some(strip_blank_glyphs(&bytes))
}

/// Unmap a face's blank glyphs so the fallback chain can serve them.
///
/// Rewrites the cmap **in memory**; the file on disk is untouched.
///
/// Two of the three cmap encodings can express a hole in place:
/// * format 4 `glyphIdArray` segments — write 0 (spec: missing glyph),
/// * format 4 single-codepoint `idDelta` segments — set the delta so the id
///   computes to 0,
/// * format 6 arrays — write 0.
///
/// Multi-codepoint delta segments cannot express a hole without resizing the
/// table, so a codepoint in one stays mapped; the UI draws the punctuation it
/// actually needs as shapes ([`crate::ui::widgets::chevron`]) for exactly that
/// reason. Table checksums and `head.checkSumAdjustment` are recomputed.
fn strip_blank_glyphs(bytes: &[u8]) -> Vec<u8> {
    /// Spaces and the soft hyphen are legitimately blank.
    const KEEP: [u32; 4] = [0x20, 0xA0, 0xAD, 0x200B];

    let mut d = bytes.to_vec();
    let Some(tables) = table_directory(&d) else {
        return d;
    };
    let Some(blank) = blank_glyph_ids(&d, &tables) else {
        return d;
    };
    let Some(&(_, cmap, _)) = tables.get("cmap") else {
        return d;
    };
    let is_blank = |gid: u16| gid != 0 && blank.contains(&gid);

    let ntab = be16(&d, cmap + 2) as usize;
    for i in 0..ntab {
        let rec = cmap + 4 + i * 8;
        let sub = cmap + be32(&d, rec + 4) as usize;
        if sub + 4 > d.len() {
            continue;
        }
        match be16(&d, sub) {
            4 => {
                let segx2 = be16(&d, sub + 6) as usize;
                let seg = segx2 / 2;
                let ends = sub + 14;
                let starts = ends + segx2 + 2;
                let deltas = starts + segx2;
                let ranges = deltas + segx2;
                if ranges + segx2 > d.len() {
                    continue;
                }
                for s in 0..seg {
                    let end = be16(&d, ends + s * 2) as u32;
                    let start = be16(&d, starts + s * 2) as u32;
                    if end == 0xFFFF || start > end {
                        continue;
                    }
                    let delta = be16(&d, deltas + s * 2);
                    let roff = be16(&d, ranges + s * 2) as usize;
                    if roff == 0 {
                        // A hole needs its own segment: only single-codepoint
                        // segments can be redirected to the missing glyph.
                        if start == end && !KEEP.contains(&start) {
                            let gid = (start as u16).wrapping_add(delta);
                            if is_blank(gid) {
                                let zeroing = (start as u16).wrapping_neg();
                                d[deltas + s * 2..deltas + s * 2 + 2]
                                    .copy_from_slice(&zeroing.to_be_bytes());
                            }
                        }
                        continue;
                    }
                    for cp in start..=end {
                        if KEEP.contains(&cp) {
                            continue;
                        }
                        let idx = ranges + s * 2 + roff + (cp - start) as usize * 2;
                        if idx + 2 > d.len() {
                            break;
                        }
                        let raw = be16(&d, idx);
                        if raw != 0 && is_blank(raw.wrapping_add(delta)) {
                            d[idx..idx + 2].copy_from_slice(&0u16.to_be_bytes());
                        }
                    }
                }
            }
            6 => {
                let first = be16(&d, sub + 6) as u32;
                let count = be16(&d, sub + 8) as usize;
                for k in 0..count {
                    let idx = sub + 10 + k * 2;
                    if idx + 2 > d.len() {
                        break;
                    }
                    let cp = first + k as u32;
                    if KEEP.contains(&cp) {
                        continue;
                    }
                    if is_blank(be16(&d, idx)) {
                        d[idx..idx + 2].copy_from_slice(&0u16.to_be_bytes());
                    }
                }
            }
            _ => {}
        }
    }
    fix_checksums(&mut d, &tables);
    d
}

type TableDir = std::collections::BTreeMap<String, (usize, usize, usize)>;

/// `tag -> (record offset, table offset, length)`.
fn table_directory(d: &[u8]) -> Option<TableDir> {
    if d.len() < 12 {
        return None;
    }
    let n = be16(d, 4) as usize;
    let mut out = TableDir::new();
    for i in 0..n {
        let rec = 12 + i * 16;
        if rec + 16 > d.len() {
            return None;
        }
        let tag = String::from_utf8_lossy(&d[rec..rec + 4]).into_owned();
        let off = be32(d, rec + 8) as usize;
        let len = be32(d, rec + 12) as usize;
        if off + len > d.len() {
            return None;
        }
        out.insert(tag, (rec, off, len));
    }
    Some(out)
}

/// Glyph ids whose `glyf` entry is empty (zero-length in `loca`).
fn blank_glyph_ids(d: &[u8], t: &TableDir) -> Option<std::collections::BTreeSet<u16>> {
    let (_, head, _) = *t.get("head")?;
    let (_, maxp, _) = *t.get("maxp")?;
    let (_, loca, loca_len) = *t.get("loca")?;
    let long = be16(d, head + 50) == 1;
    let num_glyphs = be16(d, maxp + 4) as usize;
    let mut out = std::collections::BTreeSet::new();
    for gid in 1..num_glyphs {
        let (a, b) = if long {
            let i = loca + gid * 4;
            if i + 8 > loca + loca_len {
                break;
            }
            (be32(d, i), be32(d, i + 4))
        } else {
            let i = loca + gid * 2;
            if i + 4 > loca + loca_len {
                break;
            }
            (be16(d, i) as u32 * 2, be16(d, i + 2) as u32 * 2)
        };
        if b == a {
            out.insert(gid as u16);
        }
    }
    Some(out)
}

fn fix_checksums(d: &mut [u8], t: &TableDir) {
    let Some(&(_, head, _)) = t.get("head") else {
        return;
    };
    d[head + 8..head + 12].copy_from_slice(&0u32.to_be_bytes());
    for &(rec, off, len) in t.values() {
        let sum = sum32(&d[off..off + len]);
        d[rec + 4..rec + 8].copy_from_slice(&sum.to_be_bytes());
    }
    let whole = sum32(d);
    let adjust = 0xB1B0_AFBAu32.wrapping_sub(whole);
    d[head + 8..head + 12].copy_from_slice(&adjust.to_be_bytes());
}

/// Sum of big-endian u32 words, zero-padded (OpenType checksum).
fn sum32(bytes: &[u8]) -> u32 {
    let mut total = 0u32;
    let mut chunks = bytes.chunks_exact(4);
    for c in &mut chunks {
        total = total.wrapping_add(u32::from_be_bytes([c[0], c[1], c[2], c[3]]));
    }
    let rest = chunks.remainder();
    if !rest.is_empty() {
        let mut word = [0u8; 4];
        word[..rest.len()].copy_from_slice(rest);
        total = total.wrapping_add(u32::from_be_bytes(word));
    }
    total
}

fn be16(d: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([d[at], d[at + 1]])
}

fn be32(d: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

/// Install the app's fonts on top of egui's bundled fallbacks. Idempotent.
///
/// `custom` is a prepared face from [`custom_face`], which leads every family
/// when present.
///
/// Order matters: the interface face, then Inter at the matching weight, then
/// Ubuntu (text fallback), then the monochrome emoji-icon-font
/// (transport/media glyphs), then the system script faces, and only then
/// NotoEmoji — whose colour bitmaps parley cannot render, so it must never
/// shadow the monochrome icon font.
pub fn install(ctx: &egui::Context, custom: Option<Vec<u8>>) {
    let mut fonts = egui::FontDefinitions::default();

    let mut add = |name: &str, data: &'static [u8]| {
        fonts
            .font_data
            .insert(name.to_owned(), egui::FontData::from_static(data).into());
    };
    add("Inter-Regular", INTER_LATIN_400);
    add("Inter-Regular-Cyrl", INTER_CYRILLIC_400);
    add("Inter-Medium", INTER_LATIN_500);
    add("Inter-Medium-Cyrl", INTER_CYRILLIC_500);
    add("Inter-Bold", INTER_LATIN_700);
    add("Inter-Bold-Cyrl", INTER_CYRILLIC_700);

    let has_custom = custom.is_some();
    if let Some(bytes) = custom {
        fonts.font_data.insert(
            CUSTOM.to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
    }

    const OURS: [&str; 7] = [
        CUSTOM,
        "Inter-Regular",
        "Inter-Regular-Cyrl",
        "Inter-Medium",
        "Inter-Medium-Cyrl",
        "Inter-Bold",
        "Inter-Bold-Cyrl",
    ];
    let lead: Vec<String> = if has_custom {
        vec![CUSTOM.to_owned()]
    } else {
        Vec::new()
    };

    if let Some(list) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        let bundled: Vec<String> = list
            .iter()
            .filter(|n| !OURS.contains(&n.as_str()))
            .cloned()
            .collect();
        let mut head: Vec<String> = lead.clone();
        head.push("Inter-Regular".to_owned());
        head.push("Inter-Regular-Cyrl".to_owned());
        // Split the bundled faces: Ubuntu first, then emoji-icon, then the
        // rest (NotoEmoji last, so its unrestorable bitmaps never shadow the
        // monochrome icons).
        let mut middle = Vec::new();
        let mut tail = Vec::new();
        for name in bundled {
            if name == "Ubuntu-Light" || name == "emoji-icon-font" {
                middle.push(name);
            } else {
                tail.push(name);
            }
        }
        middle.sort_by_key(|n| if n == "Ubuntu-Light" { 0 } else { 1 });
        head.extend(middle);
        // System script fallbacks (CJK, Arabic, …). The bytes are
        // memory-mapped files, so borrowing them costs nothing — copying them
        // would put tens of MiB in the working set for scripts most sessions
        // never draw.
        for fb in crate::system_fonts::fallbacks() {
            fonts.font_data.insert(
                fb.name.clone(),
                std::sync::Arc::new(egui::FontData {
                    font: std::borrow::Cow::Borrowed(fb.bytes),
                    index: fb.index,
                    tweak: Default::default(),
                }),
            );
            head.push(fb.name.clone());
        }
        head.extend(tail);
        *list = head.clone();

        // Weight families: the custom face still leads (it is what the user
        // chose, and one cut of it beats a synthetic step), then Inter's
        // 500/700 cuts, then the same fallback chain minus Inter's other cuts
        // — so a medium/bold label never silently drops back to regular.
        let fallbacks: Vec<String> = head
            .iter()
            .filter(|n| !OURS.contains(&n.as_str()))
            .cloned()
            .collect();
        for (family, latin, cyrl) in [
            (FAMILY_MEDIUM, "Inter-Medium", "Inter-Medium-Cyrl"),
            (FAMILY_BOLD, "Inter-Bold", "Inter-Bold-Cyrl"),
        ] {
            let mut chain = lead.clone();
            chain.push(latin.to_owned());
            chain.push(cyrl.to_owned());
            chain.extend(fallbacks.iter().cloned());
            fonts
                .families
                .insert(egui::FontFamily::Name(family.into()), chain);
        }
    }

    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;
    use read_fonts::TableProvider as _;

    fn has_glyph(bytes: &[u8], ch: char) -> bool {
        let font = read_fonts::FontRef::new(bytes).expect("parse font");
        let cmap = font.cmap().expect("cmap");
        cmap.map_codepoint(ch as u32).is_some()
    }

    /// Fonts parley can actually render (NotoEmoji's colour bitmaps excluded).
    fn renderable() -> Vec<(&'static str, &'static [u8])> {
        vec![
            ("Inter-latin-400", INTER_LATIN_400),
            ("Inter-cyrl-400", INTER_CYRILLIC_400),
            ("Inter-latin-500", INTER_LATIN_500),
            ("Inter-cyrl-500", INTER_CYRILLIC_500),
            ("Inter-latin-700", INTER_LATIN_700),
            ("Inter-cyrl-700", INTER_CYRILLIC_700),
            ("Ubuntu-Light", epaint_default_fonts::UBUNTU_LIGHT),
            ("emoji-icon", epaint_default_fonts::EMOJI_ICON),
            ("Hack", epaint_default_fonts::HACK_REGULAR),
        ]
    }

    /// Every character the UI puts on screen must be drawable by something
    /// in the chain, or it appears as tofu.
    #[test]
    fn report_coverage() {
        let all = renderable();
        let mut missing = Vec::new();
        let mut table = String::from("\nchar | covered by\n");
        for ch in TEXT_SAMPLE.chars().chain(ICON_CHARS.iter().copied()) {
            let hit: Vec<&str> = all
                .iter()
                .filter(|(_, b)| has_glyph(b, ch))
                .map(|(n, _)| *n)
                .collect();
            if hit.is_empty() {
                missing.push(ch);
            }
            table.push_str(&format!(
                "U+{:04X} {} | {}\n",
                ch as u32,
                ch,
                hit.join(", ")
            ));
        }
        println!("{table}");
        assert!(
            missing.is_empty(),
            "glyphs missing everywhere renderable: {:?}",
            missing
                .iter()
                .map(|c| format!("U+{:04X} {}", *c as u32, c))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn bundled_faces_parse() {
        for (name, bytes) in renderable() {
            assert!(!bytes.is_empty(), "{name} is empty");
            read_fonts::FontRef::new(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    /// Inter covers the Latin *and* the Cyrillic the app shows — the reason it
    /// is enough on its own, with no commercial face bundled.
    #[test]
    fn inter_covers_latin_and_cyrillic() {
        for ch in "ABCXYZabcxyz0189—–«»…".chars() {
            assert!(
                has_glyph(INTER_LATIN_400, ch),
                "Inter latin is missing U+{:04X} {ch}",
                ch as u32
            );
        }
        for ch in "ЖУКабвгдЁё".chars() {
            assert!(
                has_glyph(INTER_CYRILLIC_400, ch),
                "Inter cyrillic is missing U+{:04X} {ch}",
                ch as u32
            );
        }
        // …at every weight, or a heading would drop back to regular.
        for bytes in [INTER_LATIN_500, INTER_LATIN_700] {
            assert!(has_glyph(bytes, 'A'));
        }
        for bytes in [INTER_CYRILLIC_500, INTER_CYRILLIC_700] {
            assert!(has_glyph(bytes, 'Ж'));
        }
    }

    /// The weight families must exist and resolve to glyphs — a UI built on
    /// `RichText::strong()` renders everything at 400, which is what the
    /// medium/bold families exist to avoid.
    #[test]
    fn weight_families_are_registered_and_render() {
        let ctx = egui::Context::default();
        install(&ctx, None);
        for family in [
            egui::FontFamily::Proportional,
            egui::FontFamily::Name(FAMILY_MEDIUM.into()),
            egui::FontFamily::Name(FAMILY_BOLD.into()),
        ] {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                let galley = ui.painter().layout_no_wrap(
                    "Fastcloud Тест…".to_owned(),
                    egui::FontId::new(14.0, family.clone()),
                    egui::Color32::WHITE,
                );
                assert_eq!(
                    galley.rows[0].glyphs.len(),
                    15,
                    "family {family:?} lost a glyph"
                );
            });
            output.textures_delta.clear();
        }
    }

    /// A user-supplied face leads every family, and the app still lays out
    /// text that face cannot draw.
    #[test]
    fn a_custom_face_leads_and_still_falls_back() {
        let ctx = egui::Context::default();
        // Inter's Latin cut stands in for "some face the user chose": it has
        // no Cyrillic, so the fallback has to carry Тест.
        install(&ctx, Some(INTER_LATIN_400.to_vec()));
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let galley = ui.painter().layout_no_wrap(
                "Fastcloud Тест…".to_owned(),
                egui::FontId::new(14.0, egui::FontFamily::Name(FAMILY_BOLD.into())),
                egui::Color32::WHITE,
            );
            assert_eq!(galley.rows[0].glyphs.len(), 15);
        });
        output.textures_delta.clear();
    }

    /// A missing, empty or absurd font file must be ignored rather than fatal:
    /// the setting is a path a user typed.
    #[test]
    fn a_bad_custom_font_is_ignored() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(custom_face(&dir.path().join("nope.ttf")).is_none());

        let empty = dir.path().join("empty.ttf");
        std::fs::write(&empty, []).expect("write");
        assert!(custom_face(&empty).is_none());

        let huge = dir.path().join("huge.ttf");
        std::fs::write(&huge, vec![0u8; MAX_CUSTOM_FONT_BYTES as usize + 1]).expect("write");
        assert!(custom_face(&huge).is_none());

        // Junk of a plausible size is read, but it must not panic — and it
        // must not become a face egui would then fail on.
        let junk = dir.path().join("junk.ttf");
        std::fs::write(&junk, vec![0xABu8; 4096]).expect("write");
        assert!(custom_face(&junk).is_none());
    }

    /// `strip_blank_glyphs` must unmap a codepoint the face claims but draws
    /// as nothing — the reason an arbitrary face is safe to put first.
    ///
    /// Inter maps U+200B (zero-width space) to an empty outline, which is
    /// legitimate and must survive; nothing else in it is blank.
    #[test]
    fn stripping_keeps_a_sound_face_intact() {
        let before = blank_codepoints(INTER_LATIN_400);
        assert_eq!(
            before,
            ['\u{200b}'].into_iter().collect(),
            "Inter is expected to claim only the zero-width space"
        );
        let stripped = strip_blank_glyphs(INTER_LATIN_400);
        // Still parses…
        let font = read_fonts::FontRef::new(&stripped).expect("parse stripped font");
        let cmap = font.cmap().expect("cmap");
        // …keeps everything it really draws…
        for ch in "ABCxyz0189—–…«»".chars() {
            assert!(
                cmap.map_codepoint(ch).is_some(),
                "U+{:04X} {ch} was lost",
                ch as u32
            );
        }
        // …and the space characters stay mapped.
        for ch in [' ', '\u{00a0}', '\u{200b}'] {
            assert!(
                cmap.map_codepoint(ch).is_some(),
                "U+{:04X} was lost",
                ch as u32
            );
        }
    }

    /// Truncation appends `…`; if the leading face claimed it and drew
    /// nothing, every truncated label would end in a gap.
    #[test]
    fn an_ellipsis_survives_the_chain() {
        let ctx = egui::Context::default();
        install(&ctx, None);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let galley = ui.painter().layout_no_wrap(
                "Track…".to_owned(),
                egui::FontId::new(14.0, egui::FontFamily::Proportional),
                egui::Color32::WHITE,
            );
            assert_eq!(galley.rows[0].glyphs.len(), 6);
        });
        output.textures_delta.clear();
    }

    /// Codepoints the font maps to a glyph with an empty outline.
    fn blank_codepoints(bytes: &[u8]) -> std::collections::BTreeSet<char> {
        use read_fonts::{TableProvider as _, types::GlyphId};
        let font = read_fonts::FontRef::new(bytes).expect("parse font");
        let cmap = font.cmap().expect("cmap");
        let loca = font.loca(None).expect("loca");
        let glyf = font.glyf().expect("glyf");
        let mut out = std::collections::BTreeSet::new();
        for cp in 0x21u32..0x3000 {
            let Some(ch) = char::from_u32(cp) else {
                continue;
            };
            if ch.is_whitespace() || matches!(ch, '\u{00A0}' | '\u{00AD}') {
                continue;
            }
            let Some(gid) = cmap.map_codepoint(cp) else {
                continue;
            };
            if gid == GlyphId::NOTDEF {
                continue;
            }
            // `None` here means a zero-length entry: a blank glyph.
            if loca.get_glyf(gid, &glyf).ok().flatten().is_none() {
                out.insert(ch);
            }
        }
        out
    }
}
