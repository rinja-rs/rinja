use rinja::Template;

#[derive(Template)]
enum CratePathOnVariant {
    #[template(ext = "txt", source = "🫨", rinja = rinja)]
    Variant,
}

#[derive(Template)]
enum CratePathOnVariants {
    #[template(ext = "txt", source = "🫏", rinja = rinja)]
    Variant1,
    #[template(ext = "txt", source = "🪿", rinja = rinja)]
    Variant2,
}

#[derive(Template)]
#[template(ext = "txt", source = "🪼", rinja = rinja)]
enum CratePathOnBoth {
    #[template(ext = "txt", source = "🪻", rinja = rinja)]
    Variant,
}

#[derive(Template)]
#[template(ext = "txt", source = "🫛", rinja = rinja)]
enum CratePathOnAll {
    #[template(ext = "txt", source = "🫠", rinja = rinja)]
    Variant1,
    #[template(ext = "txt", source = "🧌", rinja = rinja)]
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
