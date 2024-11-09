use rinja::Template;

// This test ensures that rust macro calls in `let`/`set` statements are not prepended with `&`.
#[test]
fn let_macro() {
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

    let template = A { y: false };
    assert_eq!(template.render().unwrap(), "blob")
}
