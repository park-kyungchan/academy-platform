use crate::js::*;
pub fn calendar_date(v: &Js) -> Result<Js> {
    let s = v.code_units();
    let b = s.as_bytes();
    if !v.string()
        || b.len() != 10
        || b[4] != b'-'
        || b[7] != b'-'
        || b.iter()
            .enumerate()
            .any(|(i, c)| i != 4 && i != 7 && !c.is_ascii_digit())
    {
        return Err(Fault::error("invalid calendar date"));
    }
    let y = s[..4].parse::<i32>().unwrap_or(-1);
    let m = s[5..7].parse::<u32>().unwrap_or(0);
    let d = s[8..].parse::<u32>().unwrap_or(0);
    let days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    };
    if d == 0 || d > days {
        Err(Fault::error("invalid calendar date"))
    } else {
        Ok(v.clone())
    }
}
/// Sunday 0, proleptic Gregorian including year zero (learning differs from CISM).
pub fn weekday(s: &str) -> usize {
    let mut y = s[..4].parse::<i32>().expect("validated date");
    let m = s[5..7].parse::<usize>().expect("validated date");
    let d = s[8..].parse::<i32>().expect("validated date");
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    if m < 3 {
        y -= 1;
    }
    (y + y.div_euclid(4) - y.div_euclid(100) + y.div_euclid(400) + t[m - 1] + d).rem_euclid(7)
        as usize
}
pub fn assert_refs(v: &Js) -> Result<()> {
    if let Js::Sparse(n) = v {
        if *n > 1 {
            return Err(Fault::error("duplicate evidence reference"));
        } else if *n == 1 {
            return Ok(());
        }
    }
    if !matches!(v, Js::Arr(_)) || v.array().is_empty() {
        return Err(Fault::error("known data needs evidence references"));
    }
    for x in v.array() {
        require_text(x, "evidence reference")?;
    }
    if unique(v.array()).len() != v.array().len() {
        return Err(Fault::error("duplicate evidence reference"));
    }
    Ok(())
}
pub fn assert_knowledge(v: &Js) -> Result<()> {
    if v.absent()
        || !matches!(
            v,
            Js::Obj(_)
                | Js::Arr(_)
                | Js::NullPrototype(_)
                | Js::Inherited(_)
                | Js::Accessor(_)
                | Js::SymbolKeys(_)
                | Js::Date(_)
                | Js::Sparse(_)
                | Js::Decorated(_, _)
                | Js::Cycle
        )
    {
        return Err(Fault::error("missing knowledge state"));
    }
    if v.get("state").is("unknown") {
        exact(v, &["state", "reason"])?;
        require_text(v.get("reason"), "unknown reason")?;
        return Ok(());
    }
    if !v.get("state").is("known") || v.get("value").absent() {
        return Err(Fault::error("invalid knowledge state"));
    }
    exact(v, &["state", "value", "evidenceRefs"])?;
    assert_refs(v.get("evidenceRefs"))
}
pub fn known(value: &Js, refs: &Js) -> Result<Js> {
    let v = obj(&[
        ("state", "known".into()),
        ("value", value.clone()),
        ("evidenceRefs", refs.clone()),
    ]);
    assert_knowledge(&v)?;
    immutable_copy(&v)
}
/// Source JSON clone order and negative-zero conversion. This mutable data helper
/// feeds FrozenValue for native observers. Existing JS callers keep their TS port.
pub fn immutable_copy(v: &Js) -> Result<Js> {
    encode(v, Family::Learning, &mut vec![])?;
    fn copy(v: &Js) -> Js {
        match v {
            Js::Num(n) if *n == 0.0 => Js::Num(0.0),
            Js::Arr(a) => Js::Arr(a.iter().map(copy).collect()),
            Js::Obj(p) | Js::NullPrototype(p) => {
                let mut p = p.clone();
                p.sort_by(|a, b| a.0.cmp(&b.0));
                Js::Obj(p.iter().map(|(k, v)| (k.clone(), copy(v))).collect())
            }
            _ => v.clone(),
        }
    }
    Ok(copy(v))
}
pub fn unknown(reason: &Js) -> Result<Js> {
    require_text(reason, "unknown reason")?;
    Ok(obj(&[
        ("state", "unknown".into()),
        ("reason", reason.clone()),
    ]))
}
