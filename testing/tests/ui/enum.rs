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

fn main() {}
