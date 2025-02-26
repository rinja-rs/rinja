use askama::Template;
use poem::error::{NotFoundError, ResponseError};
use poem::http::StatusCode;
use poem::listener::TcpListener;
use poem::middleware::Tracing;
use poem::web::{Html, Path, Query, Redirect};
use poem::{EndpointExt, IntoResponse, Response, Route, Server, get, handler};
use serde::Deserialize;
use tracing::Level;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    let app = Route::new()
        .at("/", get(start_handler))
        .at("/:lang/index.html", get(index_handler))
        .at("/:lang/greet-me.html", get(greeting_handler))
        .catch_error(|_: NotFoundError| async { AppError::NotFound })
        .with(Tracing);

    // In a real application you would most likely read the configuration from a config file.
    Server::new(TcpListener::bind("127.0.0.1:8080"))
        .run(app)
        .await
}

/// Thanks to this type, your user can select the display language of your page.
///
/// The same type is used by poem as part of the URL, and in askama to select what content to show,
/// and also as an HTML attribute in `<html lang=`. To make it possible to use the same type for
/// three different use cases, we use a few derive macros:
///
///  * `Default` to have a default/fallback language.
///  * `Debug` is not strictly needed, but it might aid debugging.
///  * `Clone` + `Copy` so that we can pass the language by value.
///  * `PartialEq` so that we can use the type in comparisons with `==` or `!=`.
///  * `serde::Deserialize` so that poem can parse the type in incoming URLs.
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

impl ResponseError for AppError {
    fn status(&self) -> StatusCode {
        match self {
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Render(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
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
        struct Tmpl<'a> {
            lang: Lang,
            err: &'a AppError,
        }

        let tmpl = Tmpl {
            lang: Lang::default(),
            err: &self,
        };
        if let Ok(body) = tmpl.render() {
            (self.status(), Html(body)).into_response()
        } else {
            (self.status(), "Something went wrong").into_response()
        }
    }
}

/// This is the first page your user hits, meaning it does not contain language information,
/// so we redirect them.
#[handler]
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
#[handler]
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
/// provided name. In here, the query argument `name` has no default value, so poem will show
/// an "400: Bad Request" error message if absent.
#[handler]
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
