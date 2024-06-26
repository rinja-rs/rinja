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
    assert_eq!(template.render().unwrap(), r#"&lt;block&gt;"#);
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
