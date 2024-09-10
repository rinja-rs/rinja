use rinja::Template;

#[derive(Template)]
#[template(in_doc = true, ext = "html")]
/// ```rinja
/// {% end %}
/// ```
struct UnexpectedEnd;

#[derive(Template)]
#[template(in_doc = true, ext = "html")]
/// ```rinja
/// {% for i in 0..10 %}
///     i = {{i}}
/// {% elif %}
///     what?
/// {% endfor %}
/// ```
struct UnexpectedElif;

#[derive(Template)]
#[template(in_doc = true, ext = "html")]
/// ```rinja
/// {% block meta %}
///     then
/// {% else %}
///     else
/// {% endblock meta %}
/// ```
struct UnexpectedElse;

#[derive(Template)]
#[template(in_doc = true, ext = "html")]
/// ```rinja
/// {% when condition %}
///     true
/// {% endwhen %}
/// ```
struct UnexpectedWhen;

#[derive(Template)]
#[template(in_doc = true, ext = "html")]
/// ```rinja
/// {% let var %}value{% endlet %}
/// ```
struct UnexpectedEndLet;

#[derive(Template)]
#[template(in_doc = true, ext = "html")]
/// ```rinja
/// {% syntax error %}
/// ```
struct Unexpected;

fn main() {}
