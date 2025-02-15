use askama::Template;

#[derive(Template)]
#[template(
    source = r#"{% extends "html-base.html" %}

{%- block body -%}
    <h1>Metadata</h1>

    {% filter wordcount %}
        {% block title %}New title{% endblock %}
        a b
    {% endfilter %}
{%- endblock body %}
"#,
    ext = "html"
)]
struct BlockInFilter;

fn main() {
}
