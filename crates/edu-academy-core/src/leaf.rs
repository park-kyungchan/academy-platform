use crate::protocol::{LeafError, WeekPeriod};

pub fn journal_wire_width(units: &[u16]) -> Result<u64, LeafError> {
    Ok(units
        .iter()
        .map(|&unit| if unit <= 127 { 1 } else { 2 })
        .sum())
}

pub fn cism_week(date: &str) -> Result<WeekPeriod, LeafError> {
    let bytes = date.as_bytes();
    if bytes.len() != 10 || bytes[7] != b'-' {
        return Err(LeafError::CismDateRejected);
    }
    let (year, month) = parse_month(&bytes[..7]).ok_or(LeafError::CismDateRejected)?;
    let day = two_digits(&bytes[8..10]).ok_or(LeafError::CismDateRejected)?;
    if day == 0 || day > month_length(year, month) {
        return Err(LeafError::CismDateRejected);
    }
    let ordinal = day_ordinal(year, month, day);
    let start = ordinal - (ordinal + 6) % 7;
    Ok(WeekPeriod {
        period_start: format_ordinal(start),
        period_end: format_ordinal(start + 6),
    })
}

pub fn cism_month_dates(month: &str) -> Result<Vec<String>, LeafError> {
    let (year, month) = parse_month(month.as_bytes()).ok_or(LeafError::CismMonthRejected)?;
    let first = day_ordinal(year, month, 1);
    let weekday = (first + 6) % 7;
    let mut dates = vec![format_ordinal(first)];
    let mut day = 1 + 7 - weekday;
    while day <= month_length(year, month) {
        dates.push(format_ordinal(first + day - 1));
        day += 7;
    }
    Ok(dates)
}

fn two_digits(bytes: &[u8]) -> Option<i32> {
    if bytes.len() != 2 || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(i32::from(bytes[0] - b'0') * 10 + i32::from(bytes[1] - b'0'))
}

fn parse_month(bytes: &[u8]) -> Option<(i32, i32)> {
    if bytes.len() != 7 || &bytes[..2] != b"20" || bytes[4] != b'-' {
        return None;
    }
    let year = 2000 + two_digits(&bytes[2..4])?;
    let month = two_digits(&bytes[5..7])?;
    (1..=12).contains(&month).then_some((year, month))
}

fn month_length(year: i32, month: i32) -> i32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn day_ordinal(year: i32, month: i32, day: i32) -> i32 {
    let years: i32 = (2000..year)
        .map(|year| {
            if month_length(year, 2) == 29 {
                366
            } else {
                365
            }
        })
        .sum();
    let months: i32 = (1..month).map(|month| month_length(year, month)).sum();
    years + months + day - 1
}

fn format_ordinal(mut ordinal: i32) -> String {
    let mut year = 2000;
    while ordinal < 0 {
        year -= 1;
        ordinal += if month_length(year, 2) == 29 {
            366
        } else {
            365
        };
    }
    loop {
        let year_length = if month_length(year, 2) == 29 {
            366
        } else {
            365
        };
        if ordinal < year_length {
            break;
        }
        ordinal -= year_length;
        year += 1;
    }
    let mut month = 1;
    while ordinal >= month_length(year, month) {
        ordinal -= month_length(year, month);
        month += 1;
    }
    format!("{year:04}-{month:02}-{:02}", ordinal + 1)
}
