//! JavaScript text rules, defined once for the whole crate.
//! Each function states the ECMAScript or TypeScript rule it mirrors, so a
//! source module never re-derives whitespace, trimming, UTF-16 length or
//! bounded decoding by hand again.

/// The ECMAScript WhiteSpace and LineTerminator set matched by `\s` and by
/// `String.prototype.trim`: TAB, LF, VT, FF, CR, SPACE, U+FEFF (ZWNBSP),
/// U+00A0, U+1680, U+2000..=U+200A, U+2028, U+2029, U+202F, U+205F, U+3000.
/// U+0085 (NEL) is notably NOT in the set.
pub(crate) fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'
            | '\u{000A}'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{000D}'
            | '\u{0020}'
            | '\u{FEFF}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'
            ..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

/// The same set as a regex character class, for expressions that mirror a
/// JavaScript `\s` (the `regex` crate's own `\s` is the Unicode White_Space
/// property, which differs: it has U+0085 and lacks U+FEFF). A unit test keeps
/// this class equal to `is_js_whitespace` for every scalar value.
pub(crate) const JS_WS_CLASS: &str = r"[\t\n\x0B\x0C\r \x{FEFF}\x{00A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}]";

/// `value.trim()`: remove leading and trailing JS whitespace.
pub(crate) fn js_trim(value: &str) -> &str {
    value.trim_matches(is_js_whitespace)
}

/// `value.replace(/\s+/g, ' ').trim()`: collapse every run of JS whitespace
/// to one ASCII space, then trim.
pub(crate) fn js_collapse_whitespace(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut in_run = false;
    for c in value.chars() {
        if is_js_whitespace(c) {
            in_run = true;
        } else {
            if in_run && !out.is_empty() {
                out.push(' ');
            }
            in_run = false;
            out.push(c);
        }
    }
    out
}

/// JavaScript `value.length`: the number of UTF-16 code units.
pub(crate) fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

/// Why a bounded decode refused, before the caller maps its own code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecodeRefusal {
    TooLarge,
    NotUtf8,
}

/// The crate body rule (`student_course::response_text`): at most
/// `max_bytes` bytes, strict UTF-8, at most `max_utf16_units` UTF-16 units.
/// The caller maps `TooLarge` and `NotUtf8` to its own refusal codes.
pub(crate) fn decode_bounded(
    bytes: &[u8],
    max_bytes: usize,
    max_utf16_units: usize,
) -> std::result::Result<String, DecodeRefusal> {
    if bytes.len() > max_bytes {
        return Err(DecodeRefusal::TooLarge);
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| DecodeRefusal::NotUtf8)?
        .to_owned();
    if utf16_len(&text) > max_utf16_units {
        return Err(DecodeRefusal::TooLarge);
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitespace_set_matches_ecmascript() {
        for c in [
            '\t', '\n', '\u{000B}', '\u{000C}', '\r', ' ', '\u{FEFF}', '\u{00A0}', '\u{1680}',
            '\u{2000}', '\u{2007}', '\u{200A}', '\u{2028}', '\u{2029}', '\u{202F}', '\u{205F}',
            '\u{3000}',
        ] {
            assert!(is_js_whitespace(c), "{c:?}");
        }
        // Rust's char::is_whitespace accepts U+0085 and rejects U+FEFF;
        // JavaScript is the opposite on both.
        assert!(!is_js_whitespace('\u{0085}'));
        assert!(!is_js_whitespace('\u{200B}'));
        assert!(!is_js_whitespace('a'));
    }

    #[test]
    fn regex_class_equals_the_whitespace_set_for_every_scalar() {
        let re = regex::Regex::new(&format!("^{JS_WS_CLASS}$")).unwrap();
        let mut buf = [0u8; 4];
        for c in (0..=0x10FFFFu32).filter_map(char::from_u32) {
            assert_eq!(
                re.is_match(c.encode_utf8(&mut buf)),
                is_js_whitespace(c),
                "{:04X}",
                c as u32
            );
        }
    }

    #[test]
    fn trim_and_collapse_use_the_js_set() {
        assert_eq!(js_trim("\u{FEFF}\u{00A0} a b \u{3000}\n"), "a b");
        assert_eq!(js_trim("\u{0085}a\u{0085}"), "\u{0085}a\u{0085}");
        assert_eq!(js_collapse_whitespace("  a \t\n\u{00A0} b  "), "a b");
        assert_eq!(js_collapse_whitespace("\u{0085}"), "\u{0085}");
        assert_eq!(js_collapse_whitespace(" \n "), "");
    }

    #[test]
    fn utf16_len_counts_code_units() {
        assert_eq!(utf16_len(""), 0);
        assert_eq!(utf16_len("abc"), 3);
        assert_eq!(utf16_len("가"), 1);
        assert_eq!(utf16_len("😀"), 2);
    }

    #[test]
    fn decode_bounded_bounds_bytes_and_units_and_requires_utf8() {
        assert_eq!(decode_bounded("가가".as_bytes(), 8, 2).unwrap(), "가가");
        assert_eq!(
            decode_bounded("가가".as_bytes(), 5, 2),
            Err(DecodeRefusal::TooLarge)
        );
        // 6 bytes, 2 units: over the byte cap, under the unit cap.
        assert_eq!(
            decode_bounded("가가".as_bytes(), 6, 1),
            Err(DecodeRefusal::TooLarge)
        );
        assert_eq!(
            decode_bounded(&[0xFF, 0xFE], 8, 8),
            Err(DecodeRefusal::NotUtf8)
        );
    }
}
