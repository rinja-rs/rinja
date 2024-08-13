use rinja::Template;

#[derive(Template)]
#[template(ext = "txt")]
/// Some documentation
///
/// ```html,rinja
/// {% if true %}
///     {% fail %}
/// {% endif %}
/// ```
struct SyntaxError;

#[derive(Template)]
#[template(ext = "txt")]
/// `````
/// ```rinja
/// {{bla}}
/// ```
/// `````
struct BlockInBlock;

fn main() {}
