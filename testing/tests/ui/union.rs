use askama::Template;

#[derive(Template)]
#[template(source = "a={{ a }} b={{ b }}", ext = "html")]
union Tmpl {
    a: i32,
    b: u32,
}

fn main() {}
