use askama::Template;

#[derive(Template)]
#[template(
    source = r#"{{ "a"|escape(b"none") }}"#,
    ext = "txt",
)]
struct BadEscapeKind;

#[derive(Template)]
#[template(
    source = r#"{{ "a"|escape(c"none") }}"#,
    ext = "txt",
)]
struct BadEscapeKind2;

fn main() {}
