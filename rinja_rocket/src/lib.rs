#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(elided_lifetimes_in_paths)]
#![deny(unreachable_pub)]

use std::io::Cursor;

#[doc(no_inline)]
pub use rinja::*;
#[doc(no_inline)]
pub use rocket;
use rocket::Response;

#[inline]
pub fn respond<T: ?Sized + rinja::Template>(tmpl: &T) -> rocket::response::Result<'static> {
    try_into_response(tmpl).map_err(|_| rocket::http::Status::InternalServerError)
}

/// Render a [`Template`] into a [`Response`], or render an error page.
pub fn into_response<T: ?Sized + rinja::Template>(tmpl: &T) -> Response<'static> {
    match try_into_response(tmpl) {
        Ok(response) => response,
        Err(err) => {
            let value = err.to_string();
            Response::build()
                .status(rocket::http::Status::InternalServerError)
                .header(rocket::http::Header::new(
                    "content-type",
                    "text/plain; charset=utf-8",
                ))
                .sized_body(value.len(), Cursor::new(value))
                .finalize()
        }
    }
}

/// Try to render a [`Template`] into a [`Response`].
pub fn try_into_response<T: ?Sized + rinja::Template>(
    tmpl: &T,
) -> Result<Response<'static>, Error> {
    let value = tmpl.render()?;
    Ok(Response::build()
        .header(rocket::http::Header::new("content-type", T::MIME_TYPE))
        .sized_body(value.len(), Cursor::new(value))
        .finalize())
}
