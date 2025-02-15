use actix_web::error::UrlGenerationError;
use actix_web::http::{StatusCode, header};
use actix_web::web::Html;
use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, Responder, ResponseError, get, middleware, web,
};
use askama::Template;
use serde::Deserialize;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let server = HttpServer::new(|| {
        // This closure contains the setup of the routing rules of your app.
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(middleware::NormalizePath::new(
                middleware::TrailingSlash::MergeOnly,
            ))
            .service(start_handler)
            .service(index_handler)
            .service(greeting_handler)
            .default_service(web::to(not_found_handler))
    });
    // In a real application you would most likely read the configuration from a config file.
    let server = server.bind(("127.0.0.1", 8080)).map_err(Error::Bind)?;
    for addr in server.addrs() {
        println!("Listening on: http://{addr}/");
    }
    server.run().await.map_err(Error::Run)
}

#[derive(displaydoc::Display, thiserror::Error, pretty_error_debug::Debug)]
enum Error {
    /// could not bind socket
    Bind(#[source] std::io::Error),
    /// could not run server
    Run(#[source] std::io::Error),
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
    /// could not generate URL
    Url(#[from] UrlGenerationError),
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl Responder for AppError {
    type Body = String;

    fn respond_to(self, req: &HttpRequest) -> HttpResponse<Self::Body> {
        // The error handler uses a askama template to display its content.
        // The member `lang` is used by "_layout.html" which "error.html" extends. Even though it
        // is always the fallback language English in here, "_layout.html" expects to be able to
        // access this field, so you have to provide it.
        #[derive(Debug, Template)]
        #[template(path = "error.html")]
        struct Tmpl<'a> {
            req: &'a HttpRequest,
            lang: Lang,
            err: &'a AppError,
        }

        let tmpl = Tmpl {
            req,
            lang: Lang::default(),
            err: &self,
        };
        if let Ok(body) = tmpl.render() {
            (Html::new(body), self.status_code()).respond_to(req)
        } else {
            ("Something went wrong".to_string(), self.status_code()).respond_to(req)
        }
    }
}

/// Thanks to this type, your user can select the display language of your page.
///
/// The same type is used by actix-web as part of the URL, and in askama to select what content to
/// show, and also as an HTML attribute in `<html lang=`. To make it possible to use the same type
/// for three different use cases, we use a few derive macros:
///
///  * `Default` to have a default/fallback language.
///  * `Debug` is not strictly needed, but it might aid debugging.
///  * `Clone` + `Copy` so that we can pass the language by value.
///  * `PartialEq` so that we can use the type in comparisons with `==` or `!=`.
///  * `serde::Deserialize` so that actix-web can parse the type in incoming URLs.
///  * `strum::AsRefStr` so that actix-web the use the type to construct URL for printing.
///  * `strum::Display` so that askama can write the value in templates.
#[derive(Default, Debug, Clone, Copy, PartialEq, Deserialize, strum::AsRefStr, strum::Display)]
#[allow(non_camel_case_types)]
enum Lang {
    #[default]
    en,
    de,
    fr,
}

/// This is your "Error: 404 - not found" handler
async fn not_found_handler() -> AppError {
    AppError::NotFound
}

/// This is the first page your user hits, meaning it does not contain language information,
/// so we redirect them.
#[get("/")]
async fn start_handler(req: HttpRequest) -> Result<impl Responder, AppError> {
    // This example show how the type `Lang` can be used to construct a URL in actix-web.
    let url = req.url_for("index_handler", [Lang::default()])?;
    Ok(HttpResponse::Found()
        .insert_header((header::LOCATION, url.as_str()))
        .finish())
}

/// This type collects the query parameter `?name=` (if present)
#[derive(Debug, Deserialize)]
struct IndexHandlerQuery {
    #[serde(default)]
    name: String,
}

/// This is the first localized page your user sees.
///
/// It has arguments in the path that need to be parsable using `serde::Deserialize`;
/// see `Lang` for an explanation.
/// And also query parameters (anything after `?` in the incoming URL).
#[get("/{lang}/index.html")]
async fn index_handler(
    req: HttpRequest,
    path: web::Path<(Lang,)>,
    web::Query(query): web::Query<IndexHandlerQuery>,
) -> Result<impl Responder, AppError> {
    // Same as in `not_found_handler`, we have `req` to build URLs in the template, and
    // `lang` to select the display language. In the template we both use `{% match lang %}` and
    // `{% if lang !=`, the former to select the text of a specific language, e.g. in the `<title>`;
    // and the latter to display references to all other available languages except the currently
    // selected one.
    // The field `name` will contain the value of the query parameter of the same name.
    // In `IndexHandlerQuery` we annotated the field with `#[serde(default)]`, so if the value is
    // absent, an empty string is selected by default, which is visible to the user an empty
    // `<input type="text" />` element.
    #[derive(Debug, Template)]
    #[template(path = "index.html")]
    struct Tmpl {
        req: HttpRequest,
        lang: Lang,
        name: String,
    }

    let (lang,) = path.into_inner();
    let template = Tmpl {
        req,
        lang,
        name: query.name,
    };
    Ok(Html::new(template.render()?))
}

#[derive(Debug, Deserialize)]
struct GreetingHandlerQuery {
    name: String,
}

/// This is the final page of this example application.
///
/// Like `index_handler` it contains a language in the URL, and a query parameter to read the user's
/// provided name. In here, the query argument `name` has no default value, so actix-web will show
/// an error message if absent.
#[get("/{lang}/greet-me.html")]
async fn greeting_handler(
    req: HttpRequest,
    path: web::Path<(Lang,)>,
    web::Query(query): web::Query<GreetingHandlerQuery>,
) -> Result<impl Responder, AppError> {
    #[derive(Debug, Template)]
    #[template(path = "greet.html")]
    struct Tmpl {
        req: HttpRequest,
        lang: Lang,
        name: String,
    }

    let (lang,) = path.into_inner();
    let template = Tmpl {
        req,
        lang,
        name: query.name,
    };
    Ok(Html::new(template.render()?))
}
