use rinja::Template;

#[derive(Template)]
#[template(source = "{% match true %}{% endmatch %}", ext = "html")]
struct EmptyMatch;

fn main() {}
