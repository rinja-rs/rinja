use askama::Template;

#[derive(Template)]
#[template(source = r#"{{ a~b }}"#, ext = "txt")]
struct NoSpaces {
    a: u32,
    b: u32,
}

#[derive(Template)]
#[template(source = r#"{{ a ~b }}"#, ext = "txt")]
struct OnlyFront {
    a: u32,
    b: u32,
}

#[derive(Template)]
#[template(source = r#"{{ a~ b }}"#, ext = "txt")]
struct OnlyBack {
    a: u32,
    b: u32,
}

#[derive(Template)]
#[template(source = r#"{{~a~b~}}"#, ext = "txt")]
struct NoSpaces2 {
    a: u32,
    b: u32,
}

#[derive(Template)]
#[template(source = r#"{{~a ~b~}}"#, ext = "txt")]
struct OnlyFront2 {
    a: u32,
    b: u32,
}

#[derive(Template)]
#[template(source = r#"{{~a~ b~}}"#, ext = "txt")]
struct OnlyBack2 {
    a: u32,
    b: u32,
}

#[derive(Template)]
#[template(source = r#"{{ a ~ }}"#, ext = "txt")]
struct MissingLhs {
    a: u32,
}

#[derive(Template)]
#[template(source = r#"{{ ~ b }}"#, ext = "txt")]
struct MissingRhs {
    b: u32,
}

#[derive(Template)]
#[template(source = r#"{{~a ~~}}"#, ext = "txt")]
struct MissingLhs2 {
    a: u32,
}

#[derive(Template)]
#[template(source = r#"{{~~ b~}}"#, ext = "txt")]
struct MissingRhs2 {
    b: u32,
}

fn main() {
}
