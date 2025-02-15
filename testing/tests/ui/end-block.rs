use askama::Template;

#[derive(Template)]
#[template(
    ext = "html",
    source = "{% if x %}{% if x %}{% endif %}",
)]
struct EndIf {
    x: bool,
}

#[derive(Template)]
#[template(
    ext = "html",
    source = "{% if x %}",
)]
struct EndIf2 {
    x: bool,
}

#[derive(Template)]
#[template(
    ext = "html",
    source = "{% match x %}",
)]
struct EndMatch {
    x: bool,
}

#[derive(Template)]
#[template(
    ext = "html",
    source = "{% for a in x %}",
)]
struct EndFor {
    x: [u32; 2],
}

#[derive(Template)]
#[template(
    ext = "html",
    source = "{% macro bla %}",
)]
struct EndMacro;

#[derive(Template)]
#[template(
    ext = "html",
    source = "{% filter bla %}",
)]
struct EndFilter;

#[derive(Template)]
#[template(
    ext = "html",
    source = "{% block bla %}",
)]
struct EndBlock;

fn main() {
}
