use askama::Template;

#[derive(Template)]
#[template(
    source = "
        {% macro one %}{% call one %}{% endmacro %}
        {% call one %}
    ",
    ext = "html"
)]
struct Direct;

#[derive(Template)]
#[template(
    source = "
        {% macro one %}{% call two %}{% endmacro %}
        {% macro two %}{% call three %}{% endmacro %}
        {% macro three %}{% call four %}{% endmacro %}
        {% macro four %}{% call five %}{% endmacro %}
        {% macro five %}{% call one %}{% endmacro %}
        {% call one %}
    ",
    ext = "html"
)]
struct Indirect;

#[derive(Template)]
#[template(
    source = r#"
        {% import "macro-recursion-1.html" as next %}
        {% macro some_macro %}
            {% call next::some_macro %}
        {% endmacro %}
        {% call some_macro %}
    "#,
    ext = "html"
)]
struct AcrossImports;

fn main() {
}
