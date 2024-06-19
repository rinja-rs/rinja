use rinja::Template;

#[derive(Template)]
#[template(ext = "txt", source = "{{a|lower}}")]
struct Lower1<'a> {
    a: &'a str,
}

#[derive(Template)]
#[template(ext = "txt", source = "{{a |lower}}")]
struct Lower2<'a> {
    a: &'a str,
}

#[derive(Template)]
#[template(ext = "txt", source = "{{a| lower}}")]
struct Lower3<'a> {
    a: &'a str,
}

#[derive(Template)]
#[template(ext = "txt", source = "{{a | lower}}")]
struct Lower4<'a> {
    a: &'a str,
}

fn main() {
}
