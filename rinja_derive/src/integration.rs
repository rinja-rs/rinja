use std::fmt::{Arguments, Display, Write};

use quote::quote;
use syn::DeriveInput;

/// Implement every integration for the given item
pub(crate) fn impl_everything(ast: &DeriveInput, buf: &mut Buffer, only_template: bool) {
    impl_display(ast, buf);
    impl_fast_writable(ast, buf);

    if !only_template {
        #[cfg(feature = "with-actix-web")]
        impl_actix_web_responder(ast, buf);
        #[cfg(feature = "with-axum")]
        impl_axum_into_response(ast, buf);
        #[cfg(feature = "with-rocket")]
        impl_rocket_responder(ast, buf);
        #[cfg(feature = "with-warp")]
        impl_warp_reply(ast, buf);
    }
}

/// Writes header for the `impl` for `TraitFromPathName` or `Template` for the given item
pub(crate) fn write_header(
    ast: &DeriveInput,
    buf: &mut Buffer,
    target: impl Display,
    params: Option<Vec<syn::GenericParam>>,
) {
    let mut generics;
    let (impl_generics, orig_ty_generics, where_clause) = if let Some(params) = params {
        generics = ast.generics.clone();
        for param in params {
            generics.params.push(param);
        }

        let (_, orig_ty_generics, _) = ast.generics.split_for_impl();
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        (impl_generics, orig_ty_generics, where_clause)
    } else {
        ast.generics.split_for_impl()
    };

    let ident = &ast.ident;
    buf.write(format_args!(
        "impl {} {} for {} {{",
        quote!(#impl_generics),
        target,
        quote!(#ident #orig_ty_generics #where_clause),
    ));
}

/// Implement `Display` for the given item.
fn impl_display(ast: &DeriveInput, buf: &mut Buffer) {
    let ident = &ast.ident;
    buf.write(format_args!(
        "\
        /// Implement the [`format!()`][rinja::helpers::std::format] trait for [`{}`]\n\
        ///\n\
        /// Please be aware of the rendering performance notice in the \
            [`Template`][rinja::Template] trait.\n\
        ",
        quote!(#ident),
    ));
    write_header(ast, buf, "rinja::helpers::core::fmt::Display", None);
    buf.write(
        "\
            #[inline]\
            fn fmt(\
                &self,\
                f: &mut rinja::helpers::core::fmt::Formatter<'_>\
            ) -> rinja::helpers::core::fmt::Result {\
                rinja::Template::render_into(self, f)\
                    .map_err(|_| rinja::helpers::core::fmt::Error)\
            }\
        }",
    );
}

/// Implement `FastWritable` for the given item.
fn impl_fast_writable(ast: &DeriveInput, buf: &mut Buffer) {
    write_header(ast, buf, "rinja::filters::FastWritable", None);
    buf.write(
        "\
            #[inline]\
            fn write_into<RinjaW>(&self, dest: &mut RinjaW) -> rinja::Result<()> \
            where \
                RinjaW: rinja::helpers::core::fmt::Write + ?rinja::helpers::core::marker::Sized,\
            {\
                rinja::Template::render_into(self, dest)\
            }\
        }",
    );
}

/// Implement Actix-web's `Responder`.
#[cfg(feature = "with-actix-web")]
fn impl_actix_web_responder(ast: &DeriveInput, buf: &mut Buffer) {
    write_header(ast, buf, "::rinja_actix::actix_web::Responder", None);
    buf.write(
        "\
            type Body = ::rinja_actix::actix_web::body::BoxBody;\
            #[inline]\
            fn respond_to(self, _req: &::rinja_actix::actix_web::HttpRequest)\
            -> ::rinja_actix::actix_web::HttpResponse<Self::Body> {\
                ::rinja_actix::into_response(&self)\
            }\
        }",
    );
}

/// Implement Axum's `IntoResponse`.
#[cfg(feature = "with-axum")]
fn impl_axum_into_response(ast: &DeriveInput, buf: &mut Buffer) {
    write_header(
        ast,
        buf,
        "::rinja_axum::axum_core::response::IntoResponse",
        None,
    );
    buf.write(
        "\
            #[inline]\
            fn into_response(self) -> ::rinja_axum::axum_core::response::Response {\
                ::rinja_axum::into_response(&self)\
            }\
        }",
    );
}

/// Implement Rocket's `Responder`.
#[cfg(feature = "with-rocket")]
fn impl_rocket_responder(ast: &DeriveInput, buf: &mut Buffer) {
    let lifetime1 = syn::Lifetime::new("'rinja1", proc_macro2::Span::call_site());
    let param1 = syn::GenericParam::Lifetime(syn::LifetimeParam::new(lifetime1));

    write_header(
        ast,
        buf,
        "::rinja_rocket::rocket::response::Responder<'rinja1, 'static>",
        Some(vec![param1]),
    );
    buf.write(
        "\
            #[inline]\
            fn respond_to(self, _: &'rinja1 ::rinja_rocket::rocket::request::Request<'_>)\
                -> ::rinja_rocket::rocket::response::Result<'static>\
            {\
                ::rinja_rocket::respond(&self)\
            }\
        }",
    );
}

/// Implement Warp's `Reply`.
#[cfg(feature = "with-warp")]
fn impl_warp_reply(ast: &DeriveInput, buf: &mut Buffer) {
    write_header(ast, buf, "::rinja_warp::warp::reply::Reply", None);
    buf.write(
        "\
            #[inline]\
            fn into_response(self) -> ::rinja_warp::warp::reply::Response {\
                ::rinja_warp::into_response(&self)\
            }\
        }",
    );
}

#[derive(Debug)]
pub(crate) struct Buffer {
    // The buffer to generate the code into
    buf: String,
    discard: bool,
    last_was_write_str: bool,
}

impl Display for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.buf)
    }
}

impl Buffer {
    pub(crate) fn new() -> Self {
        Self {
            buf: String::new(),
            discard: false,
            last_was_write_str: false,
        }
    }

    pub(crate) fn into_string(self) -> String {
        self.buf
    }

    pub(crate) fn is_discard(&self) -> bool {
        self.discard
    }

    pub(crate) fn set_discard(&mut self, discard: bool) {
        self.discard = discard;
        self.last_was_write_str = false;
    }

    pub(crate) fn write(&mut self, src: impl BufferFmt) {
        if self.discard {
            return;
        }
        self.last_was_write_str = false;

        src.append_to(&mut self.buf);
    }

    pub(crate) fn write_separated_path(&mut self, path: &[&str]) {
        if self.discard {
            return;
        }
        self.last_was_write_str = false;

        for (idx, item) in path.iter().enumerate() {
            if idx > 0 {
                self.buf.push_str("::");
            }
            self.buf.push_str(item);
        }
    }

    pub(crate) fn write_escaped_str(&mut self, s: &str) {
        if self.discard {
            return;
        }
        self.last_was_write_str = false;

        self.buf.push('"');
        string_escape(&mut self.buf, s);
        self.buf.push('"');
    }

    pub(crate) fn write_writer(&mut self, s: &str) -> usize {
        const OPEN: &str = r#"writer.write_str(""#;
        const CLOSE: &str = r#"")?;"#;

        if !s.is_empty() && !self.discard {
            if !self.last_was_write_str {
                self.last_was_write_str = true;
                self.buf.push_str(OPEN);
            } else {
                // strip trailing `")?;`, leaving an unterminated string
                self.buf.truncate(self.buf.len() - CLOSE.len());
            }
            string_escape(&mut self.buf, s);
            self.buf.push_str(CLOSE);
        }
        s.len()
    }

    pub(crate) fn clear(&mut self) {
        self.buf.clear();
        self.last_was_write_str = false;
    }
}

pub(crate) trait BufferFmt {
    fn append_to(&self, buf: &mut String);
}

impl<T: BufferFmt + ?Sized> BufferFmt for &T {
    fn append_to(&self, buf: &mut String) {
        T::append_to(self, buf);
    }
}

impl BufferFmt for char {
    fn append_to(&self, buf: &mut String) {
        buf.push(*self);
    }
}

impl BufferFmt for str {
    fn append_to(&self, buf: &mut String) {
        buf.push_str(self);
    }
}

impl BufferFmt for String {
    fn append_to(&self, buf: &mut String) {
        buf.push_str(self);
    }
}

impl BufferFmt for Arguments<'_> {
    fn append_to(&self, buf: &mut String) {
        buf.write_fmt(*self).unwrap();
    }
}

/// Similar to `write!(dest, "{src:?}")`, but only escapes the strictly needed characters,
/// and without the surrounding `"…"` quotation marks.
fn string_escape(dest: &mut String, src: &str) {
    // SAFETY: we will only push valid str slices
    let dest = unsafe { dest.as_mut_vec() };
    let src = src.as_bytes();
    let mut last = 0;

    // According to <https://doc.rust-lang.org/reference/tokens.html#string-literals>, every
    // character is valid except `" \ IsolatedCR`. We don't test if the `\r` is isolated or not,
    // but always escape it.
    for x in memchr::memchr3_iter(b'\\', b'"', b'\r', src) {
        dest.extend(&src[last..x]);
        dest.extend(match src[x] {
            b'\\' => br"\\",
            b'\"' => br#"\""#,
            _ => br"\r",
        });
        last = x + 1;
    }
    dest.extend(&src[last..]);
}
