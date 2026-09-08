//! System fallback fonts for scripts not covered by the interface font.
//!
//! Inter covers Latin and Cyrillic. Bundling fonts for every other script
//! would greatly increase the binary size, so the app scans installed fonts
//! and registers one suitable fallback per script (CJK, Arabic, Hebrew,
//! Thai, …). Adapted from Fastpotify's `system_fonts.rs` (MIT).
//!
//! ## Why the faces are mapped, not read
//!
//! On a stock Windows install the sixteen faces this picks come to ~56 MiB,
//! and the pan-CJK ones are 13–20 MiB each. Reading them into `Vec<u8>` put
//! all of that in the process's private working set — twice over, since
//! [`crate::fonts::install`] then cloned each buffer for egui — for scripts a
//! given user may never see a single glyph of.
//!
//! Each face is memory-mapped instead. The mapping is file-backed, so a page
//! only becomes resident when a glyph on it is actually rasterized, and the
//! OS can drop those pages again under pressure. A Latin-only session pays
//! for the headers it parsed and nothing else.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use skrifa::MetadataProvider as _;
use skrifa::raw::TableProvider as _;

/// A registered fallback face: its egui name, the bytes it lives in, and the
/// face index inside a collection (`.ttc`).
pub struct Fallback {
    pub name: String,
    /// The mapped file. `'static` because egui keeps font data for the
    /// lifetime of the context, and the mapping is created once per process.
    pub bytes: &'static [u8],
    pub index: u32,
}

/// Scripts Inter does not cover, with a probe character and family-name hint.
///
/// One face is selected per entry in this order. A face covering multiple
/// scripts is registered once. Glyphs are rasterized only when used.
const FALLBACK_SCRIPTS: &[(&str, char, &str)] = &[
    ("han", '\u{4e2d}', "cjk"),
    ("kana", '\u{3042}', "cjk"),
    ("hangul", '\u{d55c}', "cjk"),
    ("arabic", '\u{0627}', "arabic"),
    ("hebrew", '\u{05d0}', "hebrew"),
    ("thai", '\u{0e01}', "thai"),
    ("lao", '\u{0e81}', "lao"),
    ("khmer", '\u{1780}', "khmer"),
    ("myanmar", '\u{1000}', "myanmar"),
    ("devanagari", '\u{0915}', "devanagari"),
    ("bengali", '\u{0995}', "bengali"),
    ("gurmukhi", '\u{0a15}', "gurmukhi"),
    ("gujarati", '\u{0a95}', "gujarati"),
    ("tamil", '\u{0ba4}', "tamil"),
    ("telugu", '\u{0c15}', "telugu"),
    ("kannada", '\u{0c95}', "kannada"),
    ("malayalam", '\u{0d15}', "malayalam"),
    ("sinhala", '\u{0d9a}', "sinhala"),
    ("armenian", '\u{0531}', "armenian"),
    ("georgian", '\u{10d0}', "georgian"),
    ("ethiopic", '\u{1200}', "ethiopic"),
    ("cherokee", '\u{13a0}', "cherokee"),
    ("symbols", '\u{2605}', "symbol"),
];

/// The regional cut of a pan-CJK font a locale should be shown, longest
/// prefix first.
const HAN_REGIONS: &[(&str, &str)] = &[
    ("zh_tw", "tc"),
    ("zh_hant", "tc"),
    ("zh_hk", "hk"),
    ("zh_mo", "hk"),
    ("zh", "sc"),
    ("ja", "jp"),
    ("ko", "kr"),
];

/// How deep to walk each font directory. Distributions nest a level or two;
/// nothing legitimate goes deeper, and the bound also ends any symlink loop.
const FONT_SCAN_DEPTH: usize = 4;

/// A collection says how many faces it holds, and a corrupt or hostile file
/// can say billions. No real one holds more than a few dozen.
const MAX_FACES: u32 = 64;

/// One system fallback face per unsupported script.
///
/// The search happens once per process: font installation runs for every
/// window the app creates, and this reads every font on the machine.
pub fn fallbacks() -> &'static [Fallback] {
    static FONTS: OnceLock<Vec<Fallback>> = OnceLock::new();
    FONTS.get_or_init(load)
}

/// A face that covers a script, and how well it suits the interface.
struct Candidate {
    /// Drawn for another Han region than the locale's. Serves only when it
    /// is all there is.
    foreign: bool,
    score: u32,
    path: PathBuf,
    index: u32,
}

/// Finds the best face for each of [`FALLBACK_SCRIPTS`] and reads the files
/// they live in.
fn load() -> Vec<Fallback> {
    let han = han_region(&locale());
    let started = std::time::Instant::now();
    let mut best: BTreeMap<&str, Candidate> = BTreeMap::new();
    for dir in font_dirs() {
        probe_dir(&dir, 0, han, &mut best);
    }
    log::debug!(
        "probed the system fonts in {:.1} ms, {} of {} scripts covered",
        started.elapsed().as_secs_f32() * 1e3,
        best.len(),
        FALLBACK_SCRIPTS.len()
    );

    // A face that covers several scripts is mapped and registered once, under
    // the first script that chose it.
    let mut fonts: Vec<Fallback> = Vec::new();
    let mut taken: Vec<(PathBuf, u32)> = Vec::new();
    for (script, _, _) in FALLBACK_SCRIPTS {
        let Some(candidate) = best.get(script) else {
            log::debug!("no fallback face covers {script}");
            continue;
        };
        if taken.contains(&(candidate.path.clone(), candidate.index)) {
            continue;
        }
        let Some(bytes) = map_face(&candidate.path) else {
            continue;
        };
        log::debug!(
            "{script} fallback: {} (face {})",
            candidate.path.display(),
            candidate.index
        );
        taken.push((candidate.path.clone(), candidate.index));
        fonts.push(Fallback {
            name: format!("fallback-{script}"),
            bytes,
            index: candidate.index,
        });
    }
    fonts
}

/// Map a font file for the life of the process.
///
/// The mapping is leaked on purpose: egui holds font data as long as the
/// context lives, and there is one context per process. Leaking a handle for
/// each of at most [`FALLBACK_SCRIPTS`]`.len()` faces is bounded, and it lets
/// the pages stay lazily resident instead of being copied onto the heap.
fn map_face(path: &Path) -> Option<&'static [u8]> {
    let file = std::fs::File::open(path)
        .inspect_err(|error| log::warn!("cannot open {}: {error}", path.display()))
        .ok()?;
    // Safety: the mapping is read-only and never unmapped, so no reference to
    // it can outlive it. A font file rewritten in place underneath would
    // fault — the same bet every font enumerator on the platform makes.
    let map = unsafe { memmap2::Mmap::map(&file) }
        .inspect_err(|error| log::warn!("cannot map {}: {error}", path.display()))
        .ok()?;
    Some(&*Box::leak(Box::new(map)))
}

/// Probes every font file below `dir`, keeping the best face per script.
fn probe_dir(dir: &Path, depth: usize, han: &str, best: &mut BTreeMap<&str, Candidate>) {
    if depth >= FONT_SCAN_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() || (kind.is_symlink() && path.is_dir()) {
            probe_dir(&path, depth + 1, han, best);
        } else if is_font_file(&path) {
            probe_file(&path, han, best);
        }
    }
}

/// Whether a path names a font this can open.
fn is_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "ttf" | "otf" | "ttc" | "otc"
            )
        })
}

/// Offers every face in one font file to every script that still wants one.
fn probe_file(path: &Path, han: &str, best: &mut BTreeMap<&str, Candidate>) {
    let Ok(file) = std::fs::File::open(path) else {
        return;
    };
    // Mapping the file rather than reading it keeps this to the few pages
    // holding each font's header, names, and character map.
    //
    // Safety: the mapping is read-only and lives inside this call. A font
    // file rewritten underneath it during that window would fault, which is
    // the same bet every font enumerator on the platform makes.
    let Ok(map) = (unsafe { memmap2::Mmap::map(&file) }) else {
        return;
    };
    let faces: Vec<(u32, skrifa::FontRef)> = match skrifa::raw::FileRef::new(&map) {
        Ok(skrifa::raw::FileRef::Font(font)) => vec![(0, font)],
        Ok(skrifa::raw::FileRef::Collection(collection)) => (0..collection.len().min(MAX_FACES))
            .filter_map(|index| collection.get(index).ok().map(|font| (index, font)))
            .collect(),
        Err(_) => return,
    };
    for (index, font) in faces {
        let attributes = font.attributes();
        if attributes.style != skrifa::attribute::Style::Normal {
            continue;
        }
        let family = font
            .localized_strings(skrifa::string::StringId::FAMILY_NAME)
            .english_or_first()
            .map(|name| name.to_string())
            .unwrap_or_default()
            .to_lowercase();
        let charmap = font.charmap();
        let outlines = font.outline_glyphs();
        // A face must draw the character, not merely map it: a bitmap or
        // colour-only font passes the charmap and renders nothing.
        let draws = |character: char| {
            charmap
                .map(character)
                .is_some_and(|glyph| outlines.get(glyph).is_some())
        };
        for (script, probe, hint) in FALLBACK_SCRIPTS {
            if !draws(*probe) {
                continue;
            }
            let score = face_score(&family, attributes.weight.value(), han, hint);
            let foreign = *script == "han" && {
                let code_pages = font.os2().ok().and_then(|os2| os2.ul_code_page_range_1());
                !covers_han_region(code_pages, han)
            };
            // Ties break on the path so two machines carrying the same fonts
            // resolve the same face, whatever order their directories list.
            if best.get(script).is_none_or(|held| {
                (foreign, score, path) < (held.foreign, held.score, held.path.as_path())
            }) {
                best.insert(
                    script,
                    Candidate {
                        foreign,
                        score,
                        path: path.to_path_buf(),
                        index,
                    },
                );
            }
        }
    }
}

/// Ranks a face as interface text for the script named by `hint`, lowest
/// first. A plain sans near regular weight sits beside Inter; display,
/// serif, and mono cuts lose.
fn face_score(family: &str, weight: f32, han: &str, hint: &str) -> u32 {
    let mut score = ((weight - 400.0).abs() / 25.0) as u32;
    // A face that names the script was drawn for it.
    if !family.contains(hint) {
        score += 25;
    }
    if !family.contains("sans") && hint != "emoji" && hint != "symbol" {
        score += 50;
    }
    for (fragment, penalty) in [
        ("serif", 200),
        ("mono", 120),
        ("kufi", 80),
        ("naskh", 80),
        ("looped", 80),
        ("display", 80),
        ("condensed", 60),
        ("caption", 40),
    ] {
        if family.contains(fragment) {
            score += penalty;
        }
    }
    // Han characters are unified in Unicode; prefer the cut this locale reads.
    if let Some(region) = family
        .rsplit(' ')
        .next()
        .filter(|region| han_code_page(region).is_some())
        && region != han
    {
        score += 40;
    }
    score
}

/// The OS/2 `ulCodePageRange1` bit a face sets to declare it covers a
/// region's legacy character set: Shift JIS, GB 2312, Wansung, or Big5.
fn han_code_page(region: &str) -> Option<u32> {
    match region {
        "jp" => Some(17),
        "sc" => Some(18),
        "kr" => Some(19),
        "tc" | "hk" => Some(20),
        _ => None,
    }
}

/// Whether a face's declared code pages include the region's character set.
/// A face too old to declare any is taken at its word: none.
fn covers_han_region(code_pages: Option<u32>, han: &str) -> bool {
    han_code_page(han)
        .zip(code_pages)
        .is_some_and(|(bit, pages)| pages & (1 << bit) != 0)
}

/// The user's locale, lowercased, or an empty string when none is set. A
/// variable that is set but empty does not count, as POSIX has it.
fn locale() -> String {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .find_map(|key| std::env::var(key).ok().filter(|value| !value.is_empty()))
        .unwrap_or_default()
        .to_lowercase()
}

/// The pan-CJK cut a locale reads, defaulting to Simplified Chinese.
fn han_region(locale: &str) -> &'static str {
    HAN_REGIONS
        .iter()
        .find(|(prefix, _)| locale.starts_with(prefix))
        .map_or("sc", |(_, region)| *region)
}

/// Where the platform keeps installed fonts.
fn font_dirs() -> Vec<PathBuf> {
    let user = directories::UserDirs::new();
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut add = |dir: PathBuf| {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    };
    if cfg!(target_os = "macos") {
        add(PathBuf::from("/System/Library/Fonts"));
        add(PathBuf::from("/Library/Fonts"));
    } else if cfg!(target_os = "windows") {
        add(std::env::var_os("SystemRoot")
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
            .join("Fonts"));
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            add(PathBuf::from(local).join(r"Microsoft\Windows\Fonts"));
        }
    } else {
        let data_dirs = std::env::var("XDG_DATA_DIRS")
            .ok()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());
        for dir in data_dirs.split(':').filter(|dir| !dir.is_empty()) {
            add(PathBuf::from(dir).join("fonts"));
        }
        add(PathBuf::from("/run/host/fonts"));
        if let Some(user) = &user {
            add(user.home_dir().join(".fonts"));
        }
    }
    if let Some(font_dir) = user.as_ref().and_then(|user| user.font_dir()) {
        add(font_dir.to_path_buf());
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locales_choose_a_pan_cjk_cut() {
        assert_eq!(han_region("zh_cn.utf-8"), "sc");
        assert_eq!(han_region("zh_tw.utf-8"), "tc");
        assert_eq!(han_region("zh_hk.utf-8"), "hk");
        assert_eq!(han_region("ja_jp.utf-8"), "jp");
        assert_eq!(han_region("ko_kr.utf-8"), "kr");
        assert_eq!(han_region("en_us.utf-8"), "sc", "the default");
        assert_eq!(han_region(""), "sc", "no locale set");
    }

    #[test]
    fn interface_faces_outrank_display_ones() {
        let sans = face_score("noto sans arabic", 400.0, "sc", "arabic");
        assert!(sans < face_score("noto naskh arabic", 400.0, "sc", "arabic"));
        assert!(sans < face_score("noto kufi arabic", 400.0, "sc", "arabic"));
        assert!(sans < face_score("noto serif arabic", 400.0, "sc", "arabic"));
        assert!(sans < face_score("noto sans arabic", 700.0, "sc", "arabic"));
    }

    #[test]
    fn a_face_drawn_for_the_script_wins() {
        assert!(
            face_score("noto sans hebrew", 400.0, "sc", "hebrew")
                < face_score("liberation sans", 400.0, "sc", "hebrew")
        );
    }

    #[test]
    fn only_font_files_are_probed() {
        assert!(is_font_file(Path::new("/x/NotoSans.ttf")));
        assert!(is_font_file(Path::new("/x/NotoSansCJK.TTC")));
        assert!(!is_font_file(Path::new("/x/fonts.dir")));
        assert!(!is_font_file(Path::new("/x/README")));
    }

    /// Faces are mapped, not read: a stock Windows install picks ~56 MiB of
    /// fallbacks (three pan-CJK faces alone are ~47 MiB), and copying that
    /// onto the heap is what put a quarter of a gigabyte in the working set.
    #[test]
    fn faces_are_mapped_rather_than_copied() {
        let all = fallbacks();
        let total: usize = all.iter().map(|f| f.bytes.len()).sum();
        // Nothing to check on a machine with no extra scripts installed.
        if all.is_empty() {
            return;
        }
        println!(
            "{} fallback faces, {:.1} MiB mapped",
            all.len(),
            total as f64 / 1_048_576.0
        );
        for face in all {
            assert!(!face.bytes.is_empty(), "{} mapped nothing", face.name);
        }
        // One face per script at most, and each file registered once.
        assert!(all.len() <= FALLBACK_SCRIPTS.len());
        let mut names: Vec<&str> = all.iter().map(|f| f.name.as_str()).collect();
        names.sort_unstable();
        let unique = names.len();
        names.dedup();
        assert_eq!(names.len(), unique, "a face was registered twice");
    }

    #[test]
    fn probing_the_system_never_panics() {
        // Whatever fonts this machine has, including none.
        let _ = fallbacks();
    }
}
