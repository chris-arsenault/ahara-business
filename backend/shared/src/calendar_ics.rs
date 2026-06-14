use time::format_description::well_known::Rfc3339;
use time::{Date, Duration, Month, OffsetDateTime, PrimitiveDateTime, Time};

use crate::calendar_types::CalendarEventStatus;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedIcsEvent {
    pub(crate) title: Option<String>,
    pub(crate) starts_at: String,
    pub(crate) ends_at: String,
    pub(crate) timezone: String,
    pub(crate) location: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) status: Option<CalendarEventStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IcsProperty {
    name: String,
    params: Vec<(String, String)>,
    value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IcsDateTime {
    value: OffsetDateTime,
    all_day: bool,
}

pub(crate) fn parse_ics_event(bytes: &[u8]) -> AppResult<Option<ParsedIcsEvent>> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| AppError::Validation("calendar invite must be UTF-8".to_string()))?;
    let Some(properties) = first_event_properties(text) else {
        return Ok(None);
    };
    let Some(start_property) = property(&properties, "DTSTART") else {
        return Ok(None);
    };

    let start = parse_ics_datetime(start_property, "DTSTART")?;
    let end = event_end(&properties, start)?;
    Ok(Some(ParsedIcsEvent {
        title: text_property(&properties, "SUMMARY"),
        starts_at: format_rfc3339(start.value)?,
        ends_at: format_rfc3339(end.value)?,
        timezone: timezone(start_property).unwrap_or_else(|| "UTC".to_string()),
        location: text_property(&properties, "LOCATION"),
        description: text_property(&properties, "DESCRIPTION"),
        status: status_property(&properties),
    }))
}

fn first_event_properties(text: &str) -> Option<Vec<IcsProperty>> {
    let mut event = Vec::new();
    let mut in_event = false;
    for line in unfolded_lines(text) {
        let Some(property) = parse_property(&line) else {
            continue;
        };
        if property.name == "BEGIN" && property.value.eq_ignore_ascii_case("VEVENT") {
            in_event = true;
            event.clear();
            continue;
        }
        if property.name == "END" && property.value.eq_ignore_ascii_case("VEVENT") {
            return in_event.then_some(event);
        }
        if in_event {
            event.push(property);
        }
    }
    None
}

fn unfolded_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw in text.replace("\r\n", "\n").replace('\r', "\n").split('\n') {
        if raw.starts_with(' ') || raw.starts_with('\t') {
            if let Some(current) = lines.last_mut() {
                current.push_str(raw.trim_start_matches([' ', '\t']));
            }
            continue;
        }
        lines.push(raw.to_string());
    }
    lines
}

fn parse_property(line: &str) -> Option<IcsProperty> {
    let (head, value) = line.split_once(':')?;
    let mut parts = head.split(';');
    let name = parts.next()?.trim().to_ascii_uppercase();
    let params = parts
        .filter_map(|part| {
            let (key, value) = part.split_once('=')?;
            Some((
                key.trim().to_ascii_uppercase(),
                value.trim().trim_matches('"').to_string(),
            ))
        })
        .collect();
    Some(IcsProperty {
        name,
        params,
        value: value.trim().to_string(),
    })
}

fn property<'a>(properties: &'a [IcsProperty], name: &str) -> Option<&'a IcsProperty> {
    properties.iter().find(|property| property.name == name)
}

fn text_property(properties: &[IcsProperty], name: &str) -> Option<String> {
    property(properties, name)
        .map(|property| unescape_text(&property.value))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn timezone(property: &IcsProperty) -> Option<String> {
    param(property, "TZID")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn param<'a>(property: &'a IcsProperty, key: &str) -> Option<&'a str> {
    property
        .params
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

fn event_end(properties: &[IcsProperty], start: IcsDateTime) -> AppResult<IcsDateTime> {
    if let Some(end_property) = property(properties, "DTEND") {
        return parse_ics_datetime(end_property, "DTEND");
    }
    if let Some(duration_property) = property(properties, "DURATION")
        && let Some(duration) = parse_duration(&duration_property.value)?
    {
        return Ok(IcsDateTime {
            value: start.value + duration,
            all_day: start.all_day,
        });
    }
    Ok(IcsDateTime {
        value: start.value + default_duration(start),
        all_day: start.all_day,
    })
}

fn default_duration(start: IcsDateTime) -> Duration {
    if start.all_day {
        Duration::days(1)
    } else {
        Duration::hours(1)
    }
}

fn parse_ics_datetime(property: &IcsProperty, label: &str) -> AppResult<IcsDateTime> {
    let value = property.value.trim();
    let value_kind = param(property, "VALUE").unwrap_or_default();
    if value_kind.eq_ignore_ascii_case("DATE") || is_compact_date(value) {
        return compact_date(value, label).map(|date| IcsDateTime {
            value: PrimitiveDateTime::new(date, Time::MIDNIGHT).assume_utc(),
            all_day: true,
        });
    }
    compact_datetime(value, label).map(|value| IcsDateTime {
        value,
        all_day: false,
    })
}

fn is_compact_date(value: &str) -> bool {
    value.len() == 8 && value.chars().all(|ch| ch.is_ascii_digit())
}

fn compact_date(value: &str, label: &str) -> AppResult<Date> {
    if !is_compact_date(value) {
        return Err(invalid_datetime(label));
    }
    let year = parse_i32(&value[0..4], label)?;
    let month =
        Month::try_from(parse_u8(&value[4..6], label)?).map_err(|_| invalid_datetime(label))?;
    let day = parse_u8(&value[6..8], label)?;
    Date::from_calendar_date(year, month, day).map_err(|_| invalid_datetime(label))
}

fn compact_datetime(value: &str, label: &str) -> AppResult<OffsetDateTime> {
    let value = value.strip_suffix('Z').unwrap_or(value);
    let (date, time) = value
        .split_once('T')
        .ok_or_else(|| invalid_datetime(label))?;
    let date = compact_date(date, label)?;
    let time = compact_time(time, label)?;
    Ok(PrimitiveDateTime::new(date, time).assume_utc())
}

fn compact_time(value: &str, label: &str) -> AppResult<Time> {
    if value.len() < 6 || !value[..6].chars().all(|ch| ch.is_ascii_digit()) {
        return Err(invalid_datetime(label));
    }
    Time::from_hms(
        parse_u8(&value[0..2], label)?,
        parse_u8(&value[2..4], label)?,
        parse_u8(&value[4..6], label)?,
    )
    .map_err(|_| invalid_datetime(label))
}

fn parse_duration(value: &str) -> AppResult<Option<Duration>> {
    let value = value.trim();
    if !value.starts_with('P') {
        return Ok(None);
    }

    let mut total = Duration::ZERO;
    let mut digits = String::new();
    let mut in_time = false;
    for ch in value.chars().skip(1) {
        if ch == 'T' {
            in_time = true;
        } else if ch.is_ascii_digit() {
            digits.push(ch);
        } else {
            let amount = take_duration_amount(&mut digits)?;
            total += duration_component(ch, amount, in_time)?;
        }
    }
    if !digits.is_empty() {
        return Err(AppError::Validation(
            "calendar invite duration is invalid".to_string(),
        ));
    }
    Ok(Some(total))
}

fn take_duration_amount(digits: &mut String) -> AppResult<i64> {
    if digits.is_empty() {
        return Err(AppError::Validation(
            "calendar invite duration is invalid".to_string(),
        ));
    }
    let amount = digits
        .parse::<i64>()
        .map_err(|_| AppError::Validation("calendar invite duration is invalid".to_string()))?;
    digits.clear();
    Ok(amount)
}

fn duration_component(ch: char, amount: i64, in_time: bool) -> AppResult<Duration> {
    match (ch, in_time) {
        ('W', false) => Ok(Duration::weeks(amount)),
        ('D', false) => Ok(Duration::days(amount)),
        ('H', true) => Ok(Duration::hours(amount)),
        ('M', true) => Ok(Duration::minutes(amount)),
        ('S', true) => Ok(Duration::seconds(amount)),
        _ => Err(AppError::Validation(
            "calendar invite duration is invalid".to_string(),
        )),
    }
}

fn parse_i32(value: &str, label: &str) -> AppResult<i32> {
    value.parse().map_err(|_| invalid_datetime(label))
}

fn parse_u8(value: &str, label: &str) -> AppResult<u8> {
    value.parse().map_err(|_| invalid_datetime(label))
}

fn format_rfc3339(value: OffsetDateTime) -> AppResult<String> {
    value
        .format(&Rfc3339)
        .map_err(|err| AppError::Internal(err.to_string()))
}

fn status_property(properties: &[IcsProperty]) -> Option<CalendarEventStatus> {
    match text_property(properties, "STATUS")?
        .to_ascii_uppercase()
        .as_str()
    {
        "CONFIRMED" => Some(CalendarEventStatus::Confirmed),
        "CANCELLED" | "CANCELED" => Some(CalendarEventStatus::Canceled),
        "TENTATIVE" => Some(CalendarEventStatus::Tentative),
        _ => None,
    }
}

fn unescape_text(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => output.push('\n'),
            Some('\\') => output.push('\\'),
            Some(',') => output.push(','),
            Some(';') => output.push(';'),
            Some(other) => output.push(other),
            None => output.push('\\'),
        }
    }
    output
}

fn invalid_datetime(label: &str) -> AppError {
    AppError::Validation(format!("calendar invite {label} is invalid"))
}

#[cfg(test)]
mod tests {
    use super::parse_ics_event;
    use crate::calendar_types::CalendarEventStatus;

    #[test]
    fn parses_basic_vevent_fields() {
        let event = parse_ics_event(
            br#"BEGIN:VCALENDAR
BEGIN:VEVENT
SUMMARY:Intro call
DTSTART:20260615T140000Z
DTEND:20260615T143000Z
LOCATION:Zoom
DESCRIPTION:Bring notes\, links\; and questions
STATUS:CONFIRMED
END:VEVENT
END:VCALENDAR"#,
        )
        .unwrap()
        .unwrap();

        assert_eq!(event.title.as_deref(), Some("Intro call"));
        assert_eq!(event.starts_at, "2026-06-15T14:00:00Z");
        assert_eq!(event.ends_at, "2026-06-15T14:30:00Z");
        assert_eq!(event.location.as_deref(), Some("Zoom"));
        assert_eq!(
            event.description.as_deref(),
            Some("Bring notes, links; and questions")
        );
        assert_eq!(event.status, Some(CalendarEventStatus::Confirmed));
    }

    #[test]
    fn parses_folded_lines_and_tzid() {
        let event = parse_ics_event(
            br#"BEGIN:VCALENDAR
BEGIN:VEVENT
SUMMARY:Long
 title
DTSTART;TZID=America/New_York:20260615T100000
DURATION:PT45M
END:VEVENT
END:VCALENDAR"#,
        )
        .unwrap()
        .unwrap();

        assert_eq!(event.title.as_deref(), Some("Longtitle"));
        assert_eq!(event.starts_at, "2026-06-15T10:00:00Z");
        assert_eq!(event.ends_at, "2026-06-15T10:45:00Z");
        assert_eq!(event.timezone, "America/New_York");
    }

    #[test]
    fn defaults_all_day_events_to_one_day() {
        let event = parse_ics_event(
            br#"BEGIN:VCALENDAR
BEGIN:VEVENT
SUMMARY:Hold
DTSTART;VALUE=DATE:20260615
END:VEVENT
END:VCALENDAR"#,
        )
        .unwrap()
        .unwrap();

        assert_eq!(event.starts_at, "2026-06-15T00:00:00Z");
        assert_eq!(event.ends_at, "2026-06-16T00:00:00Z");
    }

    #[test]
    fn returns_none_without_vevent() {
        assert_eq!(
            parse_ics_event(b"BEGIN:VCALENDAR\nEND:VCALENDAR").unwrap(),
            None
        );
    }
}
