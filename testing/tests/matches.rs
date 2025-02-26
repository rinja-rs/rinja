use askama::Template;

#[test]
fn test_match_option() {
    #[derive(Template)]
    #[template(path = "match-opt.html")]
    struct MatchOptTemplate<'a> {
        item: Option<&'a str>,
    }

    let s = MatchOptTemplate { item: Some("foo") };
    assert_eq!(s.render().unwrap(), "\nFound literal foo\n");

    let s = MatchOptTemplate { item: Some("bar") };
    assert_eq!(s.render().unwrap(), "\nFound bar\n");

    let s = MatchOptTemplate { item: None };
    assert_eq!(s.render().unwrap(), "\nNot Found\n");
}

#[test]
fn test_match_option_bool() {
    #[derive(Template)]
    #[template(path = "match-opt-bool.html")]
    struct MatchOptBoolTemplate {
        item: Option<bool>,
    }

    let s = MatchOptBoolTemplate { item: Some(true) };
    assert_eq!(s.render().unwrap(), "\nFound Some(true)\n");

    let s = MatchOptBoolTemplate { item: Some(false) };
    assert_eq!(s.render().unwrap(), "\nFound Some(false)\n");

    let s = MatchOptBoolTemplate { item: None };
    assert_eq!(s.render().unwrap(), "\nNot Found\n");
}

#[test]
fn test_match_ref_deref() {
    #[derive(Template)]
    #[template(path = "match-opt.html")]
    struct MatchOptRefTemplate<'a> {
        item: &'a Option<&'a str>,
    }

    let s = MatchOptRefTemplate { item: &Some("foo") };
    assert_eq!(s.render().unwrap(), "\nFound literal foo\n");
}

#[test]
fn test_match_literal() {
    #[derive(Template)]
    #[template(path = "match-literal.html")]
    struct MatchLitTemplate<'a> {
        item: &'a str,
    }

    let s = MatchLitTemplate { item: "bar" };
    assert_eq!(s.render().unwrap(), "\nFound literal bar\n");

    let s = MatchLitTemplate { item: "qux" };
    assert_eq!(s.render().unwrap(), "\nElse found qux\n");
}

#[test]
fn test_match_literal_char() {
    #[derive(Template)]
    #[template(path = "match-literal-char.html")]
    struct MatchLitCharTemplate {
        item: char,
    }

    let s = MatchLitCharTemplate { item: 'b' };
    assert_eq!(s.render().unwrap(), "\nFound literal b\n");

    let s = MatchLitCharTemplate { item: 'c' };
    assert_eq!(s.render().unwrap(), "\nElse found c\n");
}

#[test]
fn test_match_literal_num() {
    #[derive(Template)]
    #[template(path = "match-literal-num.html")]
    struct MatchLitNumTemplate {
        item: u32,
    }

    let s = MatchLitNumTemplate { item: 42 };
    assert_eq!(s.render().unwrap(), "\nFound answer to everything\n");

    let s = MatchLitNumTemplate { item: 23 };
    assert_eq!(s.render().unwrap(), "\nElse found 23\n");
}

#[test]
fn test_match_custom_enum() {
    #[allow(dead_code)]
    enum Color {
        Rgb { r: u32, g: u32, b: u32 },
        GrayScale(u32),
        Cmyk(u32, u32, u32, u32),
    }

    #[derive(Template)]
    #[template(path = "match-custom-enum.html")]
    struct MatchCustomEnumTemplate {
        color: Color,
    }

    let s = MatchCustomEnumTemplate {
        color: Color::Rgb {
            r: 160,
            g: 0,
            b: 255,
        },
    };
    assert_eq!(s.render().unwrap(), "\nColorful: #A000FF\n");
}

#[test]
fn test_match_no_whitespace() {
    #[derive(Template)]
    #[template(path = "match-no-ws.html")]
    struct MatchNoWhitespace {
        foo: Option<usize>,
    }

    let s = MatchNoWhitespace { foo: Some(1) };
    assert_eq!(s.render().unwrap(), "1");
}

#[test]
fn test_match_without_with_keyword() {
    #[derive(Template)]
    #[template(
        source = "{% match foo %}{% when Some(bar) %}{{ bar }}{% when None %}{% endmatch %}",
        ext = "txt"
    )]
    struct MatchWithoutWithKeyword {
        foo: Option<usize>,
    }

    let s = MatchWithoutWithKeyword { foo: Some(1) };
    assert_eq!(s.render().unwrap(), "1");
    let s = MatchWithoutWithKeyword { foo: None };
    assert_eq!(s.render().unwrap(), "");
}

#[test]
fn test_match_option_result_option() {
    #[derive(Template)]
    #[template(path = "match-option-result-option.html")]
    struct MatchOptionResultOption {
        foo: Option<Result<Option<usize>, &'static str>>,
    }

    let s = MatchOptionResultOption { foo: None };
    assert_eq!(s.render().unwrap(), "nothing");
    let s = MatchOptionResultOption {
        foo: Some(Err("fail")),
    };
    assert_eq!(s.render().unwrap(), "err=fail");
    let s = MatchOptionResultOption {
        foo: Some(Ok(None)),
    };
    assert_eq!(s.render().unwrap(), "num=absent");
    let s = MatchOptionResultOption {
        foo: Some(Ok(Some(4711))),
    };
    assert_eq!(s.render().unwrap(), "num=4711");
}

#[test]
fn test_match_with_comment() {
    #[derive(Template)]
    #[template(
        ext = "txt",
        source = r#"
{%- match good -%}
    {#- when good, then good -#}
    {%- when true -%}
        good
    {%- when _ -%}
        bad
{%- endmatch -%}"#
    )]
    struct MatchWithComment {
        good: bool,
    }

    let s = MatchWithComment { good: true };
    assert_eq!(s.render().unwrap(), "good");

    let s = MatchWithComment { good: false };
    assert_eq!(s.render().unwrap(), "bad");
}

#[test]
fn test_match_enum_or() {
    enum Suit {
        Clubs,
        Diamonds,
        Hearts,
        Spades,
    }

    #[derive(Template)]
    #[template(path = "match-enum-or.html")]
    struct MatchEnumOrTemplate {
        suit: Suit,
    }

    let template = MatchEnumOrTemplate { suit: Suit::Clubs };
    assert_eq!(template.render().unwrap(), "The card is black\n");
    let template = MatchEnumOrTemplate { suit: Suit::Spades };
    assert_eq!(template.render().unwrap(), "The card is black\n");

    let template = MatchEnumOrTemplate { suit: Suit::Hearts };
    assert_eq!(template.render().unwrap(), "The card is red\n");

    let template = MatchEnumOrTemplate {
        suit: Suit::Diamonds,
    };
    assert_eq!(template.render().unwrap(), "The card is red\n");
}

#[test]
fn test_empty_match() {
    #[derive(Template)]
    #[template(
        source = "{% match true %}{% else %}otherwise{% endmatch %}",
        ext = "html"
    )]
    struct EmptyMatch;

    assert_eq!(EmptyMatch.to_string(), "otherwise");
}

#[test]
fn test_match_with_patterns() {
    #[derive(Template)]
    #[template(
        ext = "txt",
        source = r#"
{%- match n -%}
    {%- when 1 | 2 | 3 | 4 -%}
        a listed one!
    {%- when 6 | 7 -%}
        another listed one!
    {%- when n -%}
        {{ n }}
{%- endmatch -%}"#
    )]
    struct MatchPatterns {
        n: u8,
    }

    let s = MatchPatterns { n: 1 };
    assert_eq!(s.render().unwrap(), "a listed one!");

    let s = MatchPatterns { n: 6 };
    assert_eq!(s.render().unwrap(), "another listed one!");

    let s = MatchPatterns { n: 12 };
    assert_eq!(s.render().unwrap(), "12");
}

#[test]
fn test_end_when() {
    #[derive(Template)]
    #[template(in_doc = true, ext = "html")]
    /// ```askama
    /// {% match result %}
    ///     {% when Some(Ok(s)) -%}
    ///         good: {{s}}
    ///     {%- endwhen +%}
    ///     {# This is not good: #}
    ///     {%+ when Some(Err(s)) -%}
    ///         bad: {{s}}
    ///     {%- endwhen +%}
    ///     {%+ else -%}
    ///         unprocessed
    /// {% endmatch %}
    /// ```
    struct EndWhen<'a> {
        result: Option<Result<&'a str, &'a str>>,
    }

    let tmpl = EndWhen {
        result: Some(Ok("msg")),
    };
    assert_eq!(tmpl.to_string(), "good: msg");

    let tmpl = EndWhen {
        result: Some(Err("msg")),
    };
    assert_eq!(tmpl.to_string(), "bad: msg");

    let tmpl = EndWhen { result: None };
    assert_eq!(tmpl.to_string(), "unprocessed\n");
}
