use askama::Template;

#[derive(Template)]
#[template(source = r#"
{% block bla %}
{% extends "bla.txt" %}
{% endblock %}
"#, ext = "txt")]
struct MyTemplate1;

#[derive(Template)]
#[template(source = r#"
{% block bla %}
{% macro bla() %}
{% endmacro %}
{% endblock %}
"#, ext = "txt")]
struct MyTemplate2;

#[derive(Template)]
#[template(source = r#"
{% block bla %}
{% import "bla.txt" as blue %}
{% endblock %}
"#, ext = "txt")]
struct MyTemplate3;

fn main() {
}
