use askama::Template;

#[derive(Template)]
#[template(ext = "txt", source = "{% if cond %-}{% endif %}")]
struct BlockSuppress {
    cond: bool,
}

#[derive(Template)]
#[template(ext = "txt", source = "{% if cond %+}{% endif %}")]
struct BlockPreserve {
    cond: bool,
}

#[derive(Template)]
#[template(ext = "txt", source = "{% if cond %~}{% endif %}")]
struct BlockFold {
    cond: bool,
}

#[derive(Template)]
#[template(ext = "txt", source = "{% if cond %}{% endif %-}")]
struct BlockSuppress2 {
    cond: bool,
}

#[derive(Template)]
#[template(ext = "txt", source = "{% if cond %}{% endif %+}")]
struct BlockPreserve2 {
    cond: bool,
}

#[derive(Template)]
#[template(ext = "txt", source = "{% if cond %}{% endif %~}")]
struct BlockFold2 {
    cond: bool,
}

fn main() {}
