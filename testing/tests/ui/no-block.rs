use askama::Template;

#[derive(Template)]
#[template(
    ext = "txt",
    source = "{% block not_a %}{% endblock %}",
    block = "a"
)]
struct SourceTemplate;

#[derive(Template)]
#[template(path = "no-block.txt", block = "a")]
struct PathTemplate;

#[derive(Template)]
#[template(path = "no-block-with-include.txt", block = "a")]
struct NoBlockWithInclude;

#[derive(Template)]
#[template(path = "no-block-with-include-times-2.txt", block = "a")]
struct NoBlockWithIncludeTimes2;

#[derive(Template)]
#[template(path = "no-block-with-base-template.txt", block = "a")]
struct NoBlockWithBaseTemplate;

fn main() {
}
