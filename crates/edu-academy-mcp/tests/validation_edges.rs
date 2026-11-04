//! Exact pinned AJV/ECMAScript counterexamples; no runtime or domain admission.
use edu_academy_mcp::{Js, obj, validation};
fn schema(key: &str, value: &str) -> Js {
    obj(&[("type", "string".into()), (key, value.into())])
}
#[test]
fn ajv_datetime_uses_js_whitespace_not_rust_unicode_whitespace() {
    let schema = schema("format", "date-time");
    for separator in ["T", "t", " ", "\t", "\u{feff}"] {
        validation::validate(&schema, &format!("2026-11-03{separator}12:00:00Z").into()).unwrap();
    }
    for separator in ["\u{85}", "  ", "TT"] {
        assert_eq!(
            validation::validate(&schema, &format!("2026-11-03{separator}12:00:00Z").into())
                .unwrap_err()
                .code,
            "tool_input_invalid"
        );
    }
    assert!(validation::validate(&schema, &"2026-11-03T12:00:00".into()).is_err());
}
#[test]
fn regexp_digit_escape_and_literal_backslash_are_distinct() {
    for pattern in [r"^\d+$", r"^[\d]+$"] {
        let schema = schema("pattern", pattern);
        validation::validate(&schema, &"123".into()).unwrap();
        assert!(validation::validate(&schema, &"١".into()).is_err());
    }
    let literal = schema("pattern", r"^\\d$");
    validation::validate(&literal, &r"\d".into()).unwrap();
    assert!(validation::validate(&literal, &"1".into()).is_err());
    let backslash_digit = schema("pattern", r"^\\\d$");
    validation::validate(&backslash_digit, &r"\7".into()).unwrap();
    assert!(validation::validate(&backslash_digit, &r"\d".into()).is_err());
    for unsupported in [r"(?=a)", r"\w", r"\s", r"\p{L}"] {
        assert_eq!(
            validation::validate(&schema("pattern", unsupported), &"a".into())
                .unwrap_err()
                .code,
            "schema_validation_qualification_gap"
        );
    }
}
