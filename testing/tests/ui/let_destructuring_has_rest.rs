use rinja::Template;

struct X {
    a: u32,
    b: u32,
}

#[derive(Template)]
#[template(source = "
{%- if let X { a, .., } = x -%}hello {{ a }}{%- endif -%}
", ext = "html")]
struct Y {
    x: X,
}

#[derive(Template)]
#[template(source = "
{%- if let X { a .. } = x -%}hello {{ a }}{%- endif -%}
", ext = "html")]
struct Z {
    x: X,
}

fn main() {}
