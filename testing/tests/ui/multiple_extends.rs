use askama::Template;

#[derive(Template)]
#[template(source = r#"
{% extends "let.html" %}
{% extends "foo.html" %}
"#, ext = "txt")]
struct MyTemplate4;

fn main() {}
