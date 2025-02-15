use askama::Template;

#[derive(Template)]
#[template(source = r#"{% let x = [_] %}"#, ext = "html")]
struct UnderscoreErr1;

#[derive(Template)]
#[template(source = r#"{% if (_ + 12) != 0 %}{% endif %}"#, ext = "html")]
struct UnderscoreErr2;

#[derive(Template)]
#[template(source = r#"{% if 12 == _ %}{% endif %}"#, ext = "html")]
struct UnderscoreErr3;

#[derive(Template)]
#[template(source = r#"{% match _ %}{% endmatch %}"#, ext = "html")]
struct UnderscoreErr4;

fn main() {}
