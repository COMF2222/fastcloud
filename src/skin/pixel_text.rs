//! The 5×6 pixel font the mini player's marquee uses.
//!
//! Winamp kept its title font in `text.bmp`, thirty-one cells a row. Rather
//! than crop art the stock skin does not have, the glyphs are drawn from
//! bitmasks here: each character is six rows of five bits, so a title lands
//! on exact pixels at any whole scale — which is the whole point of the
//! classic look. A skinned title still reads from the skin's own sheet when
//! it has one; this is the fallback and the stock face.
//!
//! Latin and Cyrillic metadata use the same fixed 5×6 face. Scripts without
//! a glyph fall back to a block, so their spacing and marquee remain stable.

/// Glyph size in skin pixels.
pub const CHAR_W: u32 = 5;
pub const CHAR_H: u32 = 6;

/// Rows of five bits, most significant bit leftmost.
type Glyph = [u8; 6];

const BLANK: Glyph = [0, 0, 0, 0, 0, 0];
/// Anything the face cannot draw: a hollow block, like Winamp's fallback.
const UNKNOWN: Glyph = [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b11111];

/// The glyph for one character, uppercased (the face has one case).
pub fn glyph(ch: char) -> Glyph {
    let ch = ch.to_uppercase().next().unwrap_or(ch);
    match ch {
        ' ' => BLANK,
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01111, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
        'H' => [0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b11100, 0b10010, 0b10001, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10101, 0b10011, 0b01111],
        'R' => [0b11110, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b01010, 0b00100, 0b00100, 0b01010, 0b10001],
        'Y' => [0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        // Cyrillic. Letters that share the Latin pixel shape reuse it; the
        // rest have dedicated 5x6 cells so Russian metadata stays readable.
        'А' => glyph('A'),
        'В' => glyph('B'),
        'Е' | 'Ё' => glyph('E'),
        'К' => glyph('K'),
        'М' => glyph('M'),
        'Н' => glyph('H'),
        'О' => glyph('O'),
        'Р' => glyph('P'),
        'С' => glyph('C'),
        'Т' => glyph('T'),
        'У' => glyph('Y'),
        'Х' => glyph('X'),
        'Б' => [0b11111, 0b10000, 0b11110, 0b10001, 0b10001, 0b11110],
        'Г' => [0b11111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000],
        'Д' => [0b00110, 0b01010, 0b10010, 0b10010, 0b11111, 0b10001],
        'Ж' => [0b10101, 0b10101, 0b01110, 0b10101, 0b10101, 0b10101],
        'З' => [0b01110, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110],
        'И' => [0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b10001],
        'Й' => [0b01010, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001],
        'Л' => [0b00111, 0b01001, 0b10001, 0b10001, 0b10001, 0b10001],
        'П' => [0b11111, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001],
        'Ф' => [0b00100, 0b11111, 0b10101, 0b11111, 0b00100, 0b00100],
        'Ц' => [0b10001, 0b10001, 0b10001, 0b10001, 0b11111, 0b00001],
        'Ч' => [0b10001, 0b10001, 0b01111, 0b00001, 0b00001, 0b00001],
        'Ш' => [0b10101, 0b10101, 0b10101, 0b10101, 0b10101, 0b11111],
        'Щ' => [0b10101, 0b10101, 0b10101, 0b10101, 0b11111, 0b00001],
        'Ъ' => [0b11000, 0b01000, 0b01110, 0b01001, 0b01001, 0b01110],
        'Ы' => [0b10001, 0b10001, 0b11101, 0b10101, 0b10101, 0b11101],
        'Ь' => [0b10000, 0b10000, 0b11110, 0b10001, 0b10001, 0b11110],
        'Э' => [0b11110, 0b00001, 0b01111, 0b00001, 0b00001, 0b11110],
        'Ю' => [0b10110, 0b11001, 0b11001, 0b11001, 0b11001, 0b10110],
        'Я' => [0b01111, 0b10001, 0b01111, 0b00101, 0b01001, 0b10001],
        '0' => [0b01110, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11111, 0b00010, 0b00110, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00010, 0b01100],
        '.' => [0, 0, 0, 0, 0, 0b00100],
        ',' => [0, 0, 0, 0, 0b00100, 0b01000],
        ':' => [0, 0b00100, 0, 0, 0b00100, 0],
        ';' => [0, 0b00100, 0, 0, 0b00100, 0b01000],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
        '?' => [0b01110, 0b10001, 0b00010, 0b00100, 0, 0b00100],
        '\'' => [0b00100, 0b00100, 0, 0, 0, 0],
        '"' => [0b01010, 0b01010, 0, 0, 0, 0],
        '-' => [0, 0, 0, 0b11111, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0b11111],
        '+' => [0, 0b00100, 0b00100, 0b11111, 0b00100, 0b00100],
        '=' => [0, 0, 0b11111, 0, 0b11111, 0],
        '/' => [0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0],
        '\\' => [0b10000, 0b01000, 0b00100, 0b00010, 0b00001, 0],
        '(' => [0b00010, 0b00100, 0b00100, 0b00100, 0b00100, 0b00010],
        ')' => [0b01000, 0b00100, 0b00100, 0b00100, 0b00100, 0b01000],
        '[' => [0b00110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00110],
        ']' => [0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01100],
        '&' => [0b01100, 0b10010, 0b01100, 0b10101, 0b10010, 0b01101],
        '*' => [0, 0b01010, 0b00100, 0b01010, 0, 0],
        '#' => [0b01010, 0b11111, 0b01010, 0b11111, 0b01010, 0],
        '%' => [0b10001, 0b00010, 0b00100, 0b01000, 0b10001, 0],
        '@' => [0b01110, 0b10001, 0b10111, 0b10111, 0b10000, 0b01110],
        '<' => [0b00010, 0b00100, 0b01000, 0b01000, 0b00100, 0b00010],
        '>' => [0b01000, 0b00100, 0b00010, 0b00010, 0b00100, 0b01000],
        '‘' | '’' => glyph('\''),
        '“' | '”' => glyph('"'),
        '–' | '—' | '−' => glyph('-'),
        '…' => glyph('.'),
        '·' | '•' => glyph(':'),
        // Anything else gets the block, so the marquee still shows something
        // moving without switching font metrics mid-title.
        _ => UNKNOWN,
    }
}

/// A lit pixel at `(x, y)` inside `ch`'s cell?
pub fn lit(ch: char, x: u32, y: u32) -> bool {
    if x >= CHAR_W || y >= CHAR_H {
        return false;
    }
    let rows = glyph(ch);
    let bit = CHAR_W - 1 - x;
    rows[y as usize] & (1 << bit) != 0
}

/// The marquee's text for one frame.
///
/// Winamp scrolled a title that did not fit, one character a tick, with a
/// separator so the end and the start do not run together. A title that fits
/// is returned as is — it must not jitter.
pub fn marquee(text: &str, width_chars: usize, tick: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width_chars {
        return text.to_owned();
    }
    // ` *** ` is Winamp's own separator.
    let mut looped: Vec<char> = chars.clone();
    looped.extend(" *** ".chars());
    let period = looped.len();
    let start = tick % period;
    (0..width_chars)
        .map(|i| looped[(start + i) % period])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_glyphs_have_pixels_and_space_has_none() {
        assert!(lit('A', 1, 0), "A's top bar starts at x=1");
        assert!(!lit('A', 0, 0));
        assert!((0..CHAR_H).all(|y| (0..CHAR_W).all(|x| !lit(' ', x, y))));
        // Out of the cell is never lit.
        assert!(!lit('A', CHAR_W, 0));
        assert!(!lit('A', 0, CHAR_H));
    }

    #[test]
    fn lowercase_reads_as_uppercase() {
        assert_eq!(glyph('a'), glyph('A'));
        assert_eq!(glyph('z'), glyph('Z'));
        assert_eq!(glyph('я'), glyph('Я'));
        assert_ne!(glyph('Ж'), UNKNOWN);
    }

    /// A title in a script the face cannot draw still scrolls, as a block.
    #[test]
    fn unknown_characters_get_the_block() {
        assert_eq!(glyph('音'), UNKNOWN);
        assert!(lit('音', 0, 0), "the block's corner is lit");
    }

    #[test]
    fn a_short_title_does_not_scroll() {
        assert_eq!(marquee("SHORT", 30, 0), "SHORT");
        assert_eq!(marquee("SHORT", 30, 99), "SHORT");
        assert_eq!(marquee("", 30, 5), "");
    }

    /// A long title advances one character a tick and comes back round.
    #[test]
    fn a_long_title_scrolls_and_wraps() {
        let title = "A VERY LONG TRACK TITLE THAT WILL NOT FIT THE DISPLAY";
        let first = marquee(title, 10, 0);
        assert_eq!(first.chars().count(), 10);
        assert!(title.starts_with(&first));
        let second = marquee(title, 10, 1);
        assert_ne!(first, second);
        assert_eq!(second.chars().count(), 10);
        // The separator appears once the end comes round.
        let period = title.chars().count() + " *** ".chars().count();
        assert_eq!(marquee(title, 10, period), first, "one full loop");
        let near_end = marquee(title, 10, title.chars().count());
        assert!(near_end.contains('*'));
    }
}
