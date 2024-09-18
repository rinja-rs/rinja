use rinja::Template;

#[derive(Template)]
#[template(
    source = r#"{%- let x -%}
{%- if y -%}
    {%- let x = String::new() %}
{%- else -%}
    {%- let x = format!("blob") %}
{%- endif -%}
{{ x }}"#,
    ext = "html"
)]
struct A {
    y: bool,
}

// This test ensures that rust macro calls in `let`/`set` statements are not prepended with `&`.
#[test]
fn let_macro() {
    let template = A { y: false };
    assert_eq!(template.render().unwrap(), "blob")
}
