use rinja::Template;

#[derive(Template)]
#[template(
    ext = "html",
    source = r#"{% if let Ok(x) = "a"|value %}{% endif %}"#,
)]
struct A;

#[derive(Template)]
#[template(
    ext = "html",
    source = r#"{% if let Ok(x) = "a"|value::<u8, u8> %}{% endif %}"#,
)]
struct B;

#[derive(Template)]
#[template(
    ext = "html",
    source = r#"{% if let Ok(x) = rinja::get_value("a") %}{% endif %}"#,
)]
struct C;

#[derive(Template)]
#[template(
    ext = "html",
    source = r#"{% if let Ok(x) = rinja::get_value::<u8, u8>("a") %}{% endif %}"#,
)]
struct D;

#[derive(Template)]
#[template(
    ext = "html",
    source = r#"{% if let Ok(x) = rinja::get_value::<u8>() %}{% endif %}"#,
)]
struct E;

#[derive(Template)]
#[template(
    ext = "html",
    source = r#"{% if let Ok(x) = rinja::get_value::<u8>("a", "b") %}{% endif %}"#,
)]
struct F;

fn main() {}
