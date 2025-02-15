use askama::Template;

#[test]
fn test_rest() {
    #[derive(Template)]
    #[template(
        source = r#"
{%- if let [1, 2, who @ .., 4] = [1, 2, 3, 4] -%}
111> {{"{:?}"|format(who)}}
{%- endif -%}
{%- if let [who @ .., 4] = [1, 2, 3, 4] -%}
222> {{"{:?}"|format(who)}}
{%- endif -%}
{%- if let [1, who @ ..] = [1, 2, 3, 4] -%}
333> {{"{:?}"|format(who)}}
{%- endif -%}
"#,
        ext = "txt"
    )]
    struct Rest;

    assert_eq!(
        Rest.render().unwrap(),
        "111> [3]222> [1, 2, 3]333> [2, 3, 4]"
    );
}
