use actix_web::http::{header, Method};
use actix_web::{
    get, middleware, web, App, HttpRequest, HttpResponse, HttpServer, Responder, Result,
};
use rinja_actix::Template;
use serde::Deserialize;
use tokio::runtime;

// This function and the next mostly contains boiler plate to get an actix-web application running.
fn main() -> Result<(), Error> {
    let env = env_logger::Env::new().default_filter_or("info");
    env_logger::try_init_from_env(env).map_err(Error::Log)?;

    runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(Error::Rt)?
        .block_on(amain())
}

async fn amain() -> Result<(), Error> {
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

#[derive(thiserror::Error, pretty_error_debug::Debug)]
enum Error {
    #[error("could not setup logger")]
    Log(#[source] log::SetLoggerError),
    #[error("could not setup async runtime")]
    Rt(#[source] std::io::Error),
    #[error("could not bind socket")]
    Bind(#[source] std::io::Error),
    #[error("could not run server")]
    Run(#[source] std::io::Error),
}

/// Using this type your user can select the display language of your page.
///
/// The same type is used by actix-web as part of the URL, and in rinja to select what content to
/// show, and also as an HTML attribute in `<html lang=`. To make it possible to use the same type
/// for three diffent use cases, we use a few derive macros:
///
///  * `Default` to have a default/fallback language.
///  * `Debug` is not strictly needed, but it might aid debugging.
///  * `Clone` + `Copy` so that we can pass the language by value.
///  * `PartialEq` so that we can use the type in comparisons with `==` or `!=`.
///  * `serde::Deserialize` so that actix-web can parse the type in incoming URLs.
///  * `strum::AsRefStr` so that actix-web the use the type to construct URL for printing.
///  * `strum::Display` so that rinja can write the value in templates.
#[derive(Default, Debug, Clone, Copy, PartialEq, Deserialize, strum::AsRefStr, strum::Display)]
#[allow(non_camel_case_types)]
enum Lang {
    #[default]
    en,
    de,
    fr,
}

/// This is your "Error: 404 - not found" handler
async fn not_found_handler(req: HttpRequest) -> Result<impl Responder> {
    // It uses a rinja template to display its content.
    // The member `req` contains the request, and is used e.g. to generate URLs in our template.
    // The member `lang` is used by "_layout.html" which "404.html" extends. Even though it
    // is always the fallback language English in here, "_layout.html" expects to be able to access
    // this field, so you have to provide it.
    #[derive(Debug, Template)]
    #[template(path = "404.html")]
    struct Tmpl {
        req: HttpRequest,
        lang: Lang,
    }

    if req.method() == Method::GET {
        // In here we have to render the result to a string manually, because we don't want to
        // generate a "status 200" result, but "status 404". In other cases you can simply return
        // the template, wrapped in `Ok()`, and the request gets generated with "status 200",
        // and the right MIME type.
        let tmpl = Tmpl {
            req,
            lang: Lang::default(),
        };
        // The MIME type was derived by rinja by the extension of the template file.
        Ok(HttpResponse::NotFound()
            .append_header((header::CONTENT_TYPE, Tmpl::MIME_TYPE))
            .body(tmpl.to_string()))
    } else {
        Ok(HttpResponse::MethodNotAllowed().finish())
    }
}

/// The is first page your user hits does not contain language infomation, so we redirect them
/// to a URL that does contain the default language.
#[get("/")]
async fn start_handler(req: HttpRequest) -> Result<impl Responder> {
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
) -> Result<impl Responder> {
    // Same as in `not_found_handler`, we have `req` to build URLs in the template, and
    // `lang` to select the display language. In the template we both use `{% match lang %}` and
    // `{% if lang !=`, the former to select the text of a specific language, e.g. in the `<title>`;
    // and the latter to display references to all other available languages except the currently
    // selected one.
    // The field `name` will contain the value of the query paramater of the same name.
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
    Ok(Tmpl {
        req,
        lang,
        name: query.name,
    })
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
) -> Result<impl Responder> {
    #[derive(Debug, Template)]
    #[template(path = "greet.html")]
    struct Tmpl {
        req: HttpRequest,
        lang: Lang,
        name: String,
    }

    let (lang,) = path.into_inner();
    Ok(Tmpl {
        req,
        lang,
        name: query.name,
    })
}
