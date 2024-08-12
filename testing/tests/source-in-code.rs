// Regarding `#[rustfmt::skip]`: `cargo fmt` strips extraneous newlines in code block,
// but we want to test if `rinja_derive` works with extraneous newlines.

use rinja::Template;

#[rustfmt::skip]
#[test]
fn test_code_in_comment_only() {
    #[derive(Template)]
    #[template(ext = "txt")]
    /// ```rinja
    /// Hello world!
    /// ```
    struct Tmpl;

    assert_eq!(Tmpl.to_string(), "Hello world!");
}

#[rustfmt::skip]
#[test]
fn test_code_in_comment_with_line_break() {
    #[derive(Template)]
    #[template(ext = "txt")]
    /// ```rinja
    /// Hello
    /// world!
    /// ```
    struct Tmpl1;
    assert_eq!(Tmpl1.to_string(), "Hello\nworld!");

    #[derive(Template)]
    #[template(ext = "txt")]
    /// ```rinja
    /// Hello
    ///
    /// world!
    /// ```
    struct Tmpl2;
    assert_eq!(Tmpl2.to_string(), "Hello\n\nworld!");

    #[derive(Template)]
    #[template(ext = "txt")]
    /// ```rinja
    /// 
    /// Hello
    ///
    /// world!
    /// ```
    struct Tmpl3;
    assert_eq!(Tmpl3.to_string(), "\nHello\n\nworld!");

    #[derive(Template)]
    #[template(ext = "txt")]
    /// ```rinja
    /// Hello
    ///
    /// world!
    ///
    /// ```
    struct Tmpl4;
    assert_eq!(Tmpl4.to_string(), "Hello\n\nworld!\n");

    #[derive(Template)]
    #[template(ext = "txt")]
    /// ```rinja
    /// 
    ///
    /// Hello
    ///
    ///
    /// world!
    ///
    ///
    /// ```
    struct Tmpl5;
    assert_eq!(Tmpl5.to_string(), "\n\nHello\n\n\nworld!\n\n");
}

#[rustfmt::skip]
#[test]
fn test_code_in_comment_with_derive_in_between() {
    /// ```rinja
    /// Hello
    #[derive(Template)]
    #[template(ext = "txt")]
    /// world!
    /// ```
    struct Tmpl1;
    assert_eq!(Tmpl1.to_string(), "Hello\nworld!");

    /// This template greets the whole world
    ///
    /// ```rinja
    /// Hello
    #[derive(Template)]
    #[template(ext = "txt")]
    /// world!
    /// ```
    /// 
    /// Some more text.
    struct Tmpl2;
    assert_eq!(Tmpl2.to_string(), "Hello\nworld!");
}

#[rustfmt::skip]
#[test]
fn test_code_in_comment_split_up() {
    #[derive(Template)]
    #[template(ext = "txt")]
    /// Text1
    /// ```rinja
    /// Hello
    /// ```
    /// Text2
    /// ```rinja
    /// world!
    /// ```
    /// Text3
    struct Tmpl1;
    assert_eq!(Tmpl1.to_string(), "Hello\nworld!");

    #[derive(Template)]
    #[template(ext = "txt")]
    /// Text1
    ///
    /// ```rinja
    /// Hello
    /// ```
    ///
    /// Text2
    ///
    /// ```rinja
    /// world!
    /// ```
    ///
    /// Text3
    struct Tmpl2;
    assert_eq!(Tmpl2.to_string(), "Hello\nworld!");
}

#[rustfmt::skip]
#[test]
fn test_code_in_comment_doc() {
    #[derive(Template)]
    #[template(ext = "txt")]
    #[doc = "```rinja\nHello world!\n```"]
    struct Tmpl1;
    assert_eq!(Tmpl1.to_string(), "Hello world!");

    #[derive(Template)]
    #[template(ext = "txt")]
    #[doc = "```rinja"]
    #[doc = "Hello world!"]
    #[doc = "```"]
    struct Tmpl2;
    assert_eq!(Tmpl2.to_string(), "Hello world!");

    #[derive(Template)]
    #[template(ext = "txt")]
    #[doc = "```rinja\nHello world!"]
    #[doc = "```"]
    struct Tmpl3;
    assert_eq!(Tmpl3.to_string(), "Hello world!");

    #[derive(Template)]
    #[template(ext = "txt")]
    #[doc = "```rinja"]
    #[doc = "Hello world!\n```"]
    struct Tmpl4;
    assert_eq!(Tmpl4.to_string(), "Hello world!");
}

#[rustfmt::skip]
#[test]
fn test_code_in_comment_multiline() {
    #[derive(Template)]
    #[template(ext = "txt")]
    /**
    ```rinja
    Hello world!
    ```
    */
    struct Tmpl1;
    assert_eq!(Tmpl1.to_string(), "Hello world!");

    #[derive(Template)]
    #[template(ext = "txt")]
    /**
    ```rinja
    Hello
    world!
    ```
    */
    struct Tmpl2;
    assert_eq!(Tmpl2.to_string(), "Hello\nworld!");

    #[derive(Template)]
    #[template(ext = "txt")]
    /**
    ```rinja

    Hello
    world!
    ```
    */
    struct Tmpl3;
    assert_eq!(Tmpl3.to_string(), "\nHello\nworld!");

    #[derive(Template)]
    #[template(ext = "txt")]
    /**
    ```rinja
    Hello
    world!

    ```
    */
    struct Tmpl4;
    assert_eq!(Tmpl4.to_string(), "Hello\nworld!\n");

    #[derive(Template)]
    #[template(ext = "txt")]
    /**
    ```rinja

    Hello

    world!

    ```
    */
    struct Tmpl5;
    assert_eq!(Tmpl5.to_string(), "\nHello\n\nworld!\n");
}

#[rustfmt::skip]
#[test]
fn test_code_in_comment_backticks() {
    #[derive(Template)]
    #[template(ext = "txt")]
    /// ````rinja
    /// Hello
    /// ````
    struct Tmpl1;
    assert_eq!(Tmpl1.to_string(), "Hello");

    #[derive(Template)]
    #[template(ext = "txt")]
    /// `````rinja
    /// Hello
    /// `````
    struct Tmpl2;
    assert_eq!(Tmpl2.to_string(), "Hello");

    #[derive(Template)]
    #[template(ext = "txt")]
    /// ``````````````````````````````````````````````````````````````````````````````````````rinja
    /// Hello
    /// ``````````````````````````````````````````````````````````````````````````````````````
    struct Tmpl3;
    assert_eq!(Tmpl3.to_string(), "Hello");

    #[derive(Template)]
    #[template(ext = "txt")]
    /// ```rinja
    /// `````
    /// Hello
    /// `````
    /// ```
    struct Tmpl4;
    assert_eq!(Tmpl4.to_string(), "`````\nHello\n`````");

    #[derive(Template)]
    #[template(ext = "txt")]
    /// `````rinja
    /// ```
    /// Hello
    /// ```
    /// `````
    struct Tmpl5;
    assert_eq!(Tmpl5.to_string(), "```\nHello\n```");
}
