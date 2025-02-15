use askama::Template;

#[derive(Template)]
#[template(
    ext = "html",
    source = r#"In LaTeX you write `{{text}}` like `{{text|escape("latex")}}`."#,
)]
struct LocalEscaper<'a> {
    text: &'a str,
}

#[derive(Template)]
#[template(
    ext = "tex",
    source = r#"In HTML you write `{{text}}` like `{{text|escape("html")}}`."#,
)]
struct GlobalEscaper<'a> {
    text: &'a str,
}

#[derive(Template)]
#[template(path = "latex-file.tex")]
struct NoSuchEscaper;

fn main() {
}
