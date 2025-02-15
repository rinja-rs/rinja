use askama::Template;

#[test]
fn test_ref_deref() {
    #[derive(Template)]
    #[template(
        source = r#"
{%- if *title == "something" -%}
something1
{%- elif title == &"another" -%}
another2
{%- elif &*title == &&*"yep" -%}
yep3
{%- else -%}
{{title}}
{%- endif -%}
"#,
        ext = "html"
    )]
    struct RefDeref {
        title: &'static &'static str,
    }

    let x = RefDeref {
        title: &"something",
    };
    assert_eq!(x.render().unwrap(), "something1");

    let x = RefDeref { title: &"another" };
    assert_eq!(x.render().unwrap(), "another2");

    let x = RefDeref { title: &"yep" };
    assert_eq!(x.render().unwrap(), "yep3");

    let x = RefDeref { title: &"bla" };
    assert_eq!(x.render().unwrap(), "bla");
}

#[test]
fn test_ref_deref_assign() {
    #[derive(Template)]
    #[template(
        source = r#"
{%- let x = **title -%}
{%- if x == "another" -%}
another2
{%- else -%}
{{x}}
{%- endif -%}
"#,
        ext = "html"
    )]
    struct RefDerefAssignment {
        title: &'static &'static str,
    }

    let x = RefDerefAssignment { title: &"another" };
    assert_eq!(x.render().unwrap(), "another2");

    let x = RefDerefAssignment { title: &"two" };
    assert_eq!(x.render().unwrap(), "two");
}
