use askama::Template;

#[derive(Template)]
#[template(ext = "txt", source = "{{ a & b }}")]
struct BitAnd {
    a: u32,
    b: u32,
}

#[derive(Template)]
#[template(ext = "txt", source = "{{ a bitand b }}")]
struct BitAndIso646 {
    a: u32,
    b: u32,
}

#[derive(Template)]
#[template(ext = "txt", source = "{{ a bitor b }}")]
struct BitOrIso646 {
    a: u32,
    b: u32,
}

#[derive(Template)]
#[template(ext = "txt", source = "{{ a ^ b }}")]
struct Xor {
    a: u32,
    b: u32,
}

#[derive(Template)]
#[template(ext = "txt", source = "{{ a xor b }} {{ c }}")]
struct XorIso646 {
    a: u32,
    b: u32,
}

fn main() {
}
