//! Text preparation for the viewer (docs/PLAN.md §5): normalization,
//! paragraph split, and per-paragraph direction.

use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

/// Base direction of a paragraph, from its first strong character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Rtl,
    Ltr,
    /// No strong character (digits, punctuation, blank): let the renderer decide.
    Auto,
}

/// One rendered paragraph. An empty `text` marks a blank-line separator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Paragraph {
    pub text: String,
    pub dir: Direction,
}

/// Cleans captured text: CRLF/CR -> LF, Arabic presentation forms -> base
/// letters, zero-width junk removed (ZWNJ and ZWJ kept), trailing spaces
/// trimmed per line, leading/trailing blank lines removed.
pub fn normalize(text: &str) -> String {
    let unified = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut cleaned = String::with_capacity(unified.len());
    for c in unified.chars() {
        if is_zero_width_junk(c) {
            continue;
        }
        if is_arabic_presentation_form(c) {
            cleaned.extend(std::iter::once(c).nfkc());
        } else {
            cleaned.push(c);
        }
    }
    let lines: Vec<&str> = cleaned
        .split('\n')
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect();
    let first = lines.iter().position(|line| !line.trim().is_empty());
    let last = lines.iter().rposition(|line| !line.trim().is_empty());
    match (first, last) {
        (Some(first), Some(last)) => lines[first..=last].join("\n"),
        _ => String::new(),
    }
}

/// Splits normalized text into one paragraph per line. Runs of blank lines
/// collapse into a single empty separator paragraph.
pub fn paragraphs(text: &str) -> Vec<Paragraph> {
    let mut result: Vec<Paragraph> = Vec::new();
    for line in text.split('\n') {
        if line.trim().is_empty() {
            let after_separator = result.last().is_none_or(|p| p.text.is_empty());
            if !after_separator {
                result.push(Paragraph {
                    text: String::new(),
                    dir: Direction::Auto,
                });
            }
        } else {
            result.push(Paragraph {
                text: line.to_owned(),
                dir: detect_direction(line),
            });
        }
    }
    if result.last().is_some_and(|p| p.text.is_empty()) {
        result.pop();
    }
    result
}

/// Direction of the majority of words; a tie goes to RTL. First-strong
/// detection fails on Persian text that starts with a Latin term
/// ("fmt، clippy بدون ..."), which is common in technical writing.
pub fn detect_direction(text: &str) -> Direction {
    let (mut rtl, mut ltr) = (0usize, 0usize);
    for word in text.split_whitespace() {
        match majority_direction(word.chars()) {
            Direction::Rtl => rtl += 1,
            Direction::Ltr => ltr += 1,
            Direction::Auto => {}
        }
    }
    direction_from_counts(rtl, ltr)
}

/// Direction of the majority of letters in `chars`; a tie goes to RTL.
fn majority_direction(chars: impl Iterator<Item = char>) -> Direction {
    let (mut rtl, mut ltr) = (0usize, 0usize);
    for c in chars.filter(|c| c.is_alphabetic()) {
        if is_rtl_letter(c) {
            rtl += 1;
        } else {
            ltr += 1;
        }
    }
    direction_from_counts(rtl, ltr)
}

fn direction_from_counts(rtl: usize, ltr: usize) -> Direction {
    match (rtl, ltr) {
        (0, 0) => Direction::Auto,
        _ if rtl >= ltr => Direction::Rtl,
        _ => Direction::Ltr,
    }
}

/// Zero-width characters that carry no meaning in captured text. ZWNJ
/// (U+200C) is required in Persian and ZWJ (U+200D) in emoji sequences.
fn is_zero_width_junk(c: char) -> bool {
    matches!(c, '\u{200B}' | '\u{2060}' | '\u{FEFF}')
}

fn is_arabic_presentation_form(c: char) -> bool {
    matches!(c, '\u{FB50}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}')
}

/// Scripts written right-to-left: Hebrew, Arabic, Syriac, Thaana, NKo,
/// Samaritan, Mandaic, their presentation forms, and the SMP RTL blocks.
fn is_rtl_letter(c: char) -> bool {
    matches!(
        c,
        '\u{0590}'..='\u{08FF}'
            | '\u{FB1D}'..='\u{FDFF}'
            | '\u{FE70}'..='\u{FEFF}'
            | '\u{10800}'..='\u{10FFF}'
            | '\u{1E800}'..='\u{1EFFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- normalize -------------------------------------------------------

    #[test]
    fn normalize_converts_crlf_and_cr_to_lf() {
        assert_eq!(normalize("a\r\nb\rc"), "a\nb\nc");
    }

    #[test]
    fn normalize_preserves_zwnj() {
        assert_eq!(normalize("می\u{200C}خواهم"), "می\u{200C}خواهم");
    }

    #[test]
    fn normalize_strips_zero_width_space_word_joiner_and_bom() {
        assert_eq!(normalize("\u{FEFF}سلام\u{200B}دنیا\u{2060}!"), "سلامدنیا!");
    }

    #[test]
    fn normalize_keeps_zwj_for_emoji_sequences() {
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        assert_eq!(normalize(family), family);
    }

    #[test]
    fn normalize_trims_trailing_spaces_per_line() {
        assert_eq!(normalize("سلام  \t\nدنیا   "), "سلام\nدنیا");
    }

    #[test]
    fn normalize_keeps_leading_indentation() {
        assert_eq!(normalize("  indented"), "  indented");
    }

    #[test]
    fn normalize_removes_leading_and_trailing_blank_lines() {
        assert_eq!(normalize("\n\n  \nمتن\n\n"), "متن");
    }

    #[test]
    fn normalize_maps_arabic_presentation_forms_to_base_letters() {
        // "کارشناسی" as found in a PDF text layer (presentation forms).
        let pdf = "\u{FB8E}\u{FE8E}\u{FEAE}\u{FEB7}\u{FEE8}\u{FE8E}\u{FEB3}\u{FBFF}";
        assert_eq!(normalize(pdf), "کارشناسی");
    }

    #[test]
    fn normalize_expands_lam_alef_ligature() {
        assert_eq!(normalize("\u{FEFB}"), "لا");
    }

    #[test]
    fn normalize_leaves_other_compatibility_characters_alone() {
        // Only presentation forms are folded; full-width and superscripts stay.
        assert_eq!(normalize("Ａ x²"), "Ａ x²");
    }

    #[test]
    fn normalize_keeps_persian_digits() {
        assert_eq!(normalize("۱۲۳ ٤٥٦"), "۱۲۳ ٤٥٦");
    }

    // --- paragraphs -----------------------------------------------------

    #[test]
    fn paragraphs_split_on_newlines() {
        let texts: Vec<_> = paragraphs("یک\ndo").into_iter().map(|p| p.text).collect();
        assert_eq!(texts, ["یک", "do"]);
    }

    #[test]
    fn paragraphs_collapse_blank_line_runs_into_one_separator() {
        let texts: Vec<_> = paragraphs("یک\n\n\n\nدو")
            .into_iter()
            .map(|p| p.text)
            .collect();
        assert_eq!(texts, ["یک", "", "دو"]);
    }

    #[test]
    fn paragraphs_of_empty_text_is_empty() {
        assert!(paragraphs("").is_empty());
    }

    #[test]
    fn paragraphs_carry_their_own_direction() {
        let dirs: Vec<_> = paragraphs("سلام\nhello\n123")
            .into_iter()
            .map(|p| p.dir)
            .collect();
        assert_eq!(dirs, [Direction::Rtl, Direction::Ltr, Direction::Auto]);
    }

    #[test]
    fn separator_paragraph_is_auto() {
        assert_eq!(paragraphs("a\n\nb")[1].dir, Direction::Auto);
    }

    // --- detect_direction -----------------------------------------------

    #[test]
    fn direction_of_persian_is_rtl() {
        assert_eq!(detect_direction("سلام دنیا"), Direction::Rtl);
    }

    #[test]
    fn direction_of_hebrew_is_rtl() {
        assert_eq!(detect_direction("שלום"), Direction::Rtl);
    }

    #[test]
    fn direction_of_english_is_ltr() {
        assert_eq!(detect_direction("hello"), Direction::Ltr);
    }

    #[test]
    fn direction_follows_the_majority_of_words_not_the_first_letter() {
        let persian_with_leading_terms =
            "fmt، clippy بدون هیچ هشدار، ۵۲ تست از ۵۲، build و graft همه بدون مشکل بودن.";
        assert_eq!(detect_direction(persian_with_leading_terms), Direction::Rtl);
        assert_eq!(detect_direction("2. قیدهای یکتا (Unique)"), Direction::Rtl);
        assert_eq!(
            detect_direction("The word سلام means hello in Persian"),
            Direction::Ltr
        );
    }

    #[test]
    fn direction_counts_a_url_as_one_word() {
        let text = "برای دانلود به https://github.com/tauri-apps/tauri برو";
        assert_eq!(detect_direction(text), Direction::Rtl);
    }

    #[test]
    fn direction_tie_prefers_rtl() {
        assert_eq!(detect_direction("Unique قیدهای"), Direction::Rtl);
        assert_eq!(detect_direction("قیدهای Unique"), Direction::Rtl);
    }

    #[test]
    fn direction_skips_leading_digits_punctuation_and_zwnj() {
        assert_eq!(detect_direction("۱۲۳ - \u{200C}«متن»"), Direction::Rtl);
    }

    #[test]
    fn direction_without_letters_is_auto() {
        assert_eq!(detect_direction("123 - ۴۵"), Direction::Auto);
        assert_eq!(detect_direction(""), Direction::Auto);
    }

    #[test]
    fn direction_of_presentation_forms_is_rtl() {
        assert_eq!(detect_direction("\u{FEFB}"), Direction::Rtl);
    }
}
