use askama::Template;

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

#[derive(Template)]
#[template(source = "<+a+> and <+b+>", config = "operator-plus-config.toml", syntax = "plus", ext = "txt")]
struct PlusOperator;

#[derive(Template)]
#[template(source = "<)a(> and <)b(>", config = "operator-paren-config.toml", syntax = "paren", ext = "txt")]
struct ParenOperator;

fn main() {}
