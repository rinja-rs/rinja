#![no_implicit_prelude]

use ::askama::Template;

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate<'a> {
    name: &'a str,
}

#[test]
fn main() {
    let hello = HelloTemplate { name: "world" };
    ::std::assert_eq!("Hello, world!", hello.render().unwrap());
}
