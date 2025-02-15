use askama::Template;

#[derive(Template)]
#[template(path = "transclude-there.html")]
struct Indirect;

#[derive(Template)]
#[template(source = r#"{% include "transclude-there.html" %}"#, ext = "html")]
struct Direct;

fn main() {}
