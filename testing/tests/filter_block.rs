use rinja::Template;

#[derive(Template)]
#[template(
    source = r#"{% filter lower %}
    {{ t }} / HELLO / {{ u }}
{% endfilter %}

{{ u|lower }}
"#,
    ext = "html"
)]
struct A<T, U = u8>
where
    T: std::fmt::Display,
    U: std::fmt::Display,
{
    t: T,
    u: U,
}

#[test]
fn filter_block_basic() {
    let template = A { t: "a", u: "B" };
    assert_eq!(template.render().unwrap(), "\n    a / hello / b\n\n\nb\n")
}

// This test ensures that we don't have variable shadowing when we have more than one
// filter block at the same location.
#[derive(Template)]
#[template(
    source = r#"{% filter lower %}
    {{ t }} / HELLO / {{ u }}
{% endfilter %}

{% filter upper %}
{{ u }} + TaDaM + {{ t }}
{% endfilter %}

{% filter lower %}
    {{ t }} - CHECK - {{ t }}
{% endfilter %}

{{ u|upper }}"#,
    ext = "html"
)]
struct B<T, U = u8>
where
    T: std::fmt::Display,
    U: std::fmt::Display,
{
    t: T,
    u: U,
}

#[test]
fn filter_block_shadowing() {
    let template = B { t: "a", u: "B" };
    assert_eq!(
        template.render().unwrap(),
        r#"
    a / hello / b



B + TADAM + A



    a - check - a


B"#
    );
}

// This test ensures that whitespace control is correctly handled.
#[derive(Template)]
#[template(
    source = r#"{% filter lower -%}
    {{ t }} / HELLO / {{ u }}
{% endfilter %}

{%- filter upper -%}
{{ u }} + TaDaM + {{ t }}
{%- endfilter -%}

++b"#,
    ext = "html"
)]
struct C<T, U = u8>
where
    T: std::fmt::Display,
    U: std::fmt::Display,
{
    t: T,
    u: U,
}

#[test]
fn filter_block_whitespace_control() {
    let template = C { t: "a", u: "B" };
    assert_eq!(
        template.render().unwrap(),
        r#"a / hello / b
B + TADAM + A++b"#
    );
}

// This test ensures that HTML escape is correctly handled.
#[derive(Template)]
#[template(source = r#"{% filter lower %}<block>{% endfilter %}"#, ext = "html")]
struct D;

#[test]
fn filter_block_html_escape() {
    let template = D;
    assert_eq!(template.render().unwrap(), r#"&#60;block&#62;"#);
}

// This test ensures that it is not escaped if it is not HTML.
#[derive(Template)]
#[template(source = r#"{% filter lower %}<block>{% endfilter %}"#, ext = "txt")]
struct E;

#[test]
fn filter_block_not_html_escape() {
    let template = E;
    assert_eq!(template.render().unwrap(), r#"<block>"#);
}

// This test checks that the filter chaining is working as expected.
#[derive(Template)]
#[template(
    source = r#"{% filter lower|indent(2)|capitalize -%}
HELLO
{{v}}
{%- endfilter %}"#,
    ext = "txt"
)]
struct F {
    v: &'static str,
}

#[test]
fn filter_block_chaining() {
    let template = F { v: "pIKA" };
    assert_eq!(template.render().unwrap(), "Hello\n  pika");
}

// This test checks that the filter chaining is working as expected when ending
// followed by whitespace character(s).
#[derive(Template)]
#[template(
    source = r#"{% filter lower|indent(2) -%}
HELLO
{{v}}
{%- endfilter %}

{% filter lower|indent(2)   -%}
HELLO
{{v}}
{%- endfilter %}

{% filter lower|indent(2) %}
HELLO
{{v}}
{%- endfilter %}

{% filter lower|indent(2)   %}
HELLO
{{v}}
{%- endfilter %}"#,
    ext = "txt"
)]
struct G {
    v: &'static str,
}

#[test]
fn filter_block_chaining_paren_followed_by_whitespace() {
    let template = G { v: "pIKA" };
    assert_eq!(
        template.render().unwrap(),
        r#"hello
  pika

hello
  pika


  hello
  pika


  hello
  pika"#
    );
}

#[derive(Template)]
#[template(
    source = r#"{% extends "html-base.html" %}

{%- block body -%}
    <h1>Metadata</h1>
        {% set y = 12 %}

    {% filter wordcount %}
        {%- include "../Cargo.toml" +%}
        y is {{ y }}
    {% endfilter %}
{%- endblock body %}
"#,
    ext = "html"
)]
struct IncludeInFilter;

// This test ensures that `include` are correctly working inside filter blocks and that external
// variables are used correctly.
#[test]
fn filter_block_include() {
    assert_eq!(
        IncludeInFilter.render().unwrap(),
        r#"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta charset="UTF-8">

        <title>Default title</title>
    </head>

    <body class=""><h1>Metadata</h1>
        

    100</body>
</html>"#
    );
}

#[derive(Template)]
#[template(
    source = r#"
{%- filter title %}
    {{- x -}}
    {%- if x == 21 -%}
        X is big
    {%- else -%}
        No clue what X is
    {%- endif %}

    {% if let Some(v) = v -%}
        v is {{ v -}}
    {% endif -%}
{% endfilter -%}
"#,
    ext = "html"
)]
struct ConditionInFilter {
    x: usize,
    v: Option<String>,
}

// This test ensures that `include` are correctly working inside filter blocks and that external
// variables are used correctly.
#[test]
fn filter_block_conditions() {
    let s = ConditionInFilter {
        x: 21,
        v: Some("hoho".to_string()),
    };
    assert_eq!(s.render().unwrap(), "21x Is Big\n\n    V Is Hoho",);
}

// The output of `|upper` is not marked as `|safe`, so the output of `|paragraphbreaks` gets
// escaped. The '&' in the input is is not marked as `|safe`, so it should get escaped, twice.
#[derive(Template)]
#[template(
    source = r#"
    {%- let count = 1 -%}
    {%- let canary = 2 -%}
    {%- filter upper -%}
        {%- let canary = 3 -%}
        [
            {%- for _ in 0..=count %}
                {%~ filter paragraphbreaks -%}
                    {{v}}
                {%~ endfilter -%}
            {%- endfor -%}
        ]
    {%~ endfilter %}{{ canary }}"#,
    ext = "html"
)]
struct NestedFilterBlocks2 {
    v: &'static str,
}

#[test]
fn filter_nested_filter_blocks() {
    let template = NestedFilterBlocks2 {
        v: "Hello &\n\ngoodbye!",
    };
    assert_eq!(
        template.render().unwrap(),
        r#"[
&#60;P&#62;HELLO &#38;#38;&#60;/P&#62;&#60;P&#62;GOODBYE!
&#60;/P&#62;
&#60;P&#62;HELLO &#38;#38;&#60;/P&#62;&#60;P&#62;GOODBYE!
&#60;/P&#62;]
2"#
    );
}

#[derive(Template)]
#[template(
    source = r#"
    {%- filter urlencode|urlencode|urlencode|urlencode -%}
        {{ msg.clone()? }}
    {%~ endfilter %}"#,
    ext = "html"
)]
struct FilterBlockCustomErrors {
    msg: Result<String, String>,
}

#[test]
fn filter_block_custom_errors() {
    let template = FilterBlockCustomErrors {
        msg: Err("🐢".to_owned()),
    };
    assert_eq!(template.render().unwrap_err().to_string(), "🐢");
}
