use rinja::Template;

#[derive(Template)]
#[template(source = "{%- macro thrice(param) -%}
{{ param }}
{%- endmacro -%}

{%- call thrice(2, 3) -%}", ext = "html")]
struct InvalidNumberOfArgs;

#[derive(Template)]
#[template(source = "{%- macro thrice(param, param2) -%}
{{ param }} {{ param2 }}
{%- endmacro -%}

{%- call thrice() -%}", ext = "html")]
struct InvalidNumberOfArgs2;

#[derive(Template)]
#[template(source = "{%- macro thrice() -%}
{%- endmacro -%}

{%- call thrice(1, 2) -%}", ext = "html")]
struct InvalidNumberOfArgs3;

#[derive(Template)]
#[template(source = "{% macro thrice( %}{% endmacro %}", ext = "html")]
struct NoClosingParen1;

#[derive(Template)]
#[template(source = "{% macro thrice(a, b, c %}{% endmacro %}", ext = "html")]
struct NoClosingParen2;

#[derive(Template)]
#[template(source = "{% macro thrice(a, b, c= %}{% endmacro %}", ext = "html")]
struct NoClosingParen3;

#[derive(Template)]
#[template(source = "{% macro thrice(a, b, c = %}{% endmacro %}", ext = "html")]
struct NoClosingParen4;

fn main() {
}
