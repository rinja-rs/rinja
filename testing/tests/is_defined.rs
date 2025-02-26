use askama::Template;

// This test ensures that `include` are correctly working inside filter blocks and that external
// variables are used correctly.
#[test]
fn is_defined_in_expr() {
    #[derive(Template)]
    #[template(
        source = r#"<script>
const x = {{ x is defined }};
const y = {{ y is not defined }};
const z = {{ y is defined }};
const w = {{ x is not defined }};
const v = {{ y }};
</script>"#,
        ext = "html"
    )]
    struct IsDefined {
        y: u32,
    }

    let s = IsDefined { y: 0 };
    assert_eq!(
        s.render().unwrap(),
        r"<script>
const x = false;
const y = false;
const z = true;
const w = true;
const v = 0;
</script>"
    );
}

// This test ensures that if the variable is not defined, it will not generate following code.
#[test]
fn is_defined_chaining() {
    #[derive(Template)]
    #[template(
        source = r#"{% if x is defined && x == 12 %}bli{% else %}bla{% endif %}"#,
        ext = "html"
    )]
    struct IsDefinedChaining;

    assert_eq!(IsDefinedChaining.render().unwrap(), r"bla");
}
