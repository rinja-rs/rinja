use rinja::Template;

#[test]
fn a() {
    #[derive(Template)]
    #[template(source = "{% if let (a, ..) = abc  %}-{{a}}-{% endif %}", ext = "txt")]
    struct Tmpl {
        abc: (u32, u32, u32),
    }

    assert_eq!(Tmpl { abc: (1, 2, 3) }.to_string(), "-1-");
}

#[test]
fn ab() {
    #[derive(Template)]
    #[template(
        source = "{% if let (a, b, ..) = abc  %}-{{a}}{{b}}-{% endif %}",
        ext = "txt"
    )]
    struct Tmpl {
        abc: (u32, u32, u32),
    }

    assert_eq!(Tmpl { abc: (1, 2, 3) }.to_string(), "-12-");
}

#[test]
fn abc() {
    #[derive(Template)]
    #[template(
        source = "{% if let (a, b, c, ..) = abc  %}-{{a}}{{b}}{{c}}-{% endif %}",
        ext = "txt"
    )]
    struct Tmpl1 {
        abc: (u32, u32, u32),
    }

    assert_eq!(Tmpl1 { abc: (1, 2, 3) }.to_string(), "-123-");

    assert_eq!(Tmpl2 { abc: (1, 2, 3) }.to_string(), "-123-");

    #[derive(Template)]
    #[template(
        source = "{% if let (a, b, c, ..) = abc  %}-{{a}}{{b}}{{c}}-{% endif %}",
        ext = "txt"
    )]
    struct Tmpl2 {
        abc: (u32, u32, u32),
    }

    assert_eq!(Tmpl2 { abc: (1, 2, 3) }.to_string(), "-123-");
}

#[test]
fn bc() {
    #[derive(Template)]
    #[template(
        source = "{% if let (.., b, c) = abc  %}-{{b}}{{c}}-{% endif %}",
        ext = "txt"
    )]
    struct Tmpl {
        abc: (u32, u32, u32),
    }

    assert_eq!(Tmpl { abc: (1, 2, 3) }.to_string(), "-23-");
}

#[test]
fn c() {
    #[derive(Template)]
    #[template(source = "{% if let (.., c) = abc  %}-{{c}}-{% endif %}", ext = "txt")]
    struct Tmpl {
        abc: (u32, u32, u32),
    }

    assert_eq!(Tmpl { abc: (1, 2, 3) }.to_string(), "-3-");
}
