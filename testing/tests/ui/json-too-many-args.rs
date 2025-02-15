#![cfg(feature = "serde_json")]

use askama::Template;

#[derive(Template)]
#[template(ext = "txt", source = "{{ 1|json(2, 3) }}")]
struct OneTwoThree;

fn main() {
}
