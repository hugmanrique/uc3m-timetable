pub(crate) mod components;

use chrono::{DateTime, Datelike, Timelike};
use chrono_tz::Tz;
use itertools::Itertools;
use std::fmt::{Display, Formatter, Write};
use std::slice;

/// A container of [`Prop`]s.
///
/// Some properties can have multiple values, in which case
/// multiple [`Prop`]s with the same name may be added to
/// the holder. Some property types also support encoding
/// multiple values in a single [`Prop`] by separating
/// the values with a comma (`,`) character.
trait PropHolder {
    /// Returns the properties held by this object.
    fn props(&self) -> &Vec<Prop>;

    /// Searches for a property with the given name.
    fn first_prop(&self, name: &str) -> Option<&Prop> {
        self.props().iter().find(|prop| prop.name == name)
    }

    /// Tests if any property has the given name.
    fn has_prop(&self, name: &str) -> bool {
        self.props().iter().any(|prop| prop.name == name)
    }
}

/// An iCalendar object consisting of a sequence of
/// [`Prop`]s and [`Component`]s.
///
/// A property applies to the calendar object as a whole.
/// The components are collections of properties that
/// express a particular calendar semantic.
///
/// A [`VTIMEZONE`] component must be specified for each
/// unique non-global `TZID` parameter value specified in
/// the calendar object. Due to their complexity, all objects
/// of type `Tz` are formatted using global IDs (which are
/// prefixed with a solidus character -- `/`).
#[derive(Debug, Eq, PartialEq)]
pub struct Calendar {
    props: Vec<Prop>,
    components: Vec<Component>,
}

impl Calendar {
    /// Creates a calendar, where `product` is the identifier
    /// of the product that created the calendar object,
    /// `spec_version` is the highest version number of
    /// the iCalendar specification required to interpret
    /// the iCalendar object, and `components` is a non-empty
    /// vector.
    pub fn new(product: &str, spec_version: &str, components: Vec<Component>) -> Self {
        assert!(!components.is_empty(), "calendar must have >= 1 components");
        Self {
            // We don't provide mutable access to the `props` vector,
            // setting the "PRODID" and "VERSION" props should suffice
            // for now.
            props: vec![
                Prop::text("PRODID", slice::from_ref(&product)),
                Prop::text("VERSION", slice::from_ref(&spec_version)),
            ],
            components,
        }
    }

    /// Gets a reference to the calendar components.
    pub fn components(&self) -> &Vec<Component> {
        &self.components
    }

    /// Gets a mutable reference to the calendar components.
    pub fn components_mut(&mut self) -> &mut Vec<Component> {
        &mut self.components
    }
}

impl PropHolder for Calendar {
    fn props(&self) -> &Vec<Prop> {
        &self.props
    }
}

impl Display for Calendar {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("BEGIN:VCALENDAR\r\n")?;
        for prop in &self.props {
            f.write_str(&prop.to_string())?;
        }
        for component in &self.components {
            f.write_str(&component.to_string())?;
        }
        f.write_str("END:VCALENDAR\r\n")
    }
}

/// A collection of [`Prop`]s that express a particular
/// calendar semantic.
///
/// For example, a component may specify an event, a to-do,
/// time zone information, free/busy time information,
/// an alarm, etc.
#[derive(Debug, Eq, PartialEq)]
pub struct Component {
    name: &'static str,
    props: Vec<Prop>,
}

impl Component {
    /// Creates a component.
    pub fn new(name: &'static str, props: Vec<Prop>) -> Self {
        Self { name, props }
    }
}

impl PropHolder for Component {
    fn props(&self) -> &Vec<Prop> {
        &self.props
    }
}

impl Display for Component {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "BEGIN:{}\r\n", self.name)?;
        for prop in &self.props {
            f.write_str(&prop.to_string())?;
        }
        write!(f, "END:{}\r\n", self.name)
    }
}

/// A calendar property.
#[derive(Debug, Eq, PartialEq)]
pub struct Prop {
    name: &'static str,
    params: Vec<Param>,
    value: String,
}

impl Prop {
    /// Creates a property.
    pub fn new<V: Into<String>>(name: &'static str, value: V) -> Self {
        Self {
            name,
            params: Vec::new(),
            value: value.into(),
        }
    }

    /// Creates a property with comma-separated textual values, escaping
    /// characters if necessary.
    ///
    /// The language in which the text is represented can be defined
    /// by the `LANGUAGE` property [parameter](Param).
    ///
    /// To pass a single textual value without copying, use [`slice::from_ref`].
    pub fn text<V: AsRef<str>>(name: &'static str, values: &[V]) -> Self {
        // Escape text according to section 3.3.11.
        let value = values
            .iter()
            .map(V::as_ref)
            .map(|str| {
                str.replace('\\', r"\\")
                    .replace(';', r"\;")
                    .replace(',', r"\,")
                    .replace('\n', r"\n")
            })
            .join(",");
        Self::new(name, value)
    }

    /// Creates a property with a date-time value.
    pub fn date_time(name: &'static str, date_time: &DateTime<Tz>) -> Self {
        let global_tz_id = format!("/{}", date_time.timezone().name());
        Self {
            name,
            params: vec![Param::new("TZID", vec![global_tz_id])],
            value: format_date_time(date_time),
        }
    }

    /// Gets a reference to the property parameters.
    pub const fn params(&self) -> &Vec<Param> {
        &self.params
    }

    /// Gets a mutable reference to the property parameters.
    pub fn params_mut(&mut self) -> &mut Vec<Param> {
        &mut self.params
    }
}

impl Display for Prop {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // Line folding algorithm
        const MAX_LINE_LEN: usize = 75; // bytes
        let mut line_len = 0;
        let mut write_folded = |part: &str| -> std::fmt::Result {
            for ch in part.chars() {
                let ch_len = ch.len_utf8();
                line_len += ch_len;
                if line_len > MAX_LINE_LEN {
                    line_len = 1 + ch_len;
                    f.write_str("\r\n ")?;
                }
                f.write_char(ch)?;
            }
            Ok(())
        };

        write_folded(self.name)?;
        for param in &self.params {
            // Property parameters with values containing '.', ';' or
            // ',' characters must be placed in quoted text. Always
            // escape to avoid the lookup.
            let values = param.values.join(r#"",""#);
            write_folded(&format!(r#";{}="{}""#, param.name, values))?;
        }
        write_folded(&format!(":{}\r\n", self.value))
    }
}

/// A [`Prop`] parameter, containing meta-information about
/// the property or the property value.
#[derive(Debug, Eq, PartialEq)]
pub struct Param {
    name: &'static str,
    values: Vec<String>,
}

impl Param {
    pub fn new(name: &'static str, values: Vec<String>) -> Self {
        for value in &values {
            assert!(
                !value.contains('"'),
                "Parameter value cannot contain double quotes (\"); got '{}'",
                value
            );
        }
        Self { name, values }
    }
}

/// Formats a date with local time according to the RFC 5545
/// specification.
pub fn format_date_time(date_time: &DateTime<Tz>) -> String {
    // The format is loosely based on ISO 8601, but with
    // dashes (-) and the UTC offset stripped.
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}",
        date_time.year(),
        date_time.month(),
        date_time.day(),
        date_time.hour(),
        date_time.minute(),
        date_time.second()
    )
}

#[cfg(test)]
mod tests {
    use crate::ical::components::{Event, Recurrence, TimeUnit};
    use crate::ical::{Calendar, Param, Prop};
    use crate::UC3M_TIMEZONE;
    use chrono::{Duration, TimeZone, Utc};

    // Calendar

    #[test]
    fn single_event() {
        let date = Utc
            .ymd(2022, 8, 19)
            .and_hms(19, 52, 3)
            .with_timezone(&UC3M_TIMEZONE); // 21:52:03 in Madrid
        let event = Event::new("5678", date.clone(), date);
        let calendar = Calendar::new("test", "2.0", vec![event.into()]);
        assert_eq!(calendar.to_string(), "BEGIN:VCALENDAR\r\nPRODID:test\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nDTSTAMP;TZID=\"/Europe/Madrid\":20220819T215203\r\nUID:5678\r\nDTSTART;TZID=\"/Europe/Madrid\":20220819T215203\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n");
    }

    #[test]
    fn weekly_event() {
        let last_modified = Utc
            .ymd(2022, 8, 17)
            .and_hms(22, 16, 0)
            .with_timezone(&UC3M_TIMEZONE); // 2022-08-18T00:16:00 in Madrid
        let first_lecture = Utc
            .ymd(2022, 9, 12)
            .and_hms(9, 0, 0)
            .with_timezone(&UC3M_TIMEZONE); // 2022-09-12T11:00:00 in Madrid
        let event = Event::new("lecture", last_modified.clone(), first_lecture)
            .duration(Duration::hours(2))
            .created_on(last_modified)
            .summary("Lecture")
            .location("Room 101")
            .recurrence(Recurrence::times(TimeUnit::Week, 12));
        let calendar = Calendar::new("scheduler", "2.0", vec![event.into()]);
        assert_eq!(calendar.to_string(), "BEGIN:VCALENDAR\r\nPRODID:scheduler\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nDTSTAMP;TZID=\"/Europe/Madrid\":20220818T001600\r\nUID:lecture\r\nDTSTART;TZID=\"/Europe/Madrid\":20220912T110000\r\nCREATED;TZID=\"/Europe/Madrid\":20220818T001600\r\nSUMMARY:Lecture\r\nLOCATION:Room 101\r\nDURATION:PT7200S\r\nRRULE:FREQ=WEEKLY;COUNT=12\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n");
    }

    #[test]
    #[should_panic]
    fn calendar_no_components() {
        Calendar::new("Awesome", "2.0", Vec::new());
    }

    // Properties & parameters

    #[test]
    fn prop_display() {
        assert_eq!(
            Prop::new("TITLE", "Hello world!").to_string(),
            "TITLE:Hello world!\r\n"
        );

        let mut prop = Prop::new("TABLE", "The value.");
        prop.params_mut().push(Param::new("ROW", vec![";,".into()]));
        assert_eq!(prop.to_string(), "TABLE;ROW=\";,\":The value.\r\n");

        let mut prop = Prop::new("NAME", "Something.");
        prop.params_mut().extend([
            Param::new("FOO", vec!["bar".into(), "baz".into()]),
            Param::new(
                "ANOTHER",
                vec!["hello".into(), "beautiful".into(), "world".into()],
            ),
        ]);
        assert_eq!(
            prop.to_string(),
            "NAME;FOO=\"bar\",\"baz\";ANOTHER=\"hello\",\"beautiful\",\"world\":Something.\r\n"
        );

        assert_eq!(Prop::new("DESCRIPTION", "This is a long description that exists on multiple long lines since this is a very long string that exceeds the maximum number of bytes allowed by the iCalendar specification published in the Request for Comments 5545 in September 2009.").to_string(), "DESCRIPTION:This is a long description that exists on multiple long lines s\r\n ince this is a very long string that exceeds the maximum number of bytes a\r\n llowed by the iCalendar specification published in the Request for Comment\r\n s 5545 in September 2009.\r\n");
    }

    #[test]
    #[should_panic]
    fn param_value_with_double_quotes() {
        Param::new("HELLO", vec!["this is a double quote: \"".into()]);
    }
}
