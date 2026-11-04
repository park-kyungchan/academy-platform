//! Separate source grammars and Gregorian epoch arithmetic, including year zero.
use crate::{js::*, knowledge};
#[derive(Clone, Copy)]
pub enum Grammar {
    Learning,
    DailyRecord,
}
pub fn epoch_ms(v: &Js, grammar: Grammar) -> Option<i64> {
    let pattern = match grammar {
        Grammar::Learning => {
            r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T(?:[01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9](?:\.[0-9]{1,3})?(?:Z|[+-](?:[01][0-9]|2[0-3]):[0-5][0-9])$"
        }
        Grammar::DailyRecord => {
            r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T(?:[01][0-9]|2[0-3]):[0-5][0-9](?::[0-5][0-9](?:\.[0-9]+)?)?(?:Z|[+-](?:[01][0-9]|2[0-3]):[0-5][0-9])$"
        }
    };
    if !v.string() || !test(pattern, v) {
        return None;
    }
    let s = v.code_units();
    knowledge::calendar_date(&s[..10].into()).ok()?;
    let y = s[..4].parse::<i64>().ok()?;
    let m = s[5..7].parse::<usize>().ok()?;
    let d = s[8..10].parse::<i64>().ok()?;
    let leap = |y: i64| y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let days_before = |y: i64| {
        365 * y + (y + 3).div_euclid(4) - (y + 99).div_euclid(100) + (y + 399).div_euclid(400)
    };
    let months = [
        31,
        if leap(y) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let days = days_before(y) - days_before(1970) + months[..m - 1].iter().sum::<i64>() + d - 1;
    let h = s[11..13].parse::<i64>().ok()?;
    let minute = s[14..16].parse::<i64>().ok()?;
    let mut pos = 16;
    let mut second = 0;
    let mut millis = 0;
    if s.as_bytes()[pos] == b':' {
        second = s[pos + 1..pos + 3].parse().ok()?;
        pos += 3;
    }
    if s.as_bytes()[pos] == b'.' {
        pos += 1;
        let start = pos;
        while s.as_bytes()[pos].is_ascii_digit() {
            pos += 1;
        }
        for i in 0..3 {
            millis = millis * 10
                + if start + i < pos {
                    i64::from(s.as_bytes()[start + i] - b'0')
                } else {
                    0
                };
        }
    }
    let mut offset = 0;
    if s.as_bytes()[pos] != b'Z' {
        offset = (s[pos + 1..pos + 3].parse::<i64>().ok()? * 60
            + s[pos + 4..pos + 6].parse::<i64>().ok()?)
            * 60000;
        if s.as_bytes()[pos] == b'-' {
            offset = -offset;
        }
    }
    Some((((days * 24 + h) * 60 + minute) * 60 + second) * 1000 + millis - offset)
}
pub fn assert_instant(v: &Js) -> Result<()> {
    // Learning checks the grammar/Date first, then reports its separate calendar error.
    if !v.string()
        || !test(
            r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T(?:[01][0-9]|2[0-3]):[0-5][0-9]:[0-5][0-9](?:\.[0-9]{1,3})?(?:Z|[+-](?:[01][0-9]|2[0-3]):[0-5][0-9])$",
            v,
        )
    {
        return Err(Fault::error("invalid timestamp with timezone"));
    }
    let s = v.code_units();
    // Native Date.parse normalizes day 29..31; impossible month/day zero never parses.
    let month = s[5..7].parse::<u32>().unwrap_or(0);
    let day = s[8..10].parse::<u32>().unwrap_or(0);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(Fault::error("invalid timestamp with timezone"));
    }
    knowledge::calendar_date(&s[..10].into()).map(|_| ())
}
