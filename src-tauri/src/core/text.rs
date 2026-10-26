//! Text preparation for the viewer (docs/design.md, Viewer): normalization,
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
    /// [`detached_numbers`] of `text`; an RTL paragraph isolates them.
    pub numbers: Vec<(usize, usize)>,
    /// [`latin_separators`] of `text`; an RTL paragraph adds a mark after them.
    pub separators: Vec<(usize, usize)>,
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
                    numbers: Vec::new(),
                    separators: Vec::new(),
                });
            }
        } else {
            result.push(Paragraph {
                text: line.to_owned(),
                dir: detect_direction(line),
                numbers: detached_numbers(line),
                separators: latin_separators(line),
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

/// Number runs that the bidi algorithm would glue to the Latin word before
/// them although they belong to the text after them: "CPU، ۲ گیگابایت" reads
/// "۲ ،CPU گیگابایت" in an RTL paragraph, because a European number takes
/// the direction of the previous strong character (UAX #9, rule W7). A run
/// is detached when punctuation separates it from that Latin word and no
/// Latin text follows it; "Windows 11" (only a space) and "CPU, 2 GB" (Latin
/// after) stay as they are. Ranges are UTF-16 offsets, as the viewer's
/// JavaScript indexes strings.
pub fn detached_numbers(text: &str) -> Vec<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let mut offsets = Vec::with_capacity(chars.len() + 1);
    let mut offset = 0;
    for c in &chars {
        offsets.push(offset);
        offset += c.len_utf16();
    }
    offsets.push(offset);

    let mut result = Vec::new();
    let mut previous_strong = None;
    let mut punctuation_since_strong = false;
    let mut i = 0;
    while i < chars.len() {
        if is_european_digit(chars[i]) {
            let end = number_run_end(&chars, i);
            let after_latin = previous_strong == Some(Direction::Ltr) && punctuation_since_strong;
            let next_strong = chars[end..].iter().find_map(|&c| strong_direction(c));
            if after_latin && next_strong != Some(Direction::Ltr) {
                result.push((offsets[i], offsets[end]));
            }
            i = end;
            continue;
        }
        if let Some(direction) = strong_direction(chars[i]) {
            previous_strong = Some(direction);
            punctuation_since_strong = false;
        } else if !chars[i].is_whitespace() {
            punctuation_since_strong = true;
        }
        i += 1;
    }
    result
}

/// Arabic punctuation (comma, semicolon, question mark) between two Latin
/// words. It has no direction of its own, so the bidi algorithm gives it the
/// direction of the Latin text around it (UAX #9, rule N1) and the items join
/// one left-to-right run: "a.rs، b.rs" reads "b.rs" first in an RTL
/// paragraph. The mark is Persian punctuation, so the items belong to the
/// RTL text; the viewer adds a right-to-left mark after each one. Ranges are
/// UTF-16 offsets, as in [`detached_numbers`].
pub fn latin_separators(text: &str) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut previous_strong = None;
    let mut offset = 0;
    for (i, c) in text.char_indices() {
        if matches!(c, '،' | '؛' | '؟')
            && previous_strong == Some(Direction::Ltr)
            && text[i + c.len_utf8()..].chars().find_map(strong_direction) == Some(Direction::Ltr)
        {
            result.push((offset, offset + c.len_utf16()));
        }
        if let Some(direction) = strong_direction(c) {
            previous_strong = Some(direction);
        }
        offset += c.len_utf16();
    }
    result
}

/// Digits the bidi algorithm treats as European numbers: ASCII and Persian
/// (Extended Arabic-Indic). Arabic-Indic digits (U+0660..0669) are Arabic
/// numbers and are never attached to Latin text.
fn is_european_digit(c: char) -> bool {
    c.is_ascii_digit() || ('\u{06F0}'..='\u{06F9}').contains(&c)
}

/// End (exclusive) of the number starting at `start`: digits, and a single
/// separator between digits ("3.12", "1,000", "10:30", "۱۲٫۵").
fn number_run_end(chars: &[char], start: usize) -> usize {
    let mut end = start;
    loop {
        while end < chars.len() && is_european_digit(chars[end]) {
            end += 1;
        }
        let separated_digit = chars
            .get(end)
            .is_some_and(|&c| matches!(c, '.' | ',' | ':' | '/' | '٫' | '٬'))
            && chars.get(end + 1).is_some_and(|&c| is_european_digit(c));
        if !separated_digit {
            return end;
        }
        end += 1;
    }
}

/// Bidi strength of a character: letters and the directional marks.
fn strong_direction(c: char) -> Option<Direction> {
    match c {
        '\u{200F}' | '\u{061C}' => Some(Direction::Rtl),
        '\u{200E}' => Some(Direction::Ltr),
        _ if c.is_alphabetic() && is_rtl_letter(c) => Some(Direction::Rtl),
        _ if c.is_alphabetic() => Some(Direction::Ltr),
        _ => None,
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

    // --- detached_numbers -----------------------------------------------

    /// The runs `detached_numbers` marks, as strings.
    fn detached(text: &str) -> Vec<String> {
        let units: Vec<u16> = text.encode_utf16().collect();
        detached_numbers(text)
            .into_iter()
            .map(|(start, end)| String::from_utf16(&units[start..end]).unwrap())
            .collect()
    }

    #[test]
    fn numbers_after_a_latin_word_and_a_comma_are_detached() {
        let text = "۲ هسته‌ی CPU، ۲ گیگابایت RAM، ۲۰ گیگابایت";
        assert_eq!(detached(text), ["۲", "۲۰"]);
        // The second "۲" (after "CPU، "), not the first one, in UTF-16 units.
        let after_cpu = text.find("CPU، ").unwrap() + "CPU، ".len();
        let start = text[..after_cpu].encode_utf16().count();
        assert_eq!(detached_numbers(text)[0], (start, start + 1));
    }

    #[test]
    fn ascii_digits_after_a_latin_comma_are_detached() {
        assert_eq!(detached("حافظه: RAM, 16 گیگابایت"), ["16"]);
    }

    #[test]
    fn a_number_at_the_end_after_latin_punctuation_is_detached_whole() {
        assert_eq!(detached("نسخه‌ی مورد نیاز Python: 3.12"), ["3.12"]);
    }

    #[test]
    fn version_numbers_stay_with_their_latin_word() {
        assert!(detached("نسخه‌ی Windows 11 و Python ۳ را نصب کنید").is_empty());
    }

    #[test]
    fn numbers_followed_by_latin_text_stay() {
        assert!(detached("مشخصات CPU, 2 GB است").is_empty());
    }

    #[test]
    fn numbers_after_rtl_text_are_left_alone() {
        assert!(detached("ساعت 10:30 است").is_empty());
        assert!(detached("در سال ۱۴۰۳، ۲ بار").is_empty());
    }

    #[test]
    fn arabic_indic_digits_are_left_alone() {
        // U+0660..0669 are Arabic numbers to the bidi algorithm; rule W7
        // does not attach them to Latin text.
        assert!(detached("CPU، ٢ گیگابایت").is_empty());
    }

    #[test]
    fn an_existing_right_to_left_mark_already_detaches() {
        assert!(detached("CPU، \u{200F}۲ گیگابایت").is_empty());
    }

    #[test]
    fn paragraphs_carry_their_detached_numbers() {
        let paragraphs = paragraphs("RAM، ۲۰ گیگابایت\nWindows 11");
        assert_eq!(paragraphs[0].numbers, detached_numbers("RAM، ۲۰ گیگابایت"));
        assert!(!paragraphs[0].numbers.is_empty());
        assert!(paragraphs[1].numbers.is_empty());
    }

    // --- latin_separators ------------------------------------------------

    /// The characters `latin_separators` marks, as strings.
    fn separators(text: &str) -> Vec<String> {
        let units: Vec<u16> = text.encode_utf16().collect();
        latin_separators(text)
            .into_iter()
            .map(|(start, end)| String::from_utf16(&units[start..end]).unwrap())
            .collect()
    }

    #[test]
    fn arabic_commas_between_latin_items_are_marked() {
        let text = "فایل‌ها: a.rs، b.rs، c.rs و migration 0008";
        assert_eq!(separators(text), ["،", "،"]);
        let first = text[..text.find('،').unwrap()].encode_utf16().count();
        assert_eq!(latin_separators(text)[0], (first, first + 1));
    }

    #[test]
    fn a_latin_word_before_a_zwnj_suffix_counts_as_latin() {
        assert_eq!(separators("دامنه‌ی preferences، command‌ها، state.rs"), ["،"]);
    }

    #[test]
    fn arabic_semicolon_and_question_mark_are_marked() {
        assert_eq!(separators("Rust؛ Go"), ["؛"]);
        assert_eq!(separators("آیا Rust؟ Go هم"), ["؟"]);
    }

    #[test]
    fn separators_next_to_rtl_text_are_left_alone() {
        assert!(separators("Rust، فایل‌های جدید: storage، دامنه‌ی").is_empty());
        assert!(separators("Rust،").is_empty());
    }

    #[test]
    fn latin_punctuation_is_left_alone() {
        assert!(separators("فایل‌ها: a.rs, b.rs; c.rs").is_empty());
    }

    #[test]
    fn an_existing_right_to_left_mark_already_separates() {
        assert!(separators("a.rs،\u{200F} b.rs").is_empty());
    }

    #[test]
    fn paragraphs_carry_their_latin_separators() {
        let paragraphs = paragraphs("فایل‌ها: a.rs، b.rs\nRust، Go");
        assert_eq!(
            paragraphs[0].separators,
            latin_separators("فایل‌ها: a.rs، b.rs")
        );
        assert!(!paragraphs[0].separators.is_empty());
        assert!(!paragraphs[1].separators.is_empty());
    }
}
