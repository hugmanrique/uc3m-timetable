use cfg_if::cfg_if;
use std::collections::HashMap;
use uc3m_timetable::{Timetable, TimetableId, UC3M_TIMEZONE};
use worker::*;

macro_rules! parse_query_param {
    ($query_params:expr, $name:expr) => {
        if let Some(raw) = $query_params.get($name) {
            match raw.parse() {
                Ok(value) => value,
                Err(_) => {
                    return Response::error(format!("invalid `{}` query parameter", $name), 400)
                }
            }
        } else {
            return Response::error(format!("missing `{}` query parameter", $name), 400);
        }
    };
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    set_panic_hook();
    Router::new()
        .get_async("/", |req, _ctx| async move {
            let url = req.url()?;
            let query_params: HashMap<_, _> = url.query_pairs().into_iter().collect();
            let id = TimetableId::new(
                parse_query_param!(&query_params, "year"),
                parse_query_param!(&query_params, "plan"),
                parse_query_param!(&query_params, "center"),
                parse_query_param!(&query_params, "grade"),
                parse_query_param!(&query_params, "group"),
                parse_query_param!(&query_params, "period"),
                UC3M_TIMEZONE,
            );

            match Timetable::fetch(id).await {
                Ok(timetable) => {
                    let mut headers = Headers::new();
                    headers.set("Content-Type", "text/calendar")?;
                    headers.set("Cache-Control", "public, max-age=3600")?;
                    Ok(Response::ok(timetable.calendar().to_string())?.with_headers(headers))
                }
                Err(err) => Response::error(format!("cannot parse timetable: {}", err), 500),
            }
        })
        .get("/from", move |req, _ctx| {
            let url = req.url()?;
            let query_params: HashMap<_, _> = url.query_pairs().into_iter().collect();

            if let Some(timetable_url) = query_params.get("url") {
                if let Ok(timetable_url) = Url::parse(timetable_url) {
                    match TimetableId::try_from(timetable_url) {
                        Ok(id) => {
                            let redirect = format!(
                                "/?year={}&plan={}&center={}&grade={}&group={}&period={}",
                                id.year, id.plan, id.center, id.grade, id.group, id.period
                            );
                            Response::redirect(Url::parse(&redirect)?)
                        }
                        Err(err) => Response::error(format!("unknown timetable id: {}", err), 400),
                    }
                } else {
                    Response::error("cannot parse timetable url", 400)
                }
            } else {
                Response::error("missing `url` query parameter", 400)
            }
        })
        .run(req, env)
        .await
}

cfg_if! {
    if #[cfg(feature = "console_error_panic_hook")] {
        extern crate console_error_panic_hook;
        pub use self::console_error_panic_hook::set_once as set_panic_hook;
    } else {
        #[inline]
        pub fn set_panic_hook() {}
    }
}
