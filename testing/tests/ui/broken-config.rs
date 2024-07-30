use rinja::Template;

#[derive(Template)]
#[template(source = "", ext = "txt", config = "no-such-config.toml")]
struct NoSuchConfig;

#[derive(Template)]
#[template(source = "", ext = "txt", config = "folder-config.toml")]
struct FolderConfig;

#[derive(Template)]
#[template(source = "", ext = "txt", config = "delim-clash.toml")]
struct DelimClash;

#[derive(Template)]
#[template(source = "", ext = "txt", config = "delim-too-short.toml")]
struct DelimTooShort;

fn main() {}
