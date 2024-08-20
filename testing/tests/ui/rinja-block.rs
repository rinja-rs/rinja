use rinja::Template;

#[derive(Template)]
#[template(ext = "txt", in_doc = true)]
/// Some documentation
///
/// ```html,rinja
/// {% if true %}
///     {% fail %}
/// {% endif %}
/// ```
struct SyntaxError;

#[derive(Template)]
#[template(ext = "txt", in_doc = true)]
/// `````
/// ```rinja
/// {{bla}}
/// ```
/// `````
struct BlockInBlock;

#[derive(Template)]
#[template(ext = "txt")]
/// Some documentation
///
/// ```html,rinja
/// Hello.
/// ```
struct InDocMissing;

#[derive(Template)]
#[template(ext = "txt", in_doc)]
/// Some documentation
///
/// ```html,rinja
/// Hello.
/// ```
struct InDocEmpty;

#[derive(Template)]
#[template(ext = "txt", in_doc = false)]
/// Some documentation
///
/// ```html,rinja
/// Hello.
/// ```
struct InDocWrong;

#[derive(Template)]
#[template(ext = "txt", in_doc = "yes")]
/// Some documentation
///
/// ```html,rinja
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
