use rinja::Template;

#[derive(Template)]
#[template(source = r#"{% let __rinja_var %}"#, ext = "html")]
struct Define;

#[derive(Template)]
#[template(source = r#"{% let __rinja_var = "var" %}"#, ext = "html")]
struct Assign;

fn main() {
}
