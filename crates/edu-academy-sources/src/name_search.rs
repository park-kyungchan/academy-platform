//! Private inert projection of the pinned assessmentReadProtocol name-search parser.
//! This matches the original literal regexp, not JavaScript syntax or executable code.
use crate::data::*;
use regex::Regex;
use std::{collections::HashSet, fmt::Write, sync::OnceLock};

// ECMAScript \s, which differs from Rust regexp Unicode whitespace (notably FEFF/0085).
const SPACE: &str = r"[\x09-\x0d\x20\u{00a0}\u{1680}\u{2000}-\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}]";

pub(super) fn password_input(html: &str) -> bool {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN
        .get_or_init(|| {
            Regex::new(&format!(
                r#"(?i-u:<input\b)[^>]*(?i-u:\btype){SPACE}*={SPACE}*["'](?i-u:password)["']"#
            ))
            .expect("fixed password regexp")
        })
        .is_match(html)
}

fn name(value: &Js) -> Result<()> {
    if !value.string() || trim(value.units()).is_empty() || value.units().len() > 200 {
        return Err(Error::error("invalid_student_name"));
    }
    Ok(())
}

fn source_key(value: &Js) -> Result<String> {
    if let Js::Num(n) = value {
        if n.is_finite() && *n > 0.0 && n.fract() == 0.0 && *n <= 9_007_199_254_740_991.0 {
            return Ok(format!("{n:.0}"));
        }
    } else if let Js::Str(units) = value {
        if !units.is_empty()
            && units.iter().all(|u| (48..=57).contains(u))
            && units.iter().any(|u| *u != 48)
        {
            return Ok(string(value));
        }
    }
    Err(Error::error("invalid_source_key"))
}

fn decode_single_quoted(source: &str) -> Result<Text> {
    let units = text(source);
    let mut decoded = Vec::with_capacity(units.len());
    let mut i = 0;
    while i < units.len() {
        let unit = units[i];
        i += 1;
        if unit != 92 {
            decoded.push(unit);
            continue;
        }
        let escape = *units
            .get(i)
            .ok_or_else(|| Error::error("invalid_embedded_escape"))?;
        i += 1;
        decoded.push(match escape {
            39 | 34 | 92 | 47 => escape,
            110 => 10,
            114 => 13,
            116 => 9,
            98 => 8,
            102 => 12,
            117 | 120 => {
                let width = if escape == 117 { 4 } else { 2 };
                let hex = units
                    .get(i..i + width)
                    .ok_or_else(|| Error::error("invalid_embedded_escape"))?;
                let mut number = 0;
                for digit in hex {
                    let n = match *digit {
                        48..=57 => *digit - 48,
                        65..=70 => *digit - 65 + 10,
                        97..=102 => *digit - 97 + 10,
                        _ => return Err(Error::error("invalid_embedded_escape")),
                    };
                    number = (number << 4) | n;
                }
                i += width;
                number
            }
            _ => return Err(Error::error("invalid_embedded_escape")),
        });
    }
    Ok(decoded)
}

/// The central JSON codec takes UTF-8. Preserve raw UTF-16 surrogate units by
/// spelling them as JSON escapes, without parsing a second grammar. A surrogate
/// after an odd backslash run is an invalid JSON escape, never a repaired value.
fn parse_decoded(units: &[u16]) -> Result<Js> {
    let mut json_text = String::with_capacity(units.len());
    let mut escaped = false;
    for unit in units {
        if (0xd800..=0xdfff).contains(unit) {
            if escaped {
                return parse("\\"); // Central codec's SyntaxError, not an invented domain code.
            }
            write!(json_text, "\\u{unit:04x}").expect("String write");
        } else {
            json_text.push(char::from_u32(u32::from(*unit)).expect("non-surrogate UTF-16 unit"));
        }
        escaped = *unit == 92 && !escaped;
    }
    parse(&json_text)
}

pub(super) fn inspect(html: &str, requested_name: &Js) -> Result<Js> {
    name(requested_name)?;
    if html.encode_utf16().count() > 2_000_000 || password_input(html) {
        return Err(Error::error("student_search_source_rejected"));
    }
    static LITERAL: OnceLock<Regex> = OnceLock::new();
    let pattern = LITERAL.get_or_init(|| {
        // JS non-u \b is ASCII; JS dot excludes all four line terminators.
        Regex::new(&format!(
            r"(?-u:\b)testResultList{SPACE}*:{SPACE}*JSON\.parse\({SPACE}*'((?:\\[^\n\r\u{{2028}}\u{{2029}}]|[^'\\])*)'{SPACE}*\)"
        ))
        .expect("fixed result-list regexp")
    });
    let mut matches = pattern.captures_iter(html);
    let first = matches
        .next()
        .ok_or_else(|| Error::error("result_list_missing_or_ambiguous"))?;
    if matches.next().is_some() {
        return Err(Error::error("result_list_missing_or_ambiguous"));
    }
    let raw = parse_decoded(&decode_single_quoted(&first[1])?)?;
    if !array(&raw) || raw.array().len() > 2000 {
        return Err(Error::error("student_search_list_rejected"));
    }
    let mut seen = HashSet::new();
    let mut pairs = vec![];
    for row in raw.array() {
        if !object(row) {
            return Err(Error::error("invalid_capture_shape"));
        }
        name(row.get("userName"))?;
        if row.get("userName") == requested_name {
            let key = source_key(row.get("priNo"))?;
            if seen.insert(key.clone()) {
                pairs.push(obj(&[
                    ("studentKey", key.into()),
                    ("studentName", requested_name.clone()),
                ]));
            }
        }
    }
    if pairs.len() > 200 {
        return Err(Error::error("student_search_list_rejected"));
    }
    Ok(Js::Arr(pairs))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_parser_matches_all_31_pinned_original_reference_cases() {
        use std::{fs, path::PathBuf};
        const ROOT: &str =
            "/home/palantirkc/work/ws-replan/preparation/slices/S11/name-search-reference-v1";
        const IDS: [&str; 31] = [
            "empty",
            "ordered-dedupe-key-forms",
            "nonmatching-key-not-validated",
            "invalid-nonmatching-name",
            "matching-zero-key",
            "matching-unsafe-number",
            "matching-fraction",
            "invalid-object",
            "not-array",
            "requested-number",
            "requested-empty",
            "requested-whitespace",
            "name-leading-space-exact",
            "name-lone-surrogate",
            "name-astral-200-units",
            "name-astral-over-200-units",
            "200-unique",
            "201-unique",
            "2000-duplicate-rows",
            "2001-rows",
            "missing-literal",
            "ambiguous-literal",
            "invalid-escape",
            "malformed-json",
            "password-input",
            "hex-unicode-escaped-json",
            "ascii-word-boundary",
            "js-feff-whitespace",
            "non-js-0085-whitespace",
            "html-exact-2m-units",
            "html-over-2m-units",
        ];
        let selected = std::env::var_os("S11_NAME_REFERENCE_ROOT")
            .expect("Root must explicitly select S11_NAME_REFERENCE_ROOT; no default or skip");
        let root = PathBuf::from(selected);
        assert!(root.is_absolute());
        assert_eq!(
            root.as_os_str(),
            std::ffi::OsStr::new(ROOT),
            "exact Root-selected corpus only"
        );
        assert_eq!(
            fs::canonicalize(&root).expect("canonical corpus root"),
            root
        );
        let result_path = root.join("ORIGINAL-RESULTS.json");
        assert!(
            fs::symlink_metadata(&result_path)
                .expect("original capture required")
                .file_type()
                .is_file()
        );
        let result_bytes = fs::read(&result_path).expect("read immutable original results");
        assert_eq!(
            hash(&result_bytes),
            "73e6302671185f9d93c79d963253cc55db697bac6242aefe9a52bb096e861c5b",
            "exact captured original-results bytes; never silently repin"
        );
        let reference = parse(std::str::from_utf8(&result_bytes).expect("results UTF-8"))
            .expect("original results JSON");
        assert_eq!(
            reference.get("sourceBase"),
            &Js::from("27506037debcf519bc3f3ed42faaad478656560d")
        );
        assert_eq!(reference.get("bunVersion"), &Js::from("1.4.3"));
        assert_eq!(reference.get("originalOnly"), &Js::Bool(true));
        assert_eq!(reference.get("serviceRequests"), &Js::Num(0.0));
        assert_eq!(reference.get("originalCaptureVersion"), &Js::Num(1.0));
        let cases = reference.get("cases");
        assert!(array(cases));
        assert_eq!(cases.array().len(), IDS.len(), "no omitted or extra cases");
        let mut mismatches = vec![];
        for (case, id) in cases.array().iter().zip(IDS) {
            assert_eq!(case.get("id"), &Js::from(id), "exact ordered corpus ids");
            let input_path = root.join("inputs").join(format!("{id}.json"));
            assert_eq!(
                case.get("inputPath"),
                &Js::from(input_path.to_str().expect("canonical input path"))
            );
            assert_eq!(
                fs::canonicalize(&input_path).expect("canonical input"),
                input_path
            );
            assert!(
                fs::symlink_metadata(&input_path)
                    .expect("input required")
                    .file_type()
                    .is_file()
            );
            let bytes = fs::read(&input_path).expect("read immutable input");
            assert_eq!(
                case.get("inputSha256"),
                &Js::from(hash(&bytes)),
                "input sha mismatch: {id}"
            );
            let input =
                parse(std::str::from_utf8(&bytes).expect("input UTF-8")).expect("input JSON");
            assert_eq!(input.get("id"), &Js::from(id));
            assert!(input.get("html").string());
            assert_eq!(
                case.get("htmlUtf16Units"),
                &Js::from(input.get("html").units().len())
            );
            assert!(input.has("requestedName"));
            let html = String::from_utf16(input.get("html").units())
                .expect("captured HTML is scalar text");
            let actual = match inspect(&html, input.get("requestedName")) {
                Ok(value) => obj(&[("outcome", "returned".into()), ("value", value)]),
                Err(error) => obj(&[
                    ("outcome", "threw".into()),
                    ("name", error.name.into()),
                    ("message", error.message.into()),
                ]),
            };
            // Compare every case, including exact ordered values and error name/message.
            // No diagnostic normalization, SyntaxError exception, or early mismatch skip.
            if &actual != case.get("observed") {
                mismatches.push((id, actual, case.get("observed").clone()));
            }
        }
        assert!(
            mismatches.is_empty(),
            "original/candidate mismatches (actual, original): {mismatches:?}"
        );
    }

    fn literal(source: &str) -> String {
        format!("testResultList:JSON.parse('{source}')")
    }
    fn page(rows: Vec<Js>) -> String {
        literal(
            &json(&Js::Arr(rows))
                .replace('\\', "\\\\")
                .replace('\'', "\\'"),
        )
    }
    fn row(key: Js, name: Js) -> Js {
        obj(&[("priNo", key), ("userName", name)])
    }
    fn rejected(html: &str, name: &Js, code: &str) {
        let error = inspect(html, name).unwrap_err();
        assert_eq!(error.name, "Error");
        assert_eq!(error.message, code);
    }

    #[test]
    fn name_projection_preserves_exact_names_first_seen_order_and_string_key_identity() {
        let name: Js = "Fictional Learner".into();
        let html = page(vec![
            row(
                "invalid ignored key".into(),
                "Other Fictional Learner".into(),
            ),
            row(2usize.into(), name.clone()),
            row("01".into(), name.clone()),
            row(1usize.into(), name.clone()),
            row("2".into(), name.clone()),
            row("0001".into(), name.clone()),
            row("9007199254740992000000000".into(), name.clone()),
            row(Js::Null, "Fictional Learner ".into()),
        ]);
        let output = inspect(&html, &name).unwrap();
        assert_eq!(
            output,
            Js::Arr(
                ["2", "01", "1", "0001", "9007199254740992000000000"]
                    .map(|key| obj(&[("studentKey", key.into()), ("studentName", name.clone())]))
                    .to_vec()
            )
        );
        assert_eq!(
            inspect(&html, &"Absent Fictional Learner".into()).unwrap(),
            Js::Arr(vec![])
        );
    }

    #[test]
    fn name_source_keys_use_safe_numbers_and_unbounded_nonzero_digit_strings() {
        for key in [
            Js::Num(0.0),
            Js::Num(-0.0),
            Js::Num(-1.0),
            Js::Num(1.5),
            Js::Num(9_007_199_254_740_992.0),
            Js::Num(f64::NAN),
            Js::Num(f64::INFINITY),
            "".into(),
            "000".into(),
            "+1".into(),
            "1\n".into(),
            "１".into(),
            Js::Bool(true),
            Js::Null,
            Js::Undefined,
        ] {
            assert_eq!(source_key(&key).unwrap_err().message, "invalid_source_key");
        }
        assert_eq!(
            source_key(&Js::Num(9_007_199_254_740_991.0)).unwrap(),
            "9007199254740991"
        );
        let huge = "0".repeat(3000) + "1";
        assert_eq!(source_key(&huge.clone().into()).unwrap(), huge);
        // JSON.parse applies IEEE-754 rounding before Number.isSafeInteger.
        assert_eq!(
            inspect(
                &literal(r#"[{"userName":"F","priNo":1.00000000000000001}]"#),
                &"F".into()
            )
            .unwrap()
            .array()[0]
                .get("studentKey"),
            &Js::from("1")
        );
    }

    #[test]
    fn name_rows_validate_every_shape_and_name_but_only_matching_keys() {
        for value in [
            Js::Null,
            Js::Bool(false),
            1usize.into(),
            "F".into(),
            Js::Arr(vec![]),
        ] {
            rejected(&page(vec![value]), &"F".into(), "invalid_capture_shape");
        }
        for name in [
            Js::Undefined,
            Js::Null,
            1usize.into(),
            "".into(),
            "\u{feff}\u{2028}".into(),
            "x".repeat(201).into(),
        ] {
            rejected(
                &page(vec![row(Js::Null, name)]),
                &"F".into(),
                "invalid_student_name",
            );
        }
        rejected(
            &page(vec![row(Js::Null, "F".into())]),
            &"F".into(),
            "invalid_source_key",
        );
        assert_eq!(
            inspect(&page(vec![row(Js::Null, "Other".into())]), &"F".into()).unwrap(),
            Js::Arr(vec![])
        );
        rejected("", &"\u{feff}".into(), "invalid_student_name");
    }

    #[test]
    fn name_unicode_utf16_and_lone_surrogates_are_lossless() {
        for name in [
            "😀".repeat(100).into(),
            "\u{85}".into(),
            " e\u{301} ".into(),
            Js::Str(vec![0xd800]),
            Js::Str(vec![0xdc00]),
            Js::Str(vec![0xd800, 65, 0xdc00]),
        ] {
            let output = inspect(&page(vec![row(1usize.into(), name.clone())]), &name).unwrap();
            assert_eq!(output.array()[0].get("studentName"), &name);
        }
        rejected(
            &page(vec![]),
            &"😀".repeat(101).into(),
            "invalid_student_name",
        );
        assert_eq!(
            inspect(
                &page(vec![row(1usize.into(), "é".into())]),
                &"e\u{301}".into()
            )
            .unwrap(),
            Js::Arr(vec![])
        );
        // Outer JS escape creates raw UTF-16 in the JSON text, including an unpaired unit.
        let output = inspect(
            &literal(r#"[{"userName":"\uD800","priNo":1}]"#),
            &Js::Str(vec![0xd800]),
        )
        .unwrap();
        assert_eq!(output.array()[0].get("studentName"), &Js::Str(vec![0xd800]));
        // Raw surrogate after a JSON escape introducer must remain a SyntaxError.
        assert_eq!(
            parse_decoded(&[34, 92, 0xd800, 34]).unwrap_err().name,
            "SyntaxError"
        );
        assert_eq!(
            parse_decoded(&[34, 92, 92, 0xd800, 34]).unwrap(),
            Js::Str(vec![92, 0xd800])
        );
    }

    #[test]
    fn name_literal_regexp_has_js_whitespace_ascii_boundary_and_nonexecuting_scope() {
        let name = "F".into();
        for html in [
            literal("[]"),
            format!("é{}", literal("[]")),
            "testResultList\u{feff}:\u{a0}JSON.parse(\u{2028}'[]'\u{3000})".into(),
            "// inert comment testResultList:JSON.parse('[]'); throw Error('never run')".into(),
            literal("[\n]"),
        ] {
            assert_eq!(inspect(&html, &name).unwrap(), Js::Arr(vec![]));
        }
        // U+2028 is regexp whitespace, but is not JSON whitespace.
        assert_eq!(
            inspect(&literal("[\u{2028}\"x\"]"), &name)
                .unwrap_err()
                .name,
            "SyntaxError"
        );
        for html in [
            format!("x{}", literal("[]")),
            "testResultList\u{85}:JSON.parse('[]')".into(),
            "testResultList:JSON.parse ('[]')".into(),
            "testResultList:JSON.parse(\"[]\")".into(),
            "testResultList:JSON.parse('[]' + '[]')".into(),
            "TestResultList:JSON.parse('[]')".into(),
            literal("\\\n"),
            literal("\\\r"),
            literal("\\\u{2028}"),
            literal("\\\u{2029}"),
        ] {
            rejected(&html, &name, "result_list_missing_or_ambiguous");
        }
        rejected(
            &(literal("[]") + &literal("[]")),
            &name,
            "result_list_missing_or_ambiguous",
        );
        // A malformed first literal is not a match; exactly one later match is accepted.
        assert_eq!(
            inspect(&(literal("\\\n") + &literal("[]")), &name).unwrap(),
            Js::Arr(vec![])
        );
    }

    #[test]
    fn name_literal_decoder_preserves_only_original_escapes_and_json_errors() {
        assert_eq!(
            decode_single_quoted(r#"\'\"\\\n\r\t\b\f\/\u0041\x42"#).unwrap(),
            text("'\"\\\n\r\t\x08\x0c/AB")
        );
        for source in [
            "\\", r"\v", r"\0", r"\u{41}", r"\u123", r"\uZZZZ", r"\x0", r"\xGG",
        ] {
            assert_eq!(
                decode_single_quoted(source).unwrap_err().message,
                "invalid_embedded_escape"
            );
        }
        for source in [r"\v", r"\uZZZZ", r"\x0"] {
            rejected(&literal(source), &"F".into(), "invalid_embedded_escape");
        }
        for source in ["undefined", "[", "[1,]", r#"[{"userName":"\n","priNo":1}]"#] {
            assert_eq!(
                inspect(&literal(source), &"F".into()).unwrap_err().name,
                "SyntaxError"
            );
        }
        for source in ["null", "{}", "1", "true", r#""F""#] {
            rejected(
                &literal(source),
                &"F".into(),
                "student_search_list_rejected",
            );
        }
    }

    #[test]
    fn name_raw_and_unique_limits_preserve_validation_order() {
        let name: Js = "F".into();
        let rows = |n: i32| -> Vec<Js> {
            (1..=n)
                .map(|k| row(Js::Num(f64::from(k)), name.clone()))
                .collect()
        };
        assert_eq!(inspect(&page(rows(200)), &name).unwrap().array().len(), 200);
        rejected(&page(rows(201)), &name, "student_search_list_rejected");
        assert_eq!(
            inspect(&page(vec![row(1usize.into(), name.clone()); 2000]), &name)
                .unwrap()
                .array()
                .len(),
            1
        );
        rejected(
            &page(vec![row(1usize.into(), name.clone()); 2001]),
            &name,
            "student_search_list_rejected",
        );
        let mut invalid_after_many = rows(201);
        invalid_after_many.push(row(Js::Null, name.clone()));
        rejected(&page(invalid_after_many), &name, "invalid_source_key");
    }

    #[test]
    fn name_source_limit_and_password_pattern_follow_js_units_and_ascii_casefold() {
        let name = "F".into();
        let mut html = literal("[]");
        html.push_str(&"😀".repeat((2_000_000 - html.len()) / 2));
        if html.encode_utf16().count() < 2_000_000 {
            html.push(' ');
        }
        assert_eq!(html.encode_utf16().count(), 2_000_000);
        assert_eq!(inspect(&html, &name).unwrap(), Js::Arr(vec![]));
        rejected(&(html + "x"), &name, "student_search_source_rejected");
        for input in [
            "<INPUT TYPE='PASSWORD'",
            "<input data-type\u{feff}=\u{a0}\"password'",
            "<inputé type='password'",
        ] {
            rejected(
                &(literal("[]") + input),
                &name,
                "student_search_source_rejected",
            );
        }
        for input in [
            "<input type=password>",
            "<input type='paſſword'>",
            "<input type\u{85}='password'>",
            "<inputx type='password'>",
        ] {
            assert_eq!(
                inspect(&(literal("[]") + input), &name).unwrap(),
                Js::Arr(vec![])
            );
        }
    }
}
