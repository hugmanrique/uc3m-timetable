use crate::ical::components::{Event, Recurrence, TimeUnit};
use crate::util::process;
use crate::{Calendar, TimetableId};
use chrono::{Date, DateTime, Duration, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use itertools::Itertools;
use once_cell::sync::Lazy;
use scraper::{ElementRef, Html, Selector};
use selectors::attr::CaseSensitivity;
use std::error::Error;
use std::fmt::{Display, Formatter};

static PRODUCT_NAME: &str = "uc3m-timetable.hugmanrique.me";
static SPEC_VERSION: &str = "2.0";

macro_rules! selector {
    ($selector:expr) => {
        Lazy::new(|| Selector::parse($selector).unwrap())
    };
}

static TIMETABLE_SELECTOR: Lazy<Selector> = selector!(".timetable>tbody");
static TIME_SELECTOR: Lazy<Selector> = selector!(".cabeceraHora");
static GROUP_SELECTOR: Lazy<Selector> = selector!(".asignaturaGrupo");
static SESSION_SELECTOR: Lazy<Selector> = selector!(".fechasSesion");

#[derive(Debug)]
pub struct Parser<'a> {
    time_table: &'a TimetableId,
    input: &'a Html,
    created_on: &'a DateTime<Tz>,
}

impl<'a> Parser<'a> {
    /// Creates a parser for interpreting the given input.
    pub fn new(time_table: &'a TimetableId, input: &'a Html, created_on: &'a DateTime<Tz>) -> Self {
        Self {
            time_table,
            input,
            created_on,
        }
    }

    /// Parses the input as an iCalendar object.
    pub fn parse(&self) -> Result<Calendar, ParseError> {
        let table_body = self
            .input
            .select(&TIMETABLE_SELECTOR)
            .next()
            .ok_or(ParseError::MissingTbodyElem)?;
        let row_elems = table_body.children().filter_map(ElementRef::wrap);

        let mut events = Vec::with_capacity(10); // most days have 2 sessions
        for row_elem in row_elems {
            self.parse_row(row_elem, &mut events)?;
        }

        let components = events.into_iter().map(Into::into).collect();
        Ok(Calendar::new(PRODUCT_NAME, SPEC_VERSION, components))
    }

    fn parse_row(&self, row_elem: ElementRef, dest: &mut Vec<Event>) -> Result<(), ParseError> {
        fn get_time_text(elem: ElementRef) -> Result<u32, ParseError> {
            elem.first_child()
                .ok_or(ParseError::ChildlessTimeElement)?
                .value()
                .as_text()
                .ok_or(ParseError::NonTextualTimeNode)?
                .parse()
                .map_err(ParseError::NonNumericTimeValue)
        }

        let time_elem = row_elem
            .select(&TIME_SELECTOR)
            .next()
            .ok_or(ParseError::MissingRowTimeCell)?;
        let hour = get_time_text(time_elem)?;

        // The minutes are wrapped in a <sup> element
        let minutes_elem = ElementRef::wrap(time_elem.last_child().unwrap())
            .ok_or(ParseError::NonElementMinutesNode)?;
        let minutes = get_time_text(minutes_elem)?;

        let start_time = NaiveTime::from_hms(hour, minutes, 0);
        let cell_elems = row_elem
            .children()
            .filter_map(ElementRef::wrap)
            .filter(|cell_elem| {
                cell_elem
                    .value()
                    .has_class("celdaConSesion", CaseSensitivity::CaseSensitive)
            });
        let (cells, result) = process(cell_elems.map(|elem| Cell::new(self, start_time, elem)));
        for cell in cells {
            cell.push_sessions(dest)?;
        }

        let result = result.borrow().clone();
        result
    }
}

#[derive(Debug, Clone)]
pub enum ParseError {
    MissingTbodyElem,
    MissingRowTimeCell,
    ChildlessTimeElement,
    NonTextualTimeNode,
    NonElementMinutesNode,
    NonNumericTimeValue(std::num::ParseIntError),
    InvalidRowSpan(std::num::ParseIntError),
    MissingGroupElem,
    ChildlessGroupElem,
    NonTextualGroupChild,
    MissingSessionsElem,
    NonElementSessionDateNode,
    NonElementSessionLocationNode,
    MissingDateRange,
    NonTextualDateRange,
    MissingLocationSpan,
    NonTextualLocationSpan,
    InvalidStartDate,
    InvalidEndDate,
    InvalidDateFormat,
    InvalidDay(std::num::ParseIntError),
    InvalidMonth,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                ParseError::MissingTbodyElem => "cannot find the time table `tbody` element",
                ParseError::MissingRowTimeCell =>
                    "cannot find the `hh:mm` cell of the time table row",
                ParseError::ChildlessTimeElement => "time element has no children",
                ParseError::NonTextualTimeNode =>
                    "first child of the time element is not a textual node",
                ParseError::NonElementMinutesNode =>
                    "last child of the time cell is not an element",
                ParseError::NonNumericTimeValue(_) => "time cell has a non-numeric time value",
                ParseError::InvalidRowSpan(_) => "element has an invalid `rowspan` attribute value",
                ParseError::MissingGroupElem =>
                    "cannot find the subject group element of cell element",
                ParseError::ChildlessGroupElem => "cell group element has no children",
                ParseError::NonTextualGroupChild =>
                    "first child of the subject group element is not a textual node",
                ParseError::MissingSessionsElem =>
                    "cannot find the sessions element of subject group element",
                ParseError::NonElementSessionDateNode => "session date node is not an element",
                ParseError::NonElementSessionLocationNode =>
                    "session location node is not an element",
                ParseError::MissingDateRange => "session within a cell is missing date range",
                ParseError::NonTextualDateRange =>
                    "first child of date range element is not a textual node",
                ParseError::MissingLocationSpan => "cannot find the location span of session",
                ParseError::NonTextualLocationSpan =>
                    "location span of a session is not a textual node",
                ParseError::InvalidStartDate => "start date of session is invalid",
                ParseError::InvalidEndDate => "end date of session is invalid",
                ParseError::InvalidDateFormat =>
                    "formatted date does not follow the `dd.month` format",
                ParseError::InvalidDay(_) => "invalid day value",
                ParseError::InvalidMonth => "invalid month value",
            }
        )
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ParseError::NonNumericTimeValue(err)
            | ParseError::InvalidRowSpan(err)
            | ParseError::InvalidDay(err) => Some(err),
            _ => None,
        }
    }
}

struct Cell<'a> {
    parser: &'a Parser<'a>,
    /*time_table: &'a TimetableId,
    created_on: &'a DateTime<Tz>,*/
    start_time: NaiveTime,
    duration: Duration,
    group_elem: ElementRef<'a>,
}

impl<'a> Cell<'a> {
    fn new(
        parser: &'a Parser<'a>,
        start_time: NaiveTime,
        elem: ElementRef<'a>,
    ) -> Result<Self, ParseError> {
        let duration = Duration::minutes(15)
            * elem
                .value()
                .attr("rowspan")
                .map_or(Ok(1), |span| span.parse())
                .map_err(ParseError::InvalidRowSpan)?;

        let group_elem = elem
            .select(&GROUP_SELECTOR)
            .next()
            .ok_or(ParseError::MissingGroupElem)?;

        Ok(Self {
            parser,
            start_time,
            duration,
            group_elem,
        })
    }

    fn push_sessions(&self, dest: &mut Vec<Event>) -> Result<(), ParseError> {
        let course_name = &self
            .group_elem
            .first_child()
            .ok_or(ParseError::ChildlessGroupElem)?
            .value()
            .as_text()
            .ok_or(ParseError::NonTextualGroupChild)?
            .text;

        let session_elems = self
            .group_elem
            .select(&SESSION_SELECTOR)
            .next()
            .ok_or(ParseError::MissingSessionsElem)?
            .children();

        let (events, result) = process(session_elems.tuples().map(
            |(date_range_span, location_span, _)| {
                let date_range_span = ElementRef::wrap(date_range_span)
                    .ok_or(ParseError::NonElementSessionDateNode)?;
                let location_span = ElementRef::wrap(location_span)
                    .ok_or(ParseError::NonElementSessionLocationNode)?;
                self.parse_session(date_range_span, location_span, course_name)
            },
        ));
        dest.extend(events);

        let result = result.borrow().clone();
        result
    }

    fn parse_session(
        &self,
        date_range_span: ElementRef,
        location_span: ElementRef,
        course_name: &str,
    ) -> Result<Event, ParseError> {
        let raw_range = date_range_span
            .first_child()
            .ok_or(ParseError::MissingDateRange)?
            .value()
            .as_text()
            .ok_or(ParseError::NonTextualDateRange)?
            .trim_end_matches(':');
        let (start_date, end_date) = self.parse_date_range(raw_range)?;
        let location = location_span
            .first_child()
            .ok_or(ParseError::MissingLocationSpan)?
            .value()
            .as_text()
            .ok_or(ParseError::NonTextualLocationSpan)?;

        let start_datetime = start_date
            .and_time(self.start_time)
            .ok_or(ParseError::InvalidStartDate)?;

        let uid = format!("{}-{}@{}", course_name, raw_range, PRODUCT_NAME);
        let event = Event::new(uid, *self.parser.created_on, start_datetime)
            .summary(course_name)
            .location(location.to_string())
            .duration(self.duration);

        Ok(if start_date == end_date {
            event
        } else {
            let end_datetime = end_date
                .and_time(self.start_time)
                .ok_or(ParseError::InvalidEndDate)?;
            event.recurrence(Recurrence::until(TimeUnit::Week, end_datetime))
        })
    }

    fn parse_date_range(&self, range: &str) -> Result<(Date<Tz>, Date<Tz>), ParseError> {
        // If the string doesn't contain a dash, return an empty single-day range.
        Ok(match range.split_once('-') {
            Some((start, end)) => (self.parse_date(start)?, self.parse_date(end)?),
            None => {
                let date = self.parse_date(range)?;
                (date, date)
            }
        })
    }

    fn parse_date(&self, date: &str) -> Result<Date<Tz>, ParseError> {
        let (day, month) = date.split_once('.').ok_or(ParseError::InvalidDateFormat)?;
        let day = day.parse().map_err(ParseError::InvalidDay)?;
        let month = match month {
            "ene" => 1,
            "feb" => 2,
            "mar" => 3,
            "abr" => 4,
            "may" => 5,
            "jun" => 6,
            "jul" => 7,
            "ago" => 8,
            "sep" => 9,
            "oct" => 10,
            "nov" => 11,
            "dic" => 12,
            _ => return Err(ParseError::InvalidMonth),
        };

        Ok(Utc
            .ymd(self.time_table_id().year, month, day)
            .with_timezone(self.time_zone()))
    }

    const fn time_table_id(&self) -> &TimetableId {
        self.parser.time_table
    }

    const fn time_zone(&self) -> &Tz {
        &self.time_table_id().time_zone
    }
}
