use askama::Template;

#[derive(Template)]
#[template(ext = "txt", in_doc = true)]
/// Some documentation
///
/// ```html,askama
/// {% if true %}
///     {% fail %}
/// {% endif %}
/// ```
struct SyntaxError;

#[derive(Template)]
#[template(ext = "txt", in_doc = true)]
/// `````
/// ```askama
/// {{bla}}
/// ```
/// `````
struct BlockInBlock;

#[derive(Template)]
#[template(ext = "txt")]
/// Some documentation
///
/// ```html,askama
/// Hello.
/// ```
struct InDocMissing;

#[derive(Template)]
#[template(ext = "txt", in_doc)]
/// Some documentation
///
/// ```html,askama
/// Hello.
/// ```
struct InDocEmpty;

#[derive(Template)]
#[template(ext = "txt", in_doc = false)]
/// Some documentation
///
/// ```html,askama
/// Hello.
/// ```
struct InDocWrong;

#[derive(Template)]
#[template(ext = "txt", in_doc = "yes")]
/// Some documentation
///
/// ```html,askama
/// Hello.
/// ```
struct InDocWrongType;

#[derive(Template)]
#[template(ext = "txt", in_doc = true)]
enum NoDocForEnum {
    Yes,
    No,
}

#[derive(Template)]
#[template(ext = "txt", in_doc = true)]
union NoDocForUnion {
    u: u8,
    i: i8,
}

fn main() {}
