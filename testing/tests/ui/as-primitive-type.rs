use askama::Template;

#[derive(Template)]
#[template(source = r#"{{ 1234 as 4567 }}"#, ext = "html")]
struct A;

#[derive(Template)]
#[template(source = r#"{{ 1234 as ? }}"#, ext = "html")]
struct B;

#[derive(Template)]
#[template(source = r#"{{ 1234 as u1234 }}"#, ext = "html")]
struct C;

#[derive(Template)]
#[template(source = r#"{{ 1234 as core::primitive::u32 }}"#, ext = "html")]
struct D;

#[derive(Template)]
#[template(source = r#"{{ 1234 as int32_t }}"#, ext = "html")]
struct E;

#[derive(Template)]
#[template(source = r#"{{ (1234 + 4 * 12 / 45675445 - 13) as int32_t }}"#, ext = "html")]
struct F;

fn main() {
}
