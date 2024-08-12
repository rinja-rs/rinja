use rinja::Template;

#[derive(Template)]
#[template(ext = "txt")]
/// Some documentation
///
/// ```html,rinja
/// <h1>No terminator</h1>
struct Unterminated;

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

fn main() {}
