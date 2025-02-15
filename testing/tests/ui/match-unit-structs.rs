use askama::Template;

enum Greeting {
    Hello,
    Hey,
    Hi,
}

#[derive(Template)]
#[template(
    ext = "txt",
    source = r#"
{%- match greeting -%}
    {%- when Hello -%} Hello!
    {%- when Hey -%} Hey!
    {%- when Hi -%} Hi!
{%- endmatch -%}"#
)]
struct Greeter {
    greeting: Greeting,
}

fn main() {
}
