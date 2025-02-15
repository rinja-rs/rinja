use askama::Template;

#[derive(Template)]
#[template(
    ext = "html",
    source = r#"{% if x.y is defined %}{% endif %}"#,
)]
struct A;

#[derive(Template)]
#[template(
    ext = "html",
    source = r#"{% if true is defined %}{% endif %}"#,
)]
struct B;

#[derive(Template)]
#[template(
    ext = "html",
    source = r#"{% if true is %}{% endif %}"#,
)]
struct C;

#[derive(Template)]
#[template(
    ext = "html",
    source = r#"{% if x is %}{% endif %}"#,
)]
struct D;

#[derive(Template)]
#[template(
    ext = "html",
    source = r#"{% if x is blue %}{% endif %}"#,
)]
struct E;

#[derive(Template)]
#[template(
    ext = "html",
    source = r#"{% if x is blue.red %}{% endif %}"#,
)]
struct F;

fn main() {
}
