use crate::ical::{format_date_time, Component, Prop};
use chrono::{DateTime, Duration};
use chrono_tz::Tz;
use std::fmt::{Display, Formatter};
use std::num::NonZeroU32;
use std::slice;

/// A scheduled amount of time on a calendar.
#[derive(Debug, Eq, PartialEq)]
pub struct Event {
    uid: String,
    last_modified: DateTime<Tz>,
    start: DateTime<Tz>,
    created_on: Option<DateTime<Tz>>,
    summary: Option<String>,
    description: Option<String>,
    location: Option<String>,
    recurrence: Option<Recurrence>,
    // The following two properties are mutually exclusive
    end: Option<DateTime<Tz>>,
    duration: Option<Duration>,
}

impl Event {
    /// Creates a new event, where `uid` is the persistent, globally
    /// unique identifier for the event, `last_modified` is the date
    /// and time when the information associated with the event was
    /// last modified at, and `start` specifies when the event begins.
    pub fn new<U: Into<String>>(uid: U, last_modified: DateTime<Tz>, start: DateTime<Tz>) -> Self {
        Self {
            uid: uid.into(),
            last_modified,
            start,
            created_on: None,
            summary: None,
            description: None,
            location: None,
            recurrence: None,
            end: None,
            duration: None,
        }
    }

    /// Defines the date and time when the event information was
    /// created by the user agent.
    pub fn created_on(mut self, created_on: DateTime<Tz>) -> Self {
        self.created_on = Some(created_on);
        self
    }

    /// Defines a short summary or subject for the activity.
    pub fn summary<S: Into<String>>(mut self, summary: S) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Defines a textual description associated with the activity.
    pub fn description<D: Into<String>>(mut self, description: D) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Defines the intended venue for the activity.
    pub fn location<L: Into<String>>(mut self, location: L) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Defines the recurrence rule for the event.
    pub fn recurrence(mut self, recurrence: Recurrence) -> Self {
        self.recurrence = Some(recurrence);
        self
    }

    /// Defines the date and time by which the event ends.
    pub fn end(mut self, end: DateTime<Tz>) -> Self {
        assert!(
            self.duration.is_none(),
            "cannot set end datetime of event with duration"
        );
        self.end = Some(end);
        self
    }

    /// Defines the positive duration of the event.
    pub fn duration(mut self, duration: Duration) -> Self {
        assert!(
            self.end.is_none(),
            "cannot set duration of event with end datetime"
        );
        assert!(
            duration > Duration::zero(),
            "event duration must be positive; got {}",
            duration
        );
        self.duration = Some(duration);
        self
    }
}

impl From<Event> for Component {
    fn from(event: Event) -> Self {
        let mut props = vec![
            Prop::date_time("DTSTAMP", &event.last_modified),
            Prop::text("UID", slice::from_ref(&event.uid)),
            Prop::date_time("DTSTART", &event.start),
        ];
        props.extend(
            [
                event
                    .created_on
                    .map(|created_on| Prop::date_time("CREATED", &created_on)),
                event.summary.map(|summary| Prop::new("SUMMARY", summary)),
                event.description.map(|desc| Prop::new("DESCRIPTION", desc)),
                event
                    .location
                    .map(|location| Prop::new("LOCATION", location)),
                event.end.map(|end| Prop::date_time("DTEND", &end)),
                // The ISO 8601 duration format is compatible with RFC 5545, except
                // for the year and week designators, which chrono doesn't use.
                event
                    .duration
                    .map(|duration| Prop::new("DURATION", duration.to_string())),
                event
                    .recurrence
                    .map(|rrule| Prop::new("RRULE", rrule.to_string())),
            ]
            .into_iter()
            .flatten(),
        );
        Component::new("VEVENT", props)
    }
}

/// A recurrence rule specification.
#[derive(Debug, Eq, PartialEq)]
pub struct Recurrence {
    frequency: TimeUnit,
    until: Option<DateTime<Tz>>,
    count: Option<NonZeroU32>,
    interval: Option<NonZeroU32>,
}

impl Recurrence {
    /// Creates a recurrence rule that repeats with the specified
    /// frequency until the given date and time (inclusive).
    pub fn until(frequency: TimeUnit, until: DateTime<Tz>) -> Self {
        Self {
            frequency,
            until: Some(until),
            count: None,
            interval: None,
        }
    }

    /// Creates a recurrence rule that repeats with the specified
    /// frequency `count` times.
    ///
    /// The `start` of an [`Event`] counts as the first occurrence.
    pub fn times(frequency: TimeUnit, count: u32) -> Self {
        Self {
            frequency,
            until: None,
            count: Some(NonZeroU32::new(count).expect("recurrence count must be positive")),
            interval: None,
        }
    }

    /// Sets the interval at which the recurrence rule repeats.
    ///
    /// For example, within a rule with daily frequency, a value
    /// of `8` means the event occurs every eight days.
    pub fn interval(&mut self, interval: u32) {
        self.interval =
            Some(NonZeroU32::new(interval).expect("recurrence interval must be positive"));
    }
}

impl Display for Recurrence {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "FREQ={}", self.frequency.recurrence_freq())?;
        if let Some(until) = &self.until {
            // The UNTIL parameter must be specified in UTC time.
            let utc = until.with_timezone(&Tz::UTC);
            write!(f, ";UNTIL={}", format_date_time(&utc))?;
        } else {
            write!(f, ";COUNT={}", self.count.unwrap())?;
        }
        if let Some(interval) = &self.interval {
            write!(f, ";INTERVAL={}", interval)?;
        }
        Ok(())
    }
}

/// Named intervals of time.
// chrono doesn't provide this enum :(
#[derive(Debug, Eq, PartialEq)]
pub enum TimeUnit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

impl TimeUnit {
    /// Returns the raw frequency value used to format
    /// a [`Recurrence`].
    pub const fn recurrence_freq(&self) -> &'static str {
        match *self {
            TimeUnit::Second => "SECONDLY",
            TimeUnit::Minute => "MINUTELY",
            TimeUnit::Hour => "HOURLY",
            TimeUnit::Day => "DAILY",
            TimeUnit::Week => "WEEKLY",
            TimeUnit::Month => "MONTHLY",
            TimeUnit::Year => "YEARLY",
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ical::components::{Event, Recurrence, TimeUnit};
    use crate::ical::{Component, PropHolder};
    use crate::UC3M_TIMEZONE;
    use chrono::{Duration, TimeZone, Utc};

    #[test]
    fn event_builder() {
        let last_modified = Utc::now().with_timezone(&UC3M_TIMEZONE);
        let start = last_modified.clone() + Duration::days(3);
        let event = Event::new("1234", last_modified, start)
            .summary("Important Meeting")
            .description("A very important meeting.")
            .location("Room 101")
            .end(start + Duration::minutes(30));
        let component = Component::from(event);
        assert_eq!(component.first_prop("UID").unwrap().value, "1234");
        assert_eq!(
            component.first_prop("SUMMARY").unwrap().value,
            "Important Meeting"
        );
        assert_eq!(
            component.first_prop("DESCRIPTION").unwrap().value,
            "A very important meeting."
        );
        assert_eq!(component.first_prop("LOCATION").unwrap().value, "Room 101");
        assert!(component.has_prop("DTSTAMP"));
        assert!(component.has_prop("DTSTART"));
        assert!(component.has_prop("DTEND"));
        assert!(!component.has_prop("DURATION"));
    }

    #[test]
    #[should_panic]
    fn end_and_duration() {
        let now = Utc::now().with_timezone(&UC3M_TIMEZONE);
        Event::new("test", now.clone(), now.clone())
            .end(now + Duration::hours(2))
            .duration(Duration::hours(2));
    }

    #[test]
    #[should_panic]
    fn duration_and_end() {
        let now = Utc::now().with_timezone(&UC3M_TIMEZONE);
        Event::new("test", now.clone(), now.clone())
            .duration(Duration::hours(3))
            .end(now + Duration::hours(3));
    }

    #[test]
    fn display_recurrence() {
        let rule = Recurrence::times(TimeUnit::Day, 3);
        assert_eq!(rule.to_string(), "FREQ=DAILY;COUNT=3");

        let last_date = Utc
            .ymd(2022, 8, 19)
            .and_hms(20, 30, 15)
            .with_timezone(&UC3M_TIMEZONE);
        let rule = Recurrence::until(TimeUnit::Week, last_date);
        assert_eq!(rule.to_string(), "FREQ=WEEKLY;UNTIL=20220819T203015");

        let mut rule = Recurrence::times(TimeUnit::Hour, 10);
        rule.interval(2);
        assert_eq!(rule.to_string(), "FREQ=HOURLY;COUNT=10;INTERVAL=2");
    }
}
