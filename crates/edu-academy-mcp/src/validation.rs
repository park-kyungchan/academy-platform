//! Source-used JSON Schema subset, with explicit refusal of unsupported contracts.
//! This does not apply endpoint Zod defaults, trims or domain refinements.
use crate::{Error, Js, Result, string};
const INVALID: Error = Error::new("tool_input_invalid");
const UNSUPPORTED: Error = Error::new("schema_validation_qualification_gap");

pub fn check_schema(schema: &Js) -> Result<()> {
    check(schema, 0)
}
fn check(s: &Js, depth: usize) -> Result<()> {
    if depth > 128 {
        return Err(UNSUPPORTED);
    }
    if matches!(s, Js::Bool(_)) {
        return Ok(());
    }
    if !matches!(s, Js::Obj(_)) {
        return Err(UNSUPPORTED);
    }
    const KEYS: &[&str] = &[
        "$schema",
        "type",
        "properties",
        "required",
        "additionalProperties",
        "propertyNames",
        "items",
        "minItems",
        "maxItems",
        "uniqueItems",
        "minLength",
        "maxLength",
        "minimum",
        "maximum",
        "exclusiveMinimum",
        "pattern",
        "format",
        "const",
        "enum",
        "anyOf",
        "oneOf",
        "default",
        "description",
    ];
    for (key, _) in s.props() {
        if !KEYS.iter().any(|known| key == &crate::text(known)) {
            return Err(UNSUPPORTED);
        }
    }
    if s.has("$schema")
        && ![
            "https://json-schema.org/draft/2020-12/schema",
            "https://json-schema.org/draft/2019-09/schema",
            "http://json-schema.org/draft-07/schema#",
            "http://json-schema.org/draft-06/schema#",
        ]
        .iter()
        .any(|known| s.get("$schema").is(known))
    {
        return Err(UNSUPPORTED);
    }
    if s.has("type")
        && ![
            "object", "array", "string", "number", "integer", "boolean", "null",
        ]
        .iter()
        .any(|known| s.get("type").is(known))
    {
        return Err(UNSUPPORTED);
    }
    for key in ["minItems", "maxItems", "minLength", "maxLength"] {
        if s.has(key)
            && !matches!(s.get(key), Js::Num(n) if n.is_finite() && *n >= 0.0 && n.fract() == 0.0)
        {
            return Err(UNSUPPORTED);
        }
    }
    for key in ["minimum", "maximum", "exclusiveMinimum"] {
        if s.has(key) && !matches!(s.get(key), Js::Num(n) if n.is_finite()) {
            return Err(UNSUPPORTED);
        }
    }
    if s.has("required")
        && (!matches!(s.get("required"), Js::Arr(_))
            || s.get("required").array().iter().any(|v| !v.string()))
    {
        return Err(UNSUPPORTED);
    }
    if s.has("enum") && (!matches!(s.get("enum"), Js::Arr(_)) || s.get("enum").array().is_empty()) {
        return Err(UNSUPPORTED);
    }
    if s.has("uniqueItems") && !matches!(s.get("uniqueItems"), Js::Bool(_)) {
        return Err(UNSUPPORTED);
    }
    if s.has("properties") {
        if !matches!(s.get("properties"), Js::Obj(_)) {
            return Err(UNSUPPORTED);
        }
        for (_, child) in s.get("properties").props() {
            check(child, depth + 1)?;
        }
    }
    for key in ["additionalProperties", "items", "propertyNames"] {
        if s.has(key) {
            check(s.get(key), depth + 1)?;
        }
    }
    for key in ["anyOf", "oneOf"] {
        if s.has(key) {
            if !matches!(s.get(key), Js::Arr(_)) || s.get(key).array().is_empty() {
                return Err(UNSUPPORTED);
            }
            for child in s.get(key).array() {
                check(child, depth + 1)?;
            }
        }
    }
    if s.has("pattern") {
        pattern(&string(s.get("pattern")).map_err(|_| UNSUPPORTED)?)?;
    }
    if s.has("format")
        && !["date", "date-time"]
            .iter()
            .any(|known| s.get("format").is(known))
    {
        return Err(UNSUPPORTED);
    }
    Ok(())
}

/// Equal JSON objects ignore member order; array order and scalar types stay exact.
pub fn json_equal(left: &Js, right: &Js) -> bool {
    let mut pending = vec![(left, right)];
    while let Some((a, b)) = pending.pop() {
        match (a, b) {
            (Js::Obj(x), Js::Obj(y)) => {
                if x.len() != y.len() {
                    return false;
                }
                for (key, value) in x {
                    let Some((_, other)) = y.iter().find(|(other, _)| key == other) else {
                        return false;
                    };
                    pending.push((value, other));
                }
            }
            (Js::Arr(x), Js::Arr(y)) => {
                if x.len() != y.len() {
                    return false;
                }
                pending.extend(x.iter().zip(y));
            }
            _ if a == b => (),
            _ => return false,
        }
    }
    true
}
fn pattern(source: &str) -> Result<regex::Regex> {
    // The pinned corpus uses ASCII patterns and ECMAScript ASCII \d.
    // Do not silently reinterpret arbitrary owner-provided JS regexp features.
    if !source.is_ascii()
        || source.contains("\\p")
        || source.contains("\\s")
        || source.contains("\\w")
        || source.contains("(?=")
        || source.contains("(?!")
    {
        return Err(UNSUPPORTED);
    }
    let mut translated = String::new();
    let mut chars = source.chars();
    let mut class = false;
    while let Some(c) = chars.next() {
        if c == '\\' {
            let escaped = chars.next().ok_or(UNSUPPORTED)?;
            if escaped == 'd' {
                translated.push_str(if class { "0-9" } else { "[0-9]" });
            } else if "\\.^$|?*+()[]{}/".contains(escaped) || (escaped == '-' && class) {
                // Consume the pair atomically: a literal backslash followed by
                // d must not become a digit class. JS unicode identity escapes
                // outside this proven subset stay explicit qualification gaps.
                if escaped == '/' {
                    translated.push('/');
                } else {
                    translated.push('\\');
                    translated.push(escaped);
                }
            } else {
                return Err(UNSUPPORTED);
            }
        } else {
            if c == '[' {
                if class {
                    return Err(UNSUPPORTED);
                }
                class = true;
            }
            if c == ']' {
                class = false;
            }
            if class && c == '&' {
                return Err(UNSUPPORTED);
            }
            translated.push(c);
        }
    }
    regex::Regex::new(&translated).map_err(|_| UNSUPPORTED)
}
fn limit(s: &Js, key: &str, size: usize, minimum: bool) -> bool {
    match s.get(key) {
        Js::Num(n) => {
            if minimum {
                size as f64 >= *n
            } else {
                size as f64 <= *n
            }
        }
        _ => true,
    }
}
fn unicode_length(units: &[u16]) -> usize {
    let mut pos = 0;
    let mut count = 0;
    while pos < units.len() {
        let pair = (0xd800..=0xdbff).contains(&units[pos])
            && units
                .get(pos + 1)
                .is_some_and(|u| (0xdc00..=0xdfff).contains(u));
        pos += if pair { 2 } else { 1 };
        count += 1;
    }
    count
}
pub fn validate(schema: &Js, value: &Js) -> Result<()> {
    check_schema(schema)?;
    validate_known(schema, value)
}
fn validate_known(s: &Js, v: &Js) -> Result<()> {
    if matches!(s, Js::Bool(true)) {
        return Ok(());
    }
    if matches!(s, Js::Bool(false)) {
        return Err(INVALID);
    }
    if s.has("const") && !json_equal(s.get("const"), v) {
        return Err(INVALID);
    }
    if s.has("enum") && !s.get("enum").array().iter().any(|x| json_equal(x, v)) {
        return Err(INVALID);
    }
    for key in ["anyOf", "oneOf"] {
        if s.has(key) {
            let mut matches = 0;
            for child in s.get(key).array() {
                match validate_known(child, v) {
                    Ok(()) => matches += 1,
                    Err(e) if e == INVALID => (),
                    Err(e) => return Err(e),
                }
            }
            if matches == 0 || (key == "oneOf" && matches != 1) {
                return Err(INVALID);
            }
        }
    }
    if s.has("type") {
        let valid = match string(s.get("type"))?.as_str() {
            "object" => matches!(v, Js::Obj(_)),
            "array" => matches!(v, Js::Arr(_)),
            "string" => v.string(),
            "boolean" => matches!(v, Js::Bool(_)),
            "null" => matches!(v, Js::Null),
            "number" => matches!(v, Js::Num(n) if n.is_finite()),
            "integer" => matches!(v, Js::Num(n) if n.is_finite() && n.fract() == 0.0),
            _ => return Err(UNSUPPORTED),
        };
        if !valid {
            return Err(INVALID);
        }
    }
    match v {
        Js::Obj(values) => {
            for required in s.get("required").array() {
                if !values.iter().any(|(key, _)| key == required.units()) {
                    return Err(INVALID);
                }
            }
            for (key, value) in values {
                if s.has("propertyNames") {
                    validate_known(s.get("propertyNames"), &Js::Str(key.clone()))?;
                }
                if let Some((_, schema)) = s
                    .get("properties")
                    .props()
                    .iter()
                    .find(|(name, _)| name == key)
                {
                    validate_known(schema, value)?;
                } else if s.has("additionalProperties") {
                    validate_known(s.get("additionalProperties"), value)?;
                }
            }
        }
        Js::Arr(values) => {
            if !limit(s, "minItems", values.len(), true)
                || !limit(s, "maxItems", values.len(), false)
            {
                return Err(INVALID);
            }
            for (i, value) in values.iter().enumerate() {
                if s.has("items") {
                    validate_known(s.get("items"), value)?;
                }
                if matches!(s.get("uniqueItems"), Js::Bool(true))
                    && values[..i].iter().any(|x| json_equal(x, value))
                {
                    return Err(INVALID);
                }
            }
        }
        Js::Str(units) => {
            let size = unicode_length(units);
            if !limit(s, "minLength", size, true) || !limit(s, "maxLength", size, false) {
                return Err(INVALID);
            }
            if s.has("pattern") {
                let re = pattern(&string(s.get("pattern"))?)?;
                if !re.is_match(&String::from_utf16_lossy(units)) {
                    return Err(INVALID);
                }
            }
            if s.has("format") {
                let scalar = String::from_utf16(units).map_err(|_| INVALID)?;
                if !valid_format(&string(s.get("format"))?, &scalar) {
                    return Err(INVALID);
                }
            }
        }
        Js::Num(n) => {
            if let Js::Num(bound) = s.get("minimum") {
                if n < bound {
                    return Err(INVALID);
                }
            }
            if let Js::Num(bound) = s.get("maximum") {
                if n > bound {
                    return Err(INVALID);
                }
            }
            if let Js::Num(bound) = s.get("exclusiveMinimum") {
                if n <= bound {
                    return Err(INVALID);
                }
            }
        }
        _ => (),
    }
    Ok(())
}
fn valid_format(kind: &str, value: &str) -> bool {
    fn date(s: &str) -> bool {
        edu_academy_core::knowledge::calendar_date(&s.into()).is_ok()
    }
    if kind == "date" {
        return date(value);
    }
    // Port of the pinned ajv-formats full date-time validator, including leap seconds.
    let parts = value
        .split(|c: char| {
            c == 't'
                || c == 'T'
                || (u32::from(c) <= 0xffff && edu_academy_core::js::js_space(c as u16))
        })
        .collect::<Vec<_>>();
    if parts.len() != 2 || !date(parts[0]) {
        return false;
    }
    let Ok(re) = regex::Regex::new(
        r"(?i)^([0-9]{2}):([0-9]{2}):([0-9]{2}(?:\.[0-9]+)?)(z|([+-])([0-9]{2})(?::?([0-9]{2}))?)$",
    ) else {
        return false;
    };
    let Some(c) = re.captures(parts[1]) else {
        return false;
    };
    let number = |i| {
        c.get(i)
            .and_then(|s| s.as_str().parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    let (h, m, sec, tz_h, tz_m) = (number(1), number(2), number(3), number(6), number(7));
    if tz_h > 23.0 || tz_m > 59.0 {
        return false;
    }
    if h <= 23.0 && m <= 59.0 && sec < 60.0 {
        return true;
    }
    let sign = if c.get(5).is_some_and(|s| s.as_str() == "-") {
        -1.0
    } else {
        1.0
    };
    let utc_m = m - tz_m * sign;
    let utc_h = h - tz_h * sign - if utc_m < 0.0 { 1.0 } else { 0.0 };
    (utc_h == 23.0 || utc_h == -1.0) && (utc_m == 59.0 || utc_m == -1.0) && sec < 61.0
}
