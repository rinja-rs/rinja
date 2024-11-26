use rinja::Template;

#[test]
fn test_include() {
    #[derive(Template)]
    #[template(path = "include.html")]
    struct IncludeTemplate<'a> {
        strs: &'a [&'a str],
    }

    let strs = vec!["foo", "bar"];
    let s = IncludeTemplate { strs: &strs };
    assert_eq!(s.render().unwrap(), "\n  INCLUDED: foo\n  INCLUDED: bar");
}

#[test]
fn test_include_extends() {
    #[derive(Template)]
    #[template(path = "include-extends.html")]
    struct IncludeExtendsTemplate<'a> {
        name: &'a str,
    }

    let template = IncludeExtendsTemplate { name: "Alice" };

    assert_eq!(
        template.render().unwrap(),
        "<div>\n    \
         <h1>Welcome</h1>\n    \
         <div>\n    \
         <p>Below me is the header</p>\n    \
         foo\n    \
         <p>Above me is the header</p>\n\
         </div>\n\
         Hello, Alice!\n\
         </div>"
    );
}

#[test]
fn test_include_macro() {
    #[derive(Template)]
    #[template(path = "include-macro.html")]
    struct IncludeMacroTemplate<'a> {
        name: &'a str,
        name2: &'a str,
    }

    let template = IncludeMacroTemplate {
        name: "Alice",
        name2: "Bob",
    };

    assert_eq!(template.render().unwrap(), "Hello, Alice!\nHowdy, Bob!");
}

// FIXME: <https://github.com/rinja-rs/rinja/issues/272>
#[test]
#[should_panic(expected = r#"assertion `left == right` failed
  left: "block_in_base: from extended!\nblock_in_partial: from partial!\n"
 right: "block_in_base: from extended!\nblock_in_partial: from extended!\n"#)]
fn block_in_include() {
    #[derive(Template)]
    #[template(path = "block_in_include_extended.html")]
    struct TmplExtended;

    #[derive(Template)]
    #[template(path = "block_in_include_base.html", block = "block_in_base")]
    struct TmplBlockInBase;

    #[derive(Template)]
    #[template(path = "block_in_include_partial.html", block = "block_in_partial")]
    struct TmplBlockInPartial;

    assert_eq!(
        TmplExtended.render().unwrap(),
        "block_in_base: from extended!\nblock_in_partial: from extended!\n"
    );
    assert_eq!(
        TmplBlockInBase.render().unwrap(),
        "block_in_base: from extended!\n"
    );
    assert_eq!(
        TmplBlockInPartial.render().unwrap(),
        "block_in_partial: from extended!\n"
    );
}
