use rinja::{Template, Values};

#[test]
fn test_values() {
    #[derive(Template)]
    #[template(
        source = r#"{% if let Ok(bla) = VALUES.get::<u32>("a") %}{{bla}}{% endif %}"#,
        ext = "txt"
    )]
    struct V;

    let mut values = Values::default();

    values.add("a", 12u32);
    assert_eq!(V.render_with_values(&values).unwrap(), "12");

    values.add("a", false);
    assert_eq!(V.render_with_values(&values).unwrap(), "");
}

#[test]
fn test_values2() {
    #[derive(Template)]
    #[template(
        source = r#"{% if let Ok(bla) = VALUES.get::<&str>("a") %}{{bla}}{% endif -%}
{% if let Ok(bla) = VALUES.get::<bool>("b") %} {{bla}}{% endif %}"#,
        ext = "txt"
    )]
    struct V;

    let mut values = Values::default();

    values.add("a", "hey");
    values.add("b", false);

    assert_eq!(V.render_with_values(&values).unwrap(), "hey false");
}
