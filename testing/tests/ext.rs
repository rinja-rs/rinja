use rinja::Template;

#[test]
fn test_path_ext_html() {
    #[derive(Template)]
    #[template(path = "foo.html")]
    struct PathHtml;

    let t = PathHtml;
    assert_eq!(t.render().unwrap(), "foo.html");
    assert_eq!(PathHtml::EXTENSION, Some("html"));
}

#[test]
fn test_path_ext_jinja() {
    #[derive(Template)]
    #[template(path = "foo.jinja")]
    struct PathJinja;

    let t = PathJinja;
    assert_eq!(t.render().unwrap(), "foo.jinja");
    assert_eq!(PathJinja::EXTENSION, Some("jinja"));
}

#[test]
fn test_path_ext_html_jinja() {
    #[derive(Template)]
    #[template(path = "foo.html.jinja")]
    struct PathHtmlJinja;

    let t = PathHtmlJinja;
    assert_eq!(t.render().unwrap(), "foo.html.jinja");
    assert_eq!(PathHtmlJinja::EXTENSION, Some("html"));
}

#[test]
fn test_path_ext_html_and_ext_txt() {
    #[derive(Template)]
    #[template(path = "foo.html", ext = "txt")]
    struct PathHtmlAndExtTxt;

    let t = PathHtmlAndExtTxt;
    assert_eq!(t.render().unwrap(), "foo.html");
    assert_eq!(PathHtmlAndExtTxt::EXTENSION, Some("txt"));
}

#[test]
fn test_path_ext_jinja_and_ext_txt() {
    #[derive(Template)]
    #[template(path = "foo.jinja", ext = "txt")]
    struct PathJinjaAndExtTxt;

    let t = PathJinjaAndExtTxt;
    assert_eq!(t.render().unwrap(), "foo.jinja");
    assert_eq!(PathJinjaAndExtTxt::EXTENSION, Some("txt"));
}

#[test]
fn test_path_ext_html_jinja_and_ext_txt() {
    #[derive(Template)]
    #[template(path = "foo.html.jinja", ext = "txt")]
    struct PathHtmlJinjaAndExtTxt;

    let t = PathHtmlJinjaAndExtTxt;
    assert_eq!(t.render().unwrap(), "foo.html.jinja");
    assert_eq!(PathHtmlJinjaAndExtTxt::EXTENSION, Some("txt"));
}

#[test]
fn test_path_ext_rinja() {
    #[derive(Template)]
    #[template(path = "foo.rinja")]
    struct PathRinja;

    let t = PathRinja;
    assert_eq!(t.render().unwrap(), "foo.rinja");
    assert_eq!(PathRinja::EXTENSION, Some("rinja"));
}

#[test]
fn test_path_ext_html_rinja() {
    #[derive(Template)]
    #[template(path = "foo.html.rinja")]
    struct PathHtmlRinja;

    let t = PathHtmlRinja;
    assert_eq!(t.render().unwrap(), "foo.html.rinja");
    assert_eq!(PathHtmlRinja::EXTENSION, Some("html"));
}
