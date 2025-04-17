use nom::{
    branch::alt,
    character::complete::{self, char},
    combinator::{map, opt, verify},
    multi::separated_list1,
    sequence::{preceded, separated_pair, terminated},
    IResult, Parser,
};

use graphile_worker_crontab_types::{CrontabTimer, CrontabValue};

#[derive(Debug, PartialEq, Eq)]
enum CrontabPart {
    Minute,
    Hours,
    Days,
    Months,
    DaysOfWeek,
}

impl CrontabPart {
    fn boundaries(&self) -> (u32, u32) {
        match self {
            CrontabPart::Minute => (0, 59),
            CrontabPart::Hours => (0, 23),
            CrontabPart::Days => (1, 31),
            CrontabPart::Months => (1, 12),
            CrontabPart::DaysOfWeek => (0, 6),
        }
    }
}

/// Attempts to parse a number with crontab part boundaries
fn crontab_number<'a>(part: &CrontabPart) -> impl Fn(&'a str) -> IResult<&'a str, u32> {
    let (min, max) = part.boundaries();
    move |input| verify(complete::u32, |v| v >= &min && v <= &max).parse(input)
}

/// Attempts to parse a range with crontab part boundaries
fn crontab_range<'a, 'p>(
    part: &'p CrontabPart,
) -> impl Fn(&'a str) -> IResult<&'a str, (u32, u32)> + 'p {
    |input| {
        verify(
            separated_pair(crontab_number(part), char('-'), crontab_number(part)),
            |(left, right)| left < right,
        )
        .parse(input)
    }
}

/// Attempts to parse a step with crontab part boundaries
fn crontab_wildcard<'a, 'p>(
    part: &'p CrontabPart,
) -> impl Fn(&'a str) -> IResult<&'a str, Option<u32>> + 'p {
    |input| preceded(char('*'), opt(preceded(char('/'), crontab_number(part)))).parse(input)
}

/// Attempts to parse a crontab part
fn crontab_value<'a, 'p>(
    part: &'p CrontabPart,
) -> impl Fn(&'a str) -> IResult<&'a str, CrontabValue> + 'p {
    |input| {
        alt((
            map(crontab_range(part), |(left, right)| {
                CrontabValue::Range(left, right)
            }),
            map(crontab_wildcard(part), |divider| match divider {
                Some(d) => CrontabValue::Step(d),
                None => CrontabValue::Any,
            }),
            map(crontab_number(part), CrontabValue::Number),
        ))
        .parse(input)
    }
}

/// Attempts to parse comma separated crontab values
fn crontab_values<'a, 'p>(
    part: &'p CrontabPart,
) -> impl Fn(&'a str) -> IResult<&'a str, Vec<CrontabValue>> + 'p {
    |input| separated_list1(char(','), crontab_value(part)).parse(input)
}

/// Parse all 5 crontab values
pub(crate) fn nom_crontab_timer(input: &str) -> IResult<&str, CrontabTimer> {
    let (input, minutes) =
        terminated(crontab_values(&CrontabPart::Minute), char(' ')).parse(input)?;
    let (input, hours) = terminated(crontab_values(&CrontabPart::Hours), char(' ')).parse(input)?;
    let (input, days) = terminated(crontab_values(&CrontabPart::Days), char(' ')).parse(input)?;
    let (input, months) =
        terminated(crontab_values(&CrontabPart::Months), char(' ')).parse(input)?;
    let (input, dows) = crontab_values(&CrontabPart::DaysOfWeek).parse(input)?;

    Ok((
        input,
        CrontabTimer {
            minutes,
            hours,
            days,
            months,
            dows,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crontab_timer_test_all_wildcard() {
        assert_eq!(
            Ok((
                " foo",
                CrontabTimer {
                    minutes: vec![CrontabValue::Any],
                    hours: vec![CrontabValue::Any],
                    days: vec![CrontabValue::Any],
                    months: vec![CrontabValue::Any],
                    dows: vec![CrontabValue::Any],
                }
            )),
            nom_crontab_timer("* * * * * foo"),
        );
    }

    #[test]
    fn crontab_timer_test_complex_comma_separated_list() {
        assert_eq!(
            Ok((
                " bar",
                CrontabTimer {
                    minutes: vec![
                        CrontabValue::Step(7),
                        CrontabValue::Number(8),
                        CrontabValue::Range(30, 35)
                    ],
                    hours: vec![CrontabValue::Any],
                    days: vec![CrontabValue::Number(3), CrontabValue::Step(4)],
                    months: vec![CrontabValue::Any],
                    dows: vec![CrontabValue::Any, CrontabValue::Number(4)],
                }
            )),
            nom_crontab_timer("*/7,8,30-35 * 3,*/4 * *,4 bar"),
        );
    }

    #[test]
    fn crontab_timer_test_error() {
        let timer_result = nom_crontab_timer("*/7!,8,30-35 * 3,*/4 * *,4 bar");
        assert!(timer_result.is_err());
    }
}
