use crate::ical::Calendar;
use crate::parse::Parser;
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use reqwest::{Response, Url};
use scraper::Html;
use std::borrow::Cow;
use std::collections::HashMap;
use std::convert::Into;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::result::Result as StdResult;
use std::str::FromStr;

pub mod ical;
mod parse;
pub(crate) mod util;

// todo: replace by proper error type.
/// A [`Result`](StdResult) alias where the
/// [`Err`] case is [`Box<dyn std::error::Error>`].
pub type Result<T> = StdResult<T, Box<dyn Error>>;

/// The time zone of the UC3M university.
pub const UC3M_TIMEZONE: Tz = chrono_tz::Europe::Madrid;
static UC3M_TIMETABLE_DOMAIN: &str = "aplicaciones.uc3m.es";

/// Identifies a timetable.
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct TimetableId {
    pub year: i32,
    pub plan: u16,
    pub center: u8,
    pub grade: u8,
    pub group: u16,
    pub period: u8,
    time_zone: Tz,
}

impl TimetableId {
    /// Creates a timetable identifier.
    pub fn new(
        year: i32,
        plan: u16,
        center: u8,
        grade: u8,
        group: u16,
        period: u8,
        time_zone: Tz,
    ) -> Self {
        Self {
            year,
            plan,
            center,
            grade,
            group,
            period,
            time_zone,
        }
    }

    /// Returns the [`Url`] where the timetable is located.
    pub fn url(&self) -> Url {
        let url = format!(
            "https://{}/horarios-web/publicacion/{}/porCentroPlanCursoGrupo.tt",
            UC3M_TIMETABLE_DOMAIN, self.year
        );
        let params = [
            ("plan", self.plan.to_string()),
            ("centro", self.center.to_string()),
            ("curso", self.grade.to_string()),
            ("grupo", self.group.to_string()),
            ("tipoPer", "C".into()),
            ("valorPer", self.period.to_string()),
        ];
        Url::parse_with_params(&url, &params).expect("invalid timetable url")
    }
}

/// An error that can occur while parsing the [`Url`] of
/// a [`TimetableId`].
#[derive(Debug)]
pub enum TimetableUrlParseError {
    MissingDomain,
    IncorrectDomain,
    CannotBeABaseUrl,
    MissingYearSegment,
    InvalidYearSegment(std::num::ParseIntError),
    MissingQueryParam(&'static str),
    InvalidQueryParam(&'static str),
}

impl Display for TimetableUrlParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::MissingQueryParam(name) => write!(f, "missing query param `{}`", name),
            Self::InvalidQueryParam(name) => write!(f, "invalid query param `{}`", name),
            _ => write!(
                f,
                "{}",
                match *self {
                    Self::MissingDomain => "url is missing domain",
                    Self::IncorrectDomain => "incorrect timetable domain",
                    Self::CannotBeABaseUrl => "cannot parse cannot-be-a-base url",
                    Self::MissingYearSegment => "url is missing year segment",
                    Self::InvalidYearSegment(_) => "cannot parse non-numeric year segment",
                    _ => unreachable!(),
                }
            ),
        }
    }
}

impl Error for TimetableUrlParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidYearSegment(err) => Some(err),
            _ => None,
        }
    }
}

impl TryFrom<Url> for TimetableId {
    type Error = TimetableUrlParseError;

    fn try_from(url: Url) -> StdResult<Self, Self::Error> {
        if url.domain().ok_or(TimetableUrlParseError::MissingDomain)? != UC3M_TIMETABLE_DOMAIN {
            return Err(TimetableUrlParseError::IncorrectDomain);
        }

        let year = url
            .path_segments()
            .ok_or(TimetableUrlParseError::CannotBeABaseUrl)?
            .skip(2)
            .next()
            .ok_or(TimetableUrlParseError::MissingYearSegment)?
            .parse()
            .map_err(|err| TimetableUrlParseError::InvalidYearSegment(err))?;
        let query_params: HashMap<_, _> = url.query_pairs().into_iter().collect();

        fn parse_query_param<F: FromStr>(
            query_params: &HashMap<Cow<str>, Cow<str>>,
            name: &'static str,
        ) -> StdResult<F, TimetableUrlParseError> {
            query_params
                .get(name)
                .ok_or_else(|| TimetableUrlParseError::MissingQueryParam(name))?
                .parse()
                .map_err(|_| TimetableUrlParseError::InvalidQueryParam(name))
        }

        Ok(TimetableId::new(
            year,
            parse_query_param(&query_params, "plan")?,
            parse_query_param(&query_params, "centro")?,
            parse_query_param(&query_params, "curso")?,
            parse_query_param(&query_params, "grupo")?,
            parse_query_param(&query_params, "valorPer")?,
            UC3M_TIMEZONE,
        ))
    }
}

/// A UC3M timetable.
pub struct Timetable {
    id: TimetableId,
    calendar: Calendar,
    created_on: DateTime<Tz>,
}

impl Timetable {
    /// Fetches and parses the timetable with the given ID.
    pub async fn fetch(id: TimetableId) -> Result<Self> {
        let response = reqwest::get(id.url()).await?;
        let html = parse_response(response).await?;
        Self::parse(id, &html)
    }

    /// Parses the timetable with the given ID.
    pub fn parse(id: TimetableId, html: &Html) -> Result<Self> {
        let created_on = Utc::now().with_timezone(&id.time_zone);
        let calendar = Parser::new(&id, html, &created_on).parse()?;
        Ok(Self {
            id,
            calendar,
            created_on,
        })
    }

    /// Returns the timetable identifier.
    pub const fn id(&self) -> &TimetableId {
        &self.id
    }

    /// Returns the timetable contents as an iCalendar object.
    pub const fn calendar(&self) -> &Calendar {
        &self.calendar
    }

    pub const fn created_on(&self) -> &DateTime<Tz> {
        &self.created_on
    }
}

#[allow(unused_mut)]
async fn parse_response(mut response: Response) -> Result<Html> {
    #[cfg(target_arch = "wasm32")]
    {
        // reqwest doesn't support accessing the raw contents of
        // a `Request` yet; see https://github.com/seanmonstar/reqwest/issues/655.
        Ok(Html::parse_document(&response.text().await?))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use html5ever::tendril::{StrTendril, TendrilSink};
        use html5ever::{driver, ParseOpts};

        // Stream body contents to HTML parser.
        let mut parser = driver::parse_document(Html::new_document(), ParseOpts::default());
        while let Some(chunk) = response.chunk().await? {
            let tendril = StrTendril::try_from_byte_slice(&chunk)
                .map_err(|_| "got invalid utf-8 encoded string")?;
            parser.process(tendril);
        }
        Ok(parser.finish())
    }
}

#[cfg(test)]
mod tests {
    use crate::{TimetableId, Url, UC3M_TIMEZONE};

    #[test]
    fn timetable_to_url() {
        let timetable = TimetableId::new(2022, 433, 2, 4, 121, 1, UC3M_TIMEZONE);
        assert_eq!(timetable.url().to_string(), "https://aplicaciones.uc3m.es/horarios-web/publicacion/2022/porCentroPlanCursoGrupo.tt?plan=433&centro=2&curso=4&grupo=121&tipoPer=C&valorPer=1");
    }

    #[test]
    fn url_to_timetable() {
        let url = Url::parse("https://aplicaciones.uc3m.es/horarios-web/publicacion/2022/porCentroPlanCursoGrupo.tt?plan=433&centro=2&curso=4&grupo=121&tipoPer=C&valorPer=1").unwrap();
        assert_eq!(
            TimetableId::try_from(url).unwrap(),
            TimetableId::new(2022, 433, 2, 4, 121, 1, UC3M_TIMEZONE)
        );
    }
}
