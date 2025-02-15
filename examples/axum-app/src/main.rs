use askama::Template;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Router, serve};
use serde::Deserialize;
use tower_http::trace::TraceLayer;
use tracing::{Level, info};

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    let app = Router::new()
        .route("/", get(start_handler))
        .route("/{lang}/index.html", get(index_handler))
        .route("/{lang}/greet-me.html", get(greeting_handler))
        .fallback(|| async { AppError::NotFound })
        .layer(TraceLayer::new_for_http());

    // In a real application you would most likely read the configuration from a config file.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .map_err(Error::Bind)?;

    if let Ok(addr) = listener.local_addr() {
        info!("Listening on http://{addr}/");
    }
    serve(listener, app).await.map_err(Error::Run)
}

#[derive(displaydoc::Display, pretty_error_debug::Debug, thiserror::Error)]
enum Error {
    /// could not bind socket
    Bind(#[source] std::io::Error),
    /// could not run server
    Run(#[source] std::io::Error),
}

/// Thanks to this type, your user can select the display language of your page.
///
/// The same type is used by axum as part of the URL, and in askama to select what content to show,
/// and also as an HTML attribute in `<html lang=`. To make it possible to use the same type for
/// three different use cases, we use a few derive macros:
///
///  * `Default` to have a default/fallback language.
///  * `Debug` is not strictly needed, but it might aid debugging.
///  * `Clone` + `Copy` so that we can pass the language by value.
///  * `PartialEq` so that we can use the type in comparisons with `==` or `!=`.
///  * `serde::Deserialize` so that axum can parse the type in incoming URLs.
///  * `strum::Display` so that askama can write the value in templates.
#[derive(Default, Debug, Clone, Copy, PartialEq, Deserialize, strum::Display)]
#[allow(non_camel_case_types)]
enum Lang {
    #[default]
    en,
    de,
    fr,
}

/// This enum contains any error that could occur while handling an incoming request.
///
/// In a real application you would most likely have multiple error sources, e.g. database errors,
#[derive(Debug, displaydoc::Display, thiserror::Error)]
enum AppError {
    /// not found
    NotFound,
    /// could not render template
    Render(#[from] askama::Error),
}

/// This is your error handler
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // It uses an askama template to display its content.
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
        let tmpl = Tmpl {
            lang: Lang::default(),
            err: self,
        };
        if let Ok(body) = tmpl.render() {
            (status, Html(body)).into_response()
        } else {
            (status, "Something went wrong").into_response()
        }
    }
}

/// This is the first page your user hits, meaning it does not contain language information,
/// so we redirect them.
async fn start_handler() -> Redirect {
    Redirect::temporary("/en/index.html")
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
async fn index_handler(
    Path((lang,)): Path<(Lang,)>,
    Query(query): Query<IndexHandlerQuery>,
) -> Result<impl IntoResponse, AppError> {
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
    Ok(Html(template.render()?))
}

#[derive(Debug, Deserialize)]
struct GreetingHandlerQuery {
    name: String,
}

/// This is the final page of this example application.
///
/// Like `index_handler` it contains a language in the URL, and a query parameter to read the user's
/// provided name. In here, the query argument `name` has no default value, so axum will show
/// an error message if absent.
async fn greeting_handler(
    Path((lang,)): Path<(Lang,)>,
    Query(query): Query<GreetingHandlerQuery>,
) -> Result<impl IntoResponse, AppError> {
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
    Ok(Html(template.render()?))
}
