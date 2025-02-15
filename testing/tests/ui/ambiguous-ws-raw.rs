use askama::Template;

#[derive(Template)]
#[template(source = r#"X{#-#}Y"#, ext = "html")]
struct Suppress;

#[derive(Template)]
#[template(source = r#"X{#+#}Y"#, ext = "html")]
struct Preserve;

#[derive(Template)]
#[template(source = r#"X{#~#}Y"#, ext = "html")]
struct Minimize;

fn main() {
}
