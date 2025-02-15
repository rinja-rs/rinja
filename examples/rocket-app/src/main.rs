use std::net::{IpAddr, Ipv4Addr};

use askama::Template;
use rocket::http::Status;
use rocket::request::FromParam;
use rocket::response::content::RawHtml;
use rocket::response::{Redirect, Responder};
use rocket::{Config, Request, Response, catch, catchers, get, routes};

#[rocket::main]
async fn main() -> Result<(), Error> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    // In a real application you would most likely read the configuration from a config file.
    let config = Config {
        address: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port: 8080,
        ..Config::debug_default()
    };
    rocket::custom(&config)
        .mount("/", routes![start_handler, index_handler, greeting_handler])
        .register("/", catchers![not_found_handler])
        .launch()
        .await
        .map_err(Error::Launch)?;
    Ok(())
}

#[derive(displaydoc::Display, thiserror::Error, pretty_error_debug::Debug)]
enum Error {
    /// could not start server
    Launch(#[source] rocket::Error),
}

/// Thanks to this type, your user can select the display language of your page.
///
/// The same type is used by warp as part of the URL, and in askama to select what content to show,
/// and also as an HTML attribute in `<html lang=`. To make it possible to use the same type for
/// three different use cases, we use a few derive macros:
///
///  * `Default` to have a default/fallback language.
///  * `Debug` is not strictly needed, but it might aid debugging.
///  * `Clone` + `Copy` so that we can pass the language by value.
///  * `PartialEq` so that we can use the type in comparisons with `==` or `!=`.
///  * `strum::Display` so that askama can write the value in templates.
///  * `strum::EnumString` so that warp can parse the type in incoming URLs.
#[derive(Default, Debug, Clone, Copy, PartialEq, strum::EnumString, strum::Display)]
#[allow(non_camel_case_types)]
enum Lang {
    #[default]
    en,
    de,
    fr,
}

impl FromParam<'_> for Lang {
    type Error = AppError;

    fn from_param(param: &str) -> Result<Self, Self::Error> {
        param.parse().map_err(|_| AppError::NotFound)
    }
}

/// This enum contains any error that could occur while handling an incoming request.
///
/// In a real application you would most likely have multiple error sources, e.g. database errors.
#[derive(Debug, displaydoc::Display, thiserror::Error)]
enum AppError {
    /// not found
    NotFound,
    /// could not render template
    Render(#[from] askama::Error),
}

/// This is your error handler
impl<'r> Responder<'r, 'static> for AppError {
    fn respond_to(self, request: &'r Request<'_>) -> Result<Response<'static>, Status> {
        // It uses a askama template to display its content.
        // The member `lang` is used by "_layout.html" which "error.html" extends. Even though it
        // is always the fallback language English in here, "_layout.html" expects to be able to
        // access this field, so you have to provide it.
        #[derive(Debug, Template)]
        #[template(path = "error.html")]
        struct Tmpl {
            lang: Lang,
            err: AppError,
        }

        let status = match &self {
            AppError::NotFound => Status::NotFound,
            AppError::Render(_) => Status::InternalServerError,
        };
        let template = Tmpl {
            lang: Lang::default(),
            err: self,
        };
        if let Ok(body) = template.render() {
            (status, RawHtml(body)).respond_to(request)
        } else {
            (status, "internal server error").respond_to(request)
        }
    }
}

#[catch(404)]
fn not_found_handler() -> AppError {
    AppError::NotFound
}

/// This is the first page your user hits, meaning it does not contain language information,
/// so we redirect them.
#[get("/")]
fn start_handler<'r>() -> impl Responder<'r, 'static> {
    Redirect::found("/en/index.html")
}

/// This is the first localized page your user sees.
///
/// It has arguments in the path that need to be parsable using `FromParam`; see `Lang`
/// for an explanation. And also query parameters (anything after `?` in the incoming URL).
#[get("/<lang>/index.html?<name>")]
fn index_handler(lang: Lang, name: Option<&str>) -> Result<impl Responder<'_, 'static>, AppError> {
    // In the template we both use `{% match lang %}` and `{% if lang !=`, the former to select the
    // text of a specific language, e.g. in the `<title>`; and the latter to display references to
    // all other available languages except the currently selected one.
    // The field `name` will contain the value of the query parameter of the same name.
    // In `IndexHandlerQuery` we annotated the field with `#[serde(default)]`, so if the value is
    // absent, an empty string is selected by default, which is visible to the user an empty
    // `<input type="text" />` element.
    #[derive(Debug, Template)]
    #[template(path = "index.html")]
    struct Tmpl<'a> {
        lang: Lang,
        name: &'a str,
    }

    let template = Tmpl {
        lang,
        name: name.unwrap_or_default(),
    };
    Ok(RawHtml(template.render()?))
}

/// This is the final page of this example application.
///
/// Like `index_handler` it contains a language in the URL, and a query parameter to read the
/// user's provided name. In here, the query argument `name` has no default value, so rocket will
/// show a "422: Unprocessable Entity" message if absent.
#[get("/<lang>/greet-me.html?<name>")]
fn greeting_handler<'r>(lang: Lang, name: &str) -> Result<impl Responder<'r, 'static>, AppError> {
    #[derive(Debug, Template)]
    #[template(path = "greet.html")]
    struct Tmpl<'a> {
        lang: Lang,
        name: &'a str,
    }

    let template = Tmpl { lang, name };
    Ok(RawHtml(template.render()?))
}
