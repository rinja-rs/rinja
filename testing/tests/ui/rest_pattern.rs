use askama::Template;

// Checking that we can't use `..` more than once.
#[derive(Template)]
#[template(
    source = r#"
{%- if let [1, 2, who @ .., 4, ..] = [1, 2, 3, 4] -%}
{%- endif -%}
"#,
    ext = "txt"
)]
struct Err1;

#[derive(Template)]
#[template(
    source = r#"
{%- if let (.., 1, 2, .., 4) = (1, 2, 3, 4) -%}
{%- endif -%}
"#,
    ext = "txt"
)]
struct Err2;

// This code doesn't make sense but the goal is to ensure that you can't
// use `..` in a struct more than once.
#[derive(Template)]
#[template(
    source = r#"
{%- if let Cake { .., a, .. } = [1, 2, 3, 4] -%}
{%- endif -%}
"#,
    ext = "txt"
)]
struct Err3;

// Now checking we can't use `@ ..` in tuples and structs.
#[derive(Template)]
#[template(
    source = r#"
{%- if let (1, 2, who @ .., 4) = (1, 2, 3, 4) -%}
{%- endif -%}
"#,
    ext = "txt"
)]
struct Err4;

// This code doesn't make sense but the goal is to ensure that you can't
// use `@ ..` in a struct so here we go.
#[derive(Template)]
#[template(
    source = r#"
{%- if let Cake { a, who @ .. } = [1, 2, 3, 4] -%}
{%- endif -%}
"#,
    ext = "txt"
)]
struct Err5;

fn main() {
}
