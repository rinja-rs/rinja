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

#[derive(Template)]
#[template(
    source = r#"{% if x == b"hi".as_slice() %}bc{% endif %}
{%- if c"a".to_bytes_with_nul() == b"a\0" %} hoy{% endif %}"#,
    ext = "txt"
)]
struct ExprStr {
    x: &'static [u8],
}

#[test]
fn test_prefix_str_literal_in_expr() {
    let t = ExprStr { x: b"hi" };
    assert_eq!(t.render().unwrap(), "bc hoy");
}

#[derive(Template)]
#[template(
    source = r#"{% if let Some(b"hi") = Some(data) %}bc{% endif %}
{%- if let x = c"hi" %} hoy{% endif %}"#,
    ext = "txt"
)]
struct TargetStr {
    data: [u8; 2],
}

#[test]
fn test_prefix_str_literal_in_target() {
    let t = TargetStr { data: *b"hi" };
    assert_eq!(t.render().unwrap(), "bc hoy");
}
