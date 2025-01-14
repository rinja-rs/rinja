use std::any::Any;
use std::collections::HashMap;

use rinja::{Template, Values};

#[test]
fn test_values() {
    #[derive(Template)]
    #[template(
        source = r#"{% if let Ok(bla) = VALUES.get_value::<u32>("a") %}{{bla}}{% endif %}"#,
        ext = "txt"
    )]
    struct V;

    let mut values: HashMap<String, Box<dyn Any>> = HashMap::default();

    values.add_value("a", 12u32);
    assert_eq!(V.render_with_values(&values).unwrap(), "12");

    values.add_value("a", false);
    assert_eq!(V.render_with_values(&values).unwrap(), "");
}

#[test]
fn test_values2() {
    #[derive(Template)]
    #[template(
        source = r#"{% if let Ok(bla) = VALUES.get_value::<&str>("a") %}{{bla}}{% endif -%}
{% if let Ok(bla) = VALUES.get_value::<bool>("b") %} {{bla}}{% endif %}"#,
        ext = "txt"
    )]
    struct V;

    let mut values: HashMap<String, Box<dyn Any>> = HashMap::default();

    values.add_value("a", "hey");
    values.add_value("b", false);

    assert_eq!(V.render_with_values(&values).unwrap(), "hey false");
}
