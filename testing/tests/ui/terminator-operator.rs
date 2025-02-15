use askama::Template;

#[derive(Template)]
#[template(source = "<<a>> and <<b>>", config = "issue-128.toml", syntax = "mwe", ext="")]
struct HelloTemplate {
    a: u32,
    b: u32,
}

#[derive(Template)]
#[template(source = "<<a>> and <<b>>", config = "issue-128-2.toml", syntax = "mwe", ext="")]
struct HelloTemplate2 {
    a: u32,
    b: u32,
}

fn main() {}
