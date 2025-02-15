use askama::Template;

#[derive(Template)]
#[template(ext = "txt", source = r#"{% include "a_file_that_is_actually_a_folder.html" %}"#)]
struct YouCannotIncludeFolders;

fn main() {
}
