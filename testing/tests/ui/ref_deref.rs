use askama::Template;

#[derive(Template)]
#[template(source = "{% let *x = 2 %}", ext = "html")]
struct WrongRefDeref;

fn main() {
}
