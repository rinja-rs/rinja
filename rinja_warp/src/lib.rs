#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(elided_lifetimes_in_paths)]
#![deny(unreachable_pub)]
#![doc = include_str!("../README.md")]

#[doc(no_inline)]
pub use rinja::*;
#[doc(no_inline)]
pub use warp;
use warp::reply::Response;

/// Render a [`Template`] into a [`Response`], or render an error page.
#[must_use]
pub fn into_response<T: ?Sized + rinja::Template>(tmpl: &T) -> Response {
    match try_into_response(tmpl) {
        Ok(response) => response,
        Err(err) => warp::http::Response::builder()
            .status(warp::http::StatusCode::INTERNAL_SERVER_ERROR)
            .header(
                warp::http::header::CONTENT_TYPE,
                warp::http::HeaderValue::from_static("text/plain; charset=utf-8"),
            )
            .body(err.to_string().into())
            .unwrap(),
    }
}

/// Try to render a [`Template`] into a [`Response`].
pub fn try_into_response<T: ?Sized + rinja::Template>(tmpl: &T) -> Result<Response, Error> {
    let value = tmpl.render()?.into();
    warp::http::Response::builder()
        .status(warp::http::StatusCode::OK)
        .header(warp::http::header::CONTENT_TYPE, T::MIME_TYPE)
        .body(value)
        .map_err(|err| Error::Custom(err.into()))
}
