//! Right-to-left text support for egui's left-to-right layout.
//!
//! epaint shapes letters within Arabic and Hebrew words but does not reorder
//! the words, so a mixed-direction track title would read backwards.
//! [`display_text`] applies visual word order while preserving logical letter
//! order for shaping. Ported from Fastpotify's `bidi.rs` (MIT).

use std::borrow::Cow;

use unicode_bidi::{BidiClass, bidi_class};

/// Whether the text reads right to left: decided by its first strong
/// character, the way the bidi algorithm decides a paragraph's direction.
/// ASCII never does, and most text is ASCII, so that check comes first and
/// costs nothing per frame.
pub fn is_rtl(text: &str) -> bool {
    if text.is_ascii() {
        return false;
    }
    for character in text.chars() {
        match bidi_class(character) {
            BidiClass::L => return false,
            BidiClass::R | BidiClass::AL => return true,
            _ => {}
        }
    }
    false
}

/// `text` as the engine should receive it: every right-to-left line with
/// its words reordered. Text that needs no change comes back borrowed.
pub fn display_text(text: &str) -> Cow<'_, str> {
    if !text.contains('\n') {
        return display_line(text);
    }
    if !text.split('\n').any(is_rtl) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        text.split('\n')
            .map(display_line)
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// Owned display string for widgets (`RichText::new` etc.).
/// Costs one small allocation; labels allocate anyway.
pub fn owned(text: &str) -> String {
    display_text(text).into_owned()
}

/// One line. Punctuation, digits, and an ellipsis marking a cut keep the
/// places the bidi algorithm gives them; only the letters of each
/// right-to-left run go back to logical order, for the shaper to mirror.
fn display_line(line: &str) -> Cow<'_, str> {
    if !is_rtl(line) || line.chars().all(is_rtl_letter) {
        return Cow::Borrowed(line);
    }
    let line = line.trim_end_matches('\r');
    let info = unicode_bidi::BidiInfo::new(line, None);
    let Some(paragraph) = info.paragraphs.first() else {
        return Cow::Borrowed(line);
    };
    let visual = info.reorder_line(paragraph, paragraph.range.clone());
    let mut out = String::with_capacity(line.len());
    let mut letters: Vec<char> = Vec::new();
    for (index, word) in visual.split_whitespace().enumerate() {
        if index > 0 {
            out.push(' ');
        }
        for character in word.chars() {
            if is_rtl_letter(character) {
                letters.push(character);
            } else {
                out.extend(letters.drain(..).rev());
                out.push(character);
            }
        }
        out.extend(letters.drain(..).rev());
    }
    Cow::Owned(out)
}

/// A letter of a right-to-left script, or a mark that rides on one.
fn is_rtl_letter(character: char) -> bool {
    matches!(
        bidi_class(character),
        BidiClass::R | BidiClass::AL | BidiClass::NSM
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_rtl() {
        assert!(is_rtl("غيوم في السماء"));
        assert!(is_rtl("מה נשמע"));
        assert!(!is_rtl("Hello world"));
        assert!(!is_rtl("Hello غيوم")); // base LTR
        assert!(is_rtl("غيوم Hello")); // base RTL
        assert!(!is_rtl(""));
        assert!(!is_rtl("123"));
    }

    #[test]
    fn reorders_words_of_rtl_lines() {
        assert_eq!(display_text("غيوم في السماء"), "السماء في غيوم");
        assert_eq!(display_text("مرحبا بالعالم"), "بالعالم مرحبا");
        assert_eq!(display_text("غيوم"), "غيوم");
        assert!(matches!(display_text("غيوم"), Cow::Borrowed(_)));
        assert_eq!(display_text("Hello غيوم"), "Hello غيوم");
        assert!(matches!(display_text("Hello world"), Cow::Borrowed(_)));
        assert_eq!(display_text("غيوم Hello"), "Hello غيوم");
    }

    #[test]
    fn ellipsis_moves_to_the_reading_end() {
        assert_eq!(display_text("غيوم في\u{2026}"), "\u{2026}في غيوم");
        assert_eq!(display_text("Hello\u{2026}"), "Hello\u{2026}");
    }

    #[test]
    fn punctuation_and_digits_stay_after_their_word() {
        assert_eq!(display_text("الحلقة الأولى: كيف"), "كيف :الأولى الحلقة");
        assert_eq!(display_text("الكلمة1 الكلمة2"), "2الكلمة 1الكلمة");
    }
}
