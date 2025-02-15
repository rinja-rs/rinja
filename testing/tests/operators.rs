use askama::Template;

#[test]
fn test_compare() {
    #[derive(Template)]
    #[template(path = "compare.html")]
    struct CompareTemplate {
        a: usize,
        b: usize,
        c: usize,
    }

    let t = CompareTemplate { a: 1, b: 1, c: 2 };
    assert_eq!(t.render().unwrap(), "tf\ntf\ntf\ntf\ntf\ntf");
}

#[test]
fn test_operators() {
    #[derive(Template)]
    #[template(path = "operators.html")]
    struct OperatorsTemplate {
        a: usize,
        b: usize,
        c: usize,
    }

    let t = OperatorsTemplate { a: 1, b: 1, c: 2 };
    assert_eq!(t.render().unwrap(), "muldivmodaddrshlshbandbxorborandor");
}

#[test]
fn test_precedence() {
    #[derive(Template)]
    #[template(path = "precedence.html")]
    struct PrecedenceTemplate {}

    let t = PrecedenceTemplate {};
    assert_eq!(t.render().unwrap(), "6".repeat(7));
}

#[test]
fn test_ranges() {
    #[derive(Template)]
    #[template(path = "ranges.txt")]
    struct RangesTemplate<'a> {
        foo: Vec<&'a str>,
    }

    let t = RangesTemplate {
        foo: vec!["a", "b", "c", "d"],
    };
    assert_eq!(t.render().unwrap(), "abcd\nbcd\n\na\nab");
}

#[test]
fn test_short_circuit() {
    #[derive(Template)]
    #[template(source = "{{ true && true }}{{ false || true }}", ext = "txt")]
    struct ShortCircuitTemplate {}

    let t = ShortCircuitTemplate {};
    assert_eq!(t.render().unwrap(), "truetrue");
}
