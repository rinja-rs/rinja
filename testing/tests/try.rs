use rinja::Template;

#[derive(Template)]
#[template(source = "{% let v = self.parse()? %}{{s}}={{v}}", ext = "txt")]
struct IntParserTemplate<'a> {
    s: &'a str,
}

impl IntParserTemplate<'_> {
    fn parse(&self) -> Result<i32, std::num::ParseIntError> {
        self.s.parse()
    }
}

#[test]
fn test_int_parser() {
    let template = IntParserTemplate { s: "💯" };
    assert!(matches!(template.render(), Err(rinja::Error::Custom(_))));
    assert_eq!(
        format!("{}", &template.render().unwrap_err()),
        "invalid digit found in string"
    );

    let template = IntParserTemplate { s: "100" };
    assert_eq!(template.render().unwrap(), "100=100");
}

#[derive(Template)]
#[template(source = "{{ value()? }}", ext = "txt")]
struct FailFmt {
    inner: Option<&'static str>,
}

impl FailFmt {
    fn value(&self) -> Result<&'static str, std::fmt::Error> {
        if let Some(inner) = self.inner {
            Ok(inner)
        } else {
            Err(std::fmt::Error)
        }
    }
}

#[test]
fn fail_fmt() {
    let template = FailFmt { inner: None };
    assert!(matches!(template.render(), Err(rinja::Error::Custom(_))));
    assert_eq!(
        format!("{}", &template.render().unwrap_err()),
        format!("{}", std::fmt::Error)
    );

    let template = FailFmt {
        inner: Some("hello world"),
    };
    assert_eq!(template.render().unwrap(), "hello world");
}

#[derive(Template)]
#[template(source = "{{ value()? }}", ext = "txt")]
struct FailStr {
    value: bool,
}

impl FailStr {
    fn value(&self) -> Result<&'static str, &'static str> {
        if !self.value {
            Err("FAIL")
        } else {
            Ok("hello world")
        }
    }
}

#[test]
fn fail_str() {
    let template = FailStr { value: false };
    assert!(matches!(template.render(), Err(rinja::Error::Custom(_))));
    assert_eq!(format!("{}", &template.render().unwrap_err()), "FAIL");

    let template = FailStr { value: true };
    assert_eq!(template.render().unwrap(), "hello world");
}
