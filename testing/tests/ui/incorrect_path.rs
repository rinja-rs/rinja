use rinja::Template;

#[derive(Template)]
#[template(path = "thisdoesnotexist.html")]
struct MyTemplate;

fn main() {
}
