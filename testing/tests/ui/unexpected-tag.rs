use askama::Template;

#[derive(Template)]
#[template(in_doc = true, ext = "html")]
/// ```askama
/// {% end %}
/// ```
struct UnexpectedEnd;

#[derive(Template)]
#[template(in_doc = true, ext = "html")]
/// ```askama
/// {% for i in 0..10 %}
///     i = {{i}}
/// {% elif %}
///     what?
/// {% endfor %}
/// ```
struct UnexpectedElif;

#[derive(Template)]
#[template(in_doc = true, ext = "html")]
/// ```askama
/// {% block meta %}
///     then
/// {% else %}
///     else
/// {% endblock meta %}
/// ```
struct UnexpectedElse;

#[derive(Template)]
#[template(in_doc = true, ext = "html")]
/// ```askama
/// {% when condition %}
///     true
/// {% endwhen %}
/// ```
struct UnexpectedWhen;

#[derive(Template)]
#[template(in_doc = true, ext = "html")]
/// ```askama
/// {% let var %}value{% endlet %}
/// ```
struct UnexpectedEndLet;

#[derive(Template)]
#[template(in_doc = true, ext = "html")]
/// ```askama
/// {% syntax error %}
/// ```
struct Unexpected;

fn main() {}
