use scraper::Html;
use std::fs;
use uc3m_timetable::ical::format_date_time;
use uc3m_timetable::{Result, Timetable, TimetableId, UC3M_TIMEZONE};

#[tokio::test]
async fn parse_timetable() -> Result<()> {
    let id = TimetableId::new(2022, 433, 2, 4, 121, 1, UC3M_TIMEZONE);
    let html = Html::parse_document(&fs::read_to_string("tests/timetable.html")?);
    let timetable = Timetable::parse(id, &html)?;

    let expected = fs::read_to_string("tests/expected.ics")?
        .replace("{DTSTAMP}", &format_date_time(timetable.created_on()));
    assert_eq!(timetable.calendar().to_string(), expected);
    Ok(())
}
