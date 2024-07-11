//! Files containing tests for generated code.

use std::fmt;
use std::path::Path;

use console::style;
use similar::{Algorithm, ChangeTag, TextDiffConfig};

use crate::build_template;

// This function makes it much easier to compare expected code by adding the wrapping around
// the code we want to check.
#[track_caller]
fn compare(jinja: &str, expected: &str, fields: &[(&str, &str)], size_hint: usize) {
    let jinja = format!(
        r##"#[template(source = {jinja:?}, ext = "txt")]
struct Foo {{ {} }}"##,
        fields
            .iter()
            .map(|(name, type_)| format!("{name}: {type_}"))
            .collect::<Vec<_>>()
            .join(","),
    );
    let generated = build_template(&syn::parse_str::<syn::DeriveInput>(&jinja).unwrap())
        .unwrap()
        .parse()
        .unwrap();
    let generated: syn::File = syn::parse2(generated).unwrap();

    let size_hint = proc_macro2::Literal::usize_unsuffixed(size_hint);
    let expected: proc_macro2::TokenStream = expected.parse().unwrap();
    let expected: syn::File = syn::parse_quote! {
        impl ::rinja::Template for Foo {
            fn render_into<RinjaW>(&self, writer: &mut RinjaW) -> ::rinja::Result<()>
            where
                RinjaW: ::core::fmt::Write + ?::core::marker::Sized,
            {
                use ::rinja::filters::{AutoEscape as _, WriteWritable as _};
                use ::core::fmt::Write as _;
                #expected
                ::rinja::Result::Ok(())
            }
            const EXTENSION: ::std::option::Option<&'static ::std::primitive::str> = Some("txt");
            const SIZE_HINT: ::std::primitive::usize = #size_hint;
            const MIME_TYPE: &'static ::std::primitive::str = "text/plain; charset=utf-8";
        }
        impl ::std::fmt::Display for Foo {
            #[inline]
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                ::rinja::Template::render_into(self, f).map_err(|_| ::std::fmt::Error {})
            }
        }
    };

    let expected = prettyplease::unparse(&expected);
    let generated = prettyplease::unparse(&generated);
    if expected != generated {
        struct Diff<'a>(&'a str, &'a str);

        impl fmt::Display for Diff<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let diff = TextDiffConfig::default()
                    .algorithm(Algorithm::Patience)
                    .diff_lines(self.0, self.1);
                for change in diff.iter_all_changes() {
                    let (change, line) = match change.tag() {
                        ChangeTag::Equal => (
                            style(" ").dim().bold(),
                            style(change.to_string_lossy()).dim(),
                        ),
                        ChangeTag::Delete => (
                            style("-").red().bold(),
                            style(change.to_string_lossy()).red(),
                        ),
                        ChangeTag::Insert => (
                            style("+").green().bold(),
                            style(change.to_string_lossy()).green(),
                        ),
                    };
                    write!(f, "{change}{line}")?;
                }
                Ok(())
            }
        }

        panic!(
            "\n\
            === Expected ===\n\
            \n\
            {expected}\n\
            \n\
            === Generated ===\n\
            \n\
            {generated}\n\
            \n\
            === Diff ===\n\
            \n\
            {diff}\n\
            \n\
            === FAILURE ===",
            expected = style(&expected).red(),
            generated = style(&generated).green(),
            diff = Diff(&expected, &generated),
        );
    }
}

#[test]
fn check_if_let() {
    // In this test, we ensure that `query` never is `self.query`.
    compare(
        "{% if let Some(query) = s && !query.is_empty() %}{{query}}{% endif %}",
        r#"if let Some(query,) = &self.s && !query.is_empty() {
    match (
        &((&&::rinja::filters::AutoEscaper::new(&(query), ::rinja::filters::Text)).rinja_auto_escape()?),
    ) {
        (expr0,) => {
            (&&::rinja::filters::Writable(expr0)).rinja_write(writer)?;
        }
    }
}"#,
        &[],
        3,
    );

    // In this test, we ensure that `s` is `self.s` only in the first `if let Some(s) = self.s`
    // condition.
    compare(
        "{% if let Some(s) = s %}{{ s }}{% endif %}",
        r#"if let Some(s,) = &self.s {
    match (
        &((&&::rinja::filters::AutoEscaper::new(&(s), ::rinja::filters::Text)).rinja_auto_escape()?),
    ) {
        (expr0,) => {
            (&&::rinja::filters::Writable(expr0)).rinja_write(writer)?;
        }
    }
}"#,
        &[],
        3,
    );

    // In this test, we ensure that `s` is `self.s` only in the first `if let Some(s) = self.s`
    // condition.
    compare(
        "{% if let Some(s) = s && !s.is_empty() %}{{s}}{% endif %}",
        r#"if let Some(s,) = &self.s && !s.is_empty() {
    match (
        &((&&::rinja::filters::AutoEscaper::new(&(s), ::rinja::filters::Text)).rinja_auto_escape()?),
    ) {
        (expr0,) => {
            (&&::rinja::filters::Writable(expr0)).rinja_write(writer)?;
        }
    }
}"#,
        &[],
        3,
    );

    // In this test we make sure that every used template gets referenced exactly once.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");
    let path1 = path.join("include1.html");
    let path2 = path.join("include2.html");
    let path3 = path.join("include3.html");
    compare(
        r#"{% include "include1.html" %}"#,
        &format!(
            r#"const _: &[::core::primitive::u8] = ::core::include_bytes!({path1:#?});
            const _: &[::core::primitive::u8] = ::core::include_bytes!({path2:#?});
            const _: &[::core::primitive::u8] = ::core::include_bytes!({path3:#?});
            writer.write_str("3333")?;"#
        ),
        &[],
        4,
    );
}

#[test]
fn check_is_defined() {
    // Checks that it removes conditions if we know at compile-time that they always return false.
    //
    // We're forced to add `bla` otherwise `compare` assert fails in weird ways...
    compare(
        "{% if y is defined %}{{query}}{% endif %}bla",
        r#"writer.write_str("bla")?;"#,
        &[],
        3,
    );
    compare(
        "{% if x is not defined %}{{query}}{% endif %}bla",
        r#"writer.write_str("bla")?;"#,
        &[("x", "u32")],
        3,
    );
    compare(
        "{% if y is defined && x is not defined %}{{query}}{% endif %}bla",
        r#"writer.write_str("bla")?;"#,
        &[("x", "u32")],
        3,
    );

    // Same with declared variables.
    compare(
        "{% set y = 12 %}
         {%- if y is not defined %}{{query}}{% endif %}bla",
        r#"let y = 12;
writer.write_str("bla")?;"#,
        &[],
        3,
    );
    compare(
        "{% set y = 12 %}
         {%- if y is not defined && x is defined %}{{query}}{% endif %}bla",
        r#"let y = 12;
writer.write_str("bla")?;"#,
        &[],
        3,
    );

    // Checks that if the condition is always `true` at compile-time, then we keep the code but
    // remove the condition.
    compare(
        "{% if y is defined %}bla{% endif %}",
        r#"writer.write_str("bla")?;"#,
        &[("y", "u32")],
        3,
    );
    compare(
        "{% if x is not defined %}bla{% endif %}",
        r#"writer.write_str("bla")?;"#,
        &[],
        3,
    );
    // Same with declared variables.
    compare(
        "{% set y = 12 %}
         {%- if y is defined %}bla{% endif %}",
        r#"let y = 12;
writer.write_str("bla")?;"#,
        &[],
        3,
    );

    // If the always `true` condition is followed by more `else if`/`else`, check that they are
    // removed as well.
    compare(
        "{% if x is defined %}bli
         {%- else if x == 12 %}12{% endif %}bla",
        r#"writer.write_str("blibla")?;"#,
        &[("x", "u32")],
        6,
    );
    compare(
        "{% if x is defined %}bli
         {%- else if x == 12 %}12
         {%- else %}nope{% endif %}bla",
        r#"writer.write_str("blibla")?;"#,
        &[("x", "u32")],
        6,
    );
    // If it's not the first one.
    compare(
        "{% if x == 12 %}bli
         {%- else if x is defined %}12
         {%- else %}nope{% endif %}",
        r#"if *(&(self.x == 12) as &::core::primitive::bool) {
writer.write_str("bli")?;
} else {
writer.write_str("12")?;
}"#,
        &[("x", "u32")],
        5,
    );

    // Checking that it doesn't remove the condition if other non-"if (not) defined" checks
    // are present.
    compare(
        "{% if y is defined || x == 12 %}{{x}}{% endif %}",
        r#"if *(&(false || self.x == 12) as &::core::primitive::bool) {
    match (
        &((&&::rinja::filters::AutoEscaper::new(&(self.x), ::rinja::filters::Text)).rinja_auto_escape()?),
    ) {
        (expr0,) => {
            (&&::rinja::filters::Writable(expr0)).rinja_write(writer)?;
        }
    }
}
"#,
        &[("x", "u32")],
        3,
    );
    compare(
        "{% if y is defined || x == 12 %}{{x}}{% endif %}",
        r#"if *(&(true || self.x == 12) as &::core::primitive::bool) {
    match (
        &((&&::rinja::filters::AutoEscaper::new(&(self.x), ::rinja::filters::Text)).rinja_auto_escape()?),
    ) {
        (expr0,) => {
            (&&::rinja::filters::Writable(expr0)).rinja_write(writer)?;
        }
    }
}
"#,
        &[("y", "u32"), ("x", "u32")],
        3,
    );

    // Checking some funny cases.

    // This one is a bit useless because you can use `is not defined` but I suppose it's possible
    // to encounter cases like that in the wild so better have a check.
    compare(
        "{% if !(y is defined) %}bla{% endif %}",
        r#"writer.write_str("bla")?;"#,
        &[],
        3,
    );
    compare(
        "{% if !(y is not defined) %}bli{% endif %}bla",
        r#"writer.write_str("bla")?;"#,
        &[],
        3,
    );
    compare(
        "{% if !(y is defined) %}bli{% endif %}bla",
        r#"writer.write_str("bla")?;"#,
        &[("y", "u32")],
        3,
    );
    compare(
        "{% if !(y is not defined) %}bla{% endif %}",
        r#"writer.write_str("bla")?;"#,
        &[("y", "u32")],
        3,
    );
}

#[test]
fn check_bool_conditions() {
    // Checks that it removes conditions if we know at compile-time that they always return false.
    //
    // We're forced to add `bla` otherwise `compare` assert fails in weird ways...
    compare(
        "{% if false %}{{query}}{% endif %}bla",
        r#"writer.write_str("bla")?;"#,
        &[],
        3,
    );
    compare(
        "{% if false && false %}{{query}}{% endif %}bla",
        r#"writer.write_str("bla")?;"#,
        &[],
        3,
    );
    compare(
        "{% if false && true %}{{query}}{% endif %}bla",
        r#"writer.write_str("bla")?;"#,
        &[],
        3,
    );
    compare(
        "{% if true && false %}{{query}}{% endif %}bla",
        r#"writer.write_str("bla")?;"#,
        &[],
        3,
    );
    compare(
        "{% if false || true %}bli{% endif %}bla",
        r#"writer.write_str("blibla")?;"#,
        &[],
        6,
    );
    compare(
        "{% if true || false %}bli{% endif %}bla",
        r#"writer.write_str("blibla")?;"#,
        &[],
        6,
    );

    compare(
        "{% if true || x == 12 %}{{x}}{% endif %}",
        r#"if *(&(true || self.x == 12) as &::core::primitive::bool) {
    match (
        &((&&::rinja::filters::AutoEscaper::new(&(self.x), ::rinja::filters::Text)).rinja_auto_escape()?),
    ) {
        (expr0,) => {
            (&&::rinja::filters::Writable(expr0)).rinja_write(writer)?;
        }
    }
}
"#,
        &[("x", "u32")],
        3,
    );
    compare(
        "{% if false || x == 12 %}{{x}}{% endif %}",
        r#"if *(&(false || self.x == 12) as &::core::primitive::bool) {
    match (
        &((&&::rinja::filters::AutoEscaper::new(
            &(self.x),
            ::rinja::filters::Text,
        ))
            .rinja_auto_escape()?),
    ) {
        (expr0,) => {
            (&&::rinja::filters::Writable(expr0)).rinja_write(writer)?;
        }
    }
}
"#,
        &[("x", "u32")],
        3,
    );

    // Some funny cases.
    compare(
        "{% if !(false) %}bla{% endif %}",
        r#"writer.write_str("bla")?;"#,
        &[],
        3,
    );
    compare(
        "{% if !(true) %}{{query}}{% endif %}bla",
        r#"writer.write_str("bla")?;"#,
        &[],
        3,
    );
}

#[test]
fn check_escaping_at_compile_time() {
    compare(
        r#"The card is
        {%- match suit %}
            {%- when Suit::Clubs or Suit::Spades -%}
                {{ " black" }}
            {%- when Suit::Diamonds or Suit::Hearts -%}
                {{ " red" }}
        {%- endmatch %}"#,
        r#"writer.write_str("The card is")?;
        match &self.suit {
            Suit::Clubs | Suit::Spades => {
                writer.write_str(" black")?;
            }
            Suit::Diamonds | Suit::Hearts => {
                writer.write_str(" red")?;
            }
        }"#,
        &[("suit", "Suit")],
        16,
    );

    compare(
        r#"{{ '\x41' }}{{ '\n' }}{{ '\r' }}{{ '\t' }}{{ '\\' }}{{ '\u{2665}' }}{{ '\'' }}{{ '\"' }}{{ '"' }}
{{ "\x41\n\r\t\\\u{2665}\'\"'" }}"#,
        r#"writer.write_str("A
\r	\\♥'\"\"
A
\r	\\♥'\"'")?;"#,
        &[],
        23,
    );
}
