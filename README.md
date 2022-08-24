# uc3m-timetable

Download your [UC3M](https://www.uc3m.es/Home) lecture timetable in iCalendar (`.ics`) format,
ready to import into your favorite calendar application.

## Usage

First, access the page of the degree you're enrolled in (here's the list of [bachelor degrees](https://www.uc3m.es/bachelor-degree/studies)
and of [master programs](https://www.uc3m.es/postgraduate/programs)). Expand the "Schedules/Practical information" tab and click
on the "Schedule in bachelor's degree/Master's course schedule" link. Select the current semester and your grade, and check
that the URL has the form
```
https://aplicaciones.uc3m.es/horarios-web/publicacion/{year}/porCentroPlanCursoGrupo.tt?plan={plan}&centro={center}&curso={grade}&grupo={group}&tipoPer=C&valorPer={period}
```
Next, fill in the parameters of the following URL and access the site
```
https://uc3m-timetable.hugmanrique.me/?year={year}&plan={plan}&center={center}&grade={grade}&period={period}
```
An iCalendar object file (`.ics`) should have been downloading. To import it in your application, create a new calendar
and see the following guides:

- [Google Calendar](https://support.google.com/calendar/answer/37118) -- step 2,
- [Apple Calendar](https://support.apple.com/guide/calendar/import-or-export-calendars-icl1023/mac) -- "Import events into a calendar".

To update the timetable, download the iCalendar file, recreate the calendar, and import the file.

## Setup
You'll need the following dependencies to build uc3m-timetable
- rustc >= 1.63
- node >= 18.7

```bash
yarn install
cargo install worker-build
```

## Build
Build crates:
```bash
worker-build --release
```

Build & run local server:
```bash
wrangler dev --local
```

## License

[MIT](LICENSE) &copy; [Hugo Manrique](https://hugmanrique.me)

[license]: https://img.shields.io/github/license/hugmanrique/Cartage.svg
[license-url]: LICENSE