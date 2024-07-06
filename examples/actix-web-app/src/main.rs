use actix_web::http::{header, Method};
use actix_web::{
    get, middleware, web, App, Either, HttpRequest, HttpResponse, HttpServer, Responder, Result,
};
use rinja_actix::Template;
use serde::Deserialize;
use tokio::runtime;

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

#[derive(Default, Debug, Clone, Copy, PartialEq, Deserialize, strum::Display, strum::AsRefStr)]
#[allow(non_camel_case_types)]
enum Lang {
    #[default]
    en,
    de,
}

async fn not_found_handler(req: HttpRequest) -> Result<impl Responder> {
    #[derive(Debug, Template)]
    #[template(path = "404.html")]
    struct Tmpl {
        req: HttpRequest,
        lang: Lang,
    }

    match req.method() {
        &Method::GET => Ok(Either::Left(Tmpl {
            req,
            lang: Lang::default(),
        })),
        _ => Ok(Either::Right(HttpResponse::MethodNotAllowed().finish())),
    }
}

#[get("/")]
async fn start_handler(req: HttpRequest) -> Result<impl Responder> {
    let url = req.url_for("index_handler", [Lang::default()])?;
    Ok(HttpResponse::Found()
        .insert_header((header::LOCATION, url.as_str()))
        .finish())
}

#[derive(Debug, Deserialize)]
struct IndexHandlerQuery {
    #[serde(default)]
    name: String,
}

#[get("/{lang}/index.html")]
async fn index_handler(
    req: HttpRequest,
    path: web::Path<(Lang,)>,
    web::Query(query): web::Query<IndexHandlerQuery>,
) -> Result<impl Responder> {
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
