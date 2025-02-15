use std::net::{IpAddr, Ipv4Addr};

use askama::Template;
use salvo::catcher::Catcher;
use salvo::conn::TcpListener;
use salvo::http::StatusCode;
use salvo::logging::Logger;
use salvo::macros::Extractible;
use salvo::writing::{Redirect, Text};
use salvo::{Listener, Request, Response, Router, Scribe, Server, Service, handler};
use serde::Deserialize;
use tracing::Level;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    let router = Router::new()
        .push(Router::new().get(start_handler))
        .push(Router::with_path("{lang}/index.html").get(index_handler))
        .push(Router::with_path("{lang}/greet-me.html").get(greeting_handler));
    let server = Service::new(router)
        .catcher(Catcher::default().hoop(not_found_handler))
        .hoop(Logger::new());

    // In a real application you would most likely read the configuration from a config file.
    let acceptor = TcpListener::new((IpAddr::V4(Ipv4Addr::LOCALHOST), 8080))
        .try_bind()
        .await
        .map_err(Error::Bind)?;

    Server::new(acceptor)
        .try_serve(server)
        .await
        .map_err(Error::Run)
}

#[derive(displaydoc::Display, thiserror::Error, pretty_error_debug::Debug)]
enum Error {
    /// could not bind socket
    Bind(#[source] salvo::Error),
    /// could not run server
    Run(#[source] std::io::Error),
}

/// Thanks to this type, your user can select the display language of your page.
///
/// The same type is used by salvo as part of the URL, and in askama to select what content to show,
/// and also as an HTML attribute in `<html lang=`. To make it possible to use the same type for
/// three different use cases, we use a few derive macros:
///
///  * `Default` to have a default/fallback language.
///  * `Debug` is not strictly needed, but it might aid debugging.
///  * `Clone` + `Copy` so that we can pass the language by value.
///  * `PartialEq` so that we can use the type in comparisons with `==` or `!=`.
///  * `serde::Deserialize` so that salvo can parse the type in incoming URLs.
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
    /// could not extract information from request
    Extract(#[from] salvo::http::ParseError),
    /// could not render template
    Render(#[from] askama::Error),
}

/// This is your error handler
impl Scribe for AppError {
    fn render(self, res: &mut Response) {
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

        res.status_code(match &self {
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Extract(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::Render(_) => StatusCode::INTERNAL_SERVER_ERROR,
        });
        let tmpl = Tmpl {
            lang: Lang::default(),
            err: self,
        };
        if let Ok(body) = tmpl.render() {
            Text::Html(body).render(res);
        } else {
            Text::Plain("Something went wrong").render(res);
        }
    }
}

/// This is your "Error: 404 - not found" handler
#[handler]
async fn not_found_handler() -> impl Scribe {
    AppError::NotFound
}

/// This is the first page your user hits, meaning it does not contain language information,
/// so we redirect them.
#[handler]
async fn start_handler() -> impl Scribe {
    Redirect::found("/en/index.html")
}

/// This is the first localized page your user sees.
///
/// It has arguments in the path that need to be parsable using `serde::Deserialize`; see `Lang`
/// for an explanation. And also query parameters (anything after `?` in the incoming URL).
#[handler]
async fn index_handler(req: &mut Request) -> Result<impl Scribe, AppError> {
    /// This type collects the URL params, i.e. the `"/{lang}/"` part
    #[derive(Debug, Deserialize, Extractible)]
    #[salvo(extract(default_source(from = "param")))]
    struct Params {
        lang: Lang,
    }

    /// This type collects the query parameter `?name=` (if present)
    #[derive(Debug, Deserialize, Extractible)]
    #[salvo(extract(default_source(from = "query")))]
    struct Query {
        #[serde(default)]
        name: String,
    }

    let Params { lang } = req.extract().await?;
    let Query { name } = req.extract().await?;

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

    let template = Tmpl { lang, name };
    Ok(Text::Html(template.render()?))
}

/// This is the final page of this example application.
///
/// Like `index_handler` it contains a language in the URL, and a query parameter to read the user's
/// provided name. In here, the query argument `name` has no default value, so salvo will show
/// an error message if absent.
#[handler]
async fn greeting_handler(req: &mut Request) -> Result<impl Scribe, AppError> {
    #[derive(Debug, Deserialize, Extractible)]
    #[salvo(extract(default_source(from = "param")))]
    struct Params {
        lang: Lang,
    }

    #[derive(Debug, Deserialize, Extractible)]
    #[salvo(extract(default_source(from = "query")))]
    struct Query {
        name: String,
    }

    let Params { lang } = req.extract().await?;
    let Query { name } = req.extract().await?;

    #[derive(Debug, Template)]
    #[template(path = "greet.html")]
    struct Tmpl {
        lang: Lang,
        name: String,
    }

    let template = Tmpl { lang, name };
    Ok(Text::Html(template.render()?))
}
