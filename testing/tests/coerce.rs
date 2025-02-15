use askama::Template;

#[test]
fn test_coerce() {
    #[derive(Template)]
    #[template(path = "if-coerce.html")]
    struct IfCoerceTemplate {
        t: bool,
        f: bool,
    }

    let t = IfCoerceTemplate { t: true, f: false };
    assert_eq!(t.render().unwrap(), "ftftfttftelseifelseif");
}
