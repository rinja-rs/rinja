use rinja::Template;

#[derive(Template)]
#[template(source = "{% if x == b'a' %}bc{% endif %}", ext = "txt")]
struct Expr {
    x: u8,
}

#[test]
fn test_prefix_char_literal_in_expr() {
    let t = Expr { x: b'a' };
    assert_eq!(t.render().unwrap(), "bc");
}

#[derive(Template)]
#[template(
    source = "{% if let Some(b'a') = Some(b'a') %}bc{% endif %}
{%- if data == [b'h', b'i'] %} hoy{% endif %}",
    ext = "txt"
)]
struct Target {
    data: &'static [u8],
}

#[test]
fn test_prefix_char_literal_in_target() {
    let t = Target { data: b"hi" };
    assert_eq!(t.render().unwrap(), "bc hoy");
}
