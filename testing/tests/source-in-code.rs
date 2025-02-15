#![cfg(feature = "code-in-doc")]

// Regarding `#[rustfmt::skip]`: `cargo fmt` strips extraneous newlines in code block,
// but we want to test if `askama_derive` works with extraneous newlines.

use askama::Template;

#[rustfmt::skip]
#[test]
fn test_code_in_comment_only() {
    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /// ```askama
    /// Hello world!
    /// ```
    struct Tmpl;

    assert_eq!(Tmpl.to_string(), "Hello world!");
}

#[rustfmt::skip]
#[test]
fn test_code_in_comment_with_line_break() {
    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /// ```askama
    /// Hello
    /// world!
    /// ```
    struct Tmpl1;
    assert_eq!(Tmpl1.to_string(), "Hello\nworld!");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /// ```askama
    /// Hello
    ///
    /// world!
    /// ```
    struct Tmpl2;
    assert_eq!(Tmpl2.to_string(), "Hello\n\nworld!");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /// ```askama
    /// 
    /// Hello
    ///
    /// world!
    /// ```
    struct Tmpl3;
    assert_eq!(Tmpl3.to_string(), "\nHello\n\nworld!");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /// ```askama
    /// Hello
    ///
    /// world!
    ///
    /// ```
    struct Tmpl4;
    assert_eq!(Tmpl4.to_string(), "Hello\n\nworld!\n");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /// ```askama
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
    /// ```askama
    /// Hello
    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /// world!
    /// ```
    struct Tmpl1;
    assert_eq!(Tmpl1.to_string(), "Hello\nworld!");

    /// This template greets the whole world
    ///
    /// ```askama
    /// Hello
    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
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
    #[template(ext = "txt", in_doc = true)]
    /// Text1
    /// ```askama
    /// Hello
    /// ```
    /// Text2
    /// ```askama
    /// world!
    /// ```
    /// Text3
    struct Tmpl1;
    assert_eq!(Tmpl1.to_string(), "Hello\nworld!");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /// Text1
    ///
    /// ```askama
    /// Hello
    /// ```
    ///
    /// Text2
    ///
    /// ```askama
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
    #[template(ext = "txt", in_doc = true)]
    #[doc = "```askama\nHello world!\n```"]
    struct Tmpl1;
    assert_eq!(Tmpl1.to_string(), "Hello world!");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    #[doc = "```askama"]
    #[doc = "Hello world!"]
    #[doc = "```"]
    struct Tmpl2;
    assert_eq!(Tmpl2.to_string(), "Hello world!");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    #[doc = "```askama\nHello world!"]
    #[doc = "```"]
    struct Tmpl3;
    assert_eq!(Tmpl3.to_string(), "Hello world!");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    #[doc = "```askama"]
    #[doc = "Hello world!\n```"]
    struct Tmpl4;
    assert_eq!(Tmpl4.to_string(), "Hello world!");
}

#[rustfmt::skip]
#[test]
fn test_code_in_comment_multiline() {
    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /**
    ```askama
    Hello world!
    ```
    */
    struct Tmpl1;
    assert_eq!(Tmpl1.to_string(), "Hello world!");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /**
    ```askama
    Hello
    world!
    ```
    */
    struct Tmpl2;
    assert_eq!(Tmpl2.to_string(), "Hello\nworld!");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /**
    ```askama

    Hello
    world!
    ```
    */
    struct Tmpl3;
    assert_eq!(Tmpl3.to_string(), "\nHello\nworld!");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /**
    ```askama
    Hello
    world!

    ```
    */
    struct Tmpl4;
    assert_eq!(Tmpl4.to_string(), "Hello\nworld!\n");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /**
    ```askama

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
    #[template(ext = "txt", in_doc = true)]
    /// ````askama
    /// Hello
    /// ````
    struct Tmpl1;
    assert_eq!(Tmpl1.to_string(), "Hello");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /// `````askama
    /// Hello
    /// `````
    struct Tmpl2;
    assert_eq!(Tmpl2.to_string(), "Hello");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /// ``````````````````````````````````````````````````````````````````````````````````````askama
    /// Hello
    /// ``````````````````````````````````````````````````````````````````````````````````````
    struct Tmpl3;
    assert_eq!(Tmpl3.to_string(), "Hello");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /// ```askama
    /// `````
    /// Hello
    /// `````
    /// ```
    struct Tmpl4;
    assert_eq!(Tmpl4.to_string(), "");

    #[derive(Template)]
    #[template(ext = "txt", in_doc = true)]
    /// `````askama
    /// ```
    /// Hello
    /// ```
    /// `````
    struct Tmpl5;
    assert_eq!(Tmpl5.to_string(), "```\nHello\n```");
}
