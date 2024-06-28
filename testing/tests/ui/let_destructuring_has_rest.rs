use rinja::Template;

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

fn main() {}
