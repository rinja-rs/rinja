use askama::Template;

#[derive(Template)]
#[template(path = "fuzzed-recursion-mul-deref.txt")]
struct Filtered {
    s: &'static str,
}

fn main() {}
