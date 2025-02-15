use askama::Template;

#[derive(Template)]
#[template(source = "{% for _ in 1..=10 %}{% end %}", ext = "txt")]
struct For;

#[derive(Template)]
#[template(source = "{% macro test() %}{% end %}", ext = "txt")]
struct Macro;

#[derive(Template)]
#[template(source = "{% filter upper %}{% end %}", ext = "txt")]
struct Filter;

#[derive(Template)]
#[template(source = "{% match () %}{% when () %}{% end %}", ext = "txt")]
struct Match;

#[derive(Template)]
#[template(source = "{% block body %}{% end %}", ext = "txt")]
struct Block;

#[derive(Template)]
#[template(source = "{% if true %}{% end %}", ext = "txt")]
struct If;

#[derive(Template)]
#[template(source = "{% if true %}{% endfor %}", ext = "txt")]
struct IfFor;

fn main() {}
