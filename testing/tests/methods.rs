use askama::Template;

#[derive(Template)]
#[template(source = "{{ self.get_s() }}", ext = "txt")]
struct SelfMethodTemplate<'a> {
    s: &'a str,
}

impl<'a> SelfMethodTemplate<'a> {
    fn get_s(&self) -> &'a str {
        self.s
    }
}

#[test]
fn test_self_method() {
    let t = SelfMethodTemplate { s: "foo" };
    assert_eq!(t.render().unwrap(), "foo");
}

#[test]
fn test_self_raw_identifier_method() {
    #[derive(Template)]
    #[template(source = "{{ self.type() }}", ext = "txt")]
    struct SelfRawIdentifierMethodTemplate {}

    impl SelfRawIdentifierMethodTemplate {
        fn r#type(&self) -> &str {
            "foo"
        }
    }

    let t = SelfRawIdentifierMethodTemplate {};
    assert_eq!(t.render().unwrap(), "foo");
}

#[test]
fn test_nested() {
    #[derive(Template)]
    #[template(source = "{{ self.get_s() }} {{ t.get_s() }}", ext = "txt")]
    struct NestedSelfMethodTemplate<'a> {
        t: SelfMethodTemplate<'a>,
    }

    impl<'a> NestedSelfMethodTemplate<'a> {
        fn get_s(&self) -> &'a str {
            "bar"
        }
    }

    let t = NestedSelfMethodTemplate {
        t: SelfMethodTemplate { s: "foo" },
    };
    assert_eq!(t.render().unwrap(), "bar foo");
}
