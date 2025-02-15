use askama::Template;

#[allow(non_camel_case_types)]
#[derive(Template)]
#[template(
    ext = "html",
    source = "{{ 0x0x }}",
)]
struct IntSuffix;

#[derive(Template)]
#[template(
    ext = "html",
    source = "{{ 0.0_f127 }}",
)]
struct FloatSuffix;

#[derive(Template)]
#[template(
    ext = "html",
    source = "{{ 654u321 }}",
)]
struct EitherSuffix;

fn main() {
}
