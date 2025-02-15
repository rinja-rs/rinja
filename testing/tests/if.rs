use askama::Template;

#[test]
fn test_if() {
    #[derive(Template)]
    #[template(
        source = r#"{%- if s == "" -%}
empty
{%- else if s == "b" -%}
b
{%- elif s == "c" -%}
c
{%- else -%}
else
{%- endif -%}"#,
        ext = "txt"
    )]
    struct If<'a> {
        s: &'a str,
    }

    assert_eq!(If { s: "" }.render().unwrap(), "empty");
    assert_eq!(If { s: "b" }.render().unwrap(), "b");
    assert_eq!(If { s: "c" }.render().unwrap(), "c");
    assert_eq!(If { s: "d" }.render().unwrap(), "else");
}
