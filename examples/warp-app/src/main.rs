use std::net::Ipv4Addr;

use http::{StatusCode, Uri};
use rinja::Template;
use serde::Deserialize;
use warp::filters::query::query;
use warp::reply::{Reply, Response, html, with_status};
use warp::{Filter, any, get, path, redirect, serve};

#[tokio::main]
async fn main() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let routes = path!()
        .map(start_handler)
        .or(path!(Lang / "index.html").and(query()).map(index_handler))
        .or(path!(Lang / "greet-me.html")
            .and(query())
            .map(greeting_handler))
        .or(any().map(|| AppError::NotFound));
    let routes = get().and(routes).with(warp::log("warp-app"));

    // In a real application you would most likely read the configuration from a config file.
    serve(routes).run((Ipv4Addr::LOCALHOST, 8080)).await;
}

/// Thanks to this type, your user can select the display language of your page.
///
/// The same type is used by warp as part of the URL, and in rinja to select what content to show,
/// and also as an HTML attribute in `<html lang=`. To make it possible to use the same type for
/// three different use cases, we use a few derive macros:
///
///  * `Default` to have a default/fallback language.
///  * `Debug` is not strictly needed, but it might aid debugging.
///  * `Clone` + `Copy` so that we can pass the language by value.
///  * `PartialEq` so that we can use the type in comparisons with `==` or `!=`.
///  * `strum::Display` so that rinja can write the value in templates.
///  * `strum::EnumString` so that warp can parse the type in incoming URLs.
#[derive(Default, Debug, Clone, Copy, PartialEq, strum::EnumString, strum::Display)]
#[allow(non_camel_case_types)]
enum Lang {
    #[default]
    en,
    de,
    fr,
}

/// This enum contains any error that could occur while handling an incoming request.
///
/// In a real application you would most likely have multiple error sources, e.g. database errors.
#[derive(Debug, displaydoc::Display, thiserror::Error)]
enum AppError {
    /// not found
    NotFound,
    /// could not render template
    Render(#[from] rinja::Error),
}

/// This is your error handler
impl Reply for AppError {
    fn into_response(self) -> Response {
        // It uses a rinja template to display its content.
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
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Render(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let template = Tmpl {
            lang: Lang::default(),
            err: self,
        };
        if let Ok(body) = template.render() {
            with_status(html(body), status).into_response()
        } else {
            status.into_response()
        }
    }
}

/// This is the first page your user hits, meaning it does not contain language information,
/// so we redirect them.
fn start_handler() -> impl Reply {
    redirect::found(Uri::from_static("/en/index.html"))
}

/// This type collects the query parameter `?name=` (if present)
#[derive(Debug, Deserialize)]
struct IndexHandlerQuery {
    #[serde(default)]
    name: String,
}

/// This is the first localized page your user sees.
///
/// It has arguments in the path that need to be parsable using `serde::Deserialize`; see `Lang`
/// for an explanation. And also query parameters (anything after `?` in the incoming URL).
fn index_handler(lang: Lang, query: IndexHandlerQuery) -> Result<impl Reply, AppError> {
    // In the template we both use `{% match lang %}` and `{% if lang !=`, the former to select the
    // text of a specific language, e.g. in the `<title>`; and the latter to display references to
    // all other available languages except the currently selected one.
    // The field `name` will contain the value of the query parameter of the same name.
    // In `IndexHandlerQuery` we annotated the field with `#[serde(default)]`, so if the value is
    // absent, an empty string is selected by default, which is visible to the user an empty
    // `<input type="text" />` element.
    #[derive(Debug, Template)]
    #[template(path = "index.html")]
    struct Tmpl {
        lang: Lang,
        name: String,
    }

    let template = Tmpl {
        lang,
        name: query.name,
    };
    Ok(html(template.render()?))
}

#[derive(Debug, Deserialize)]
struct GreetingHandlerQuery {
    name: String,
}

/// This is the final page of this example application.
///
/// Like `index_handler` it contains a language in the URL, and a query parameter to read the
/// user's provided name. In here, the query argument `name` has no default value, so warp will
/// show a "404 - Not Found" message if absent.
fn greeting_handler(lang: Lang, query: GreetingHandlerQuery) -> Result<impl Reply, AppError> {
    #[derive(Debug, Template)]
    #[template(path = "greet.html")]
    struct Tmpl {
        lang: Lang,
        name: String,
    }

    let template = Tmpl {
        lang,
        name: query.name,
    };
    Ok(html(template.render()?))
}
