use askama::Template;

#[derive(Template)]
#[template(
    ext = "html",
    source = "{{ input|pluralize }}",
)]
struct Pluralize {
    input: &'static str,
}

fn main() {}
