use askama::Template;

struct X {
    a: u32,
    b: u32,
}

#[derive(Template)]
#[template(source = "
{%- if let X { a, .., } = x -%}hello {{ a }}{%- endif -%}
", ext = "html")]
struct T1 {
    x: X,
}

#[derive(Template)]
#[template(source = "
{%- if let X { a .. } = x -%}hello {{ a }}{%- endif -%}
", ext = "html")]
struct T2 {
    x: X,
}

#[derive(Template)]
#[template(source = "
{%- if let X { a, 1 } = x -%}hello {{ a }}{%- endif -%}
", ext = "html")]
struct T3 {
    x: X,
}

#[derive(Template)]
#[template(source = "
{%- if let X { a, .., b } = x -%}hello {{ a }}{%- endif -%}
", ext = "html")]
struct T4 {
    x: X,
}

#[derive(Template)]
#[template(source = "
{%- if let X { .., b } = x -%}hello {{ a }}{%- endif -%}
", ext = "html")]
struct T5 {
    x: X,
}

fn main() {}
