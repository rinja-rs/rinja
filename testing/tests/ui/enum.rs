use askama::Template;

#[derive(Template)]
enum CratePathOnVariant {
    #[template(ext = "txt", source = "🫨", askama = askama)]
    Variant,
}

#[derive(Template)]
enum CratePathOnVariants {
    #[template(ext = "txt", source = "🫏", askama = askama)]
    Variant1,
    #[template(ext = "txt", source = "🪿", askama = askama)]
    Variant2,
}

#[derive(Template)]
#[template(ext = "txt", source = "🪼", askama = askama)]
enum CratePathOnBoth {
    #[template(ext = "txt", source = "🪻", askama = askama)]
    Variant,
}

#[derive(Template)]
#[template(ext = "txt", source = "🫛", askama = askama)]
enum CratePathOnAll {
    #[template(ext = "txt", source = "🫠", askama = askama)]
    Variant1,
    #[template(ext = "txt", source = "🧌", askama = askama)]
    Variant2,
}

#[derive(Template)]
#[template(
    ext = "txt",
    source = "
        {%- block a -%} a {%- endblock -%}
        {%- block b -%} b {%- endblock -%}
        {#- no block c -#}
        {%- block d -%} d {%- endblock -%}
    ",
)]
enum MissingBlockName {
    #[template(block = "a")]
    A,
    #[template(block = "b")]
    B,
    #[template(block = "c")]
    C,
    #[template(block = "d")]
    D,
}

fn main() {}
