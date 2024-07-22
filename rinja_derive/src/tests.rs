//! Files containing tests for generated code.

use std::fmt;
use std::path::Path;

use console::style;
use similar::{Algorithm, ChangeTag, TextDiffConfig};

use crate::build_template;

#[test]
fn check_if_let() {
    // This function makes it much easier to compare expected code by adding the wrapping around
    // the code we want to check.
    #[track_caller]
    fn compare(jinja: &str, expected: &str, size_hint: usize) {
        let jinja = format!(r#"#[template(source = {jinja:?}, ext = "txt")] struct Foo;"#);
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
                    use ::rinja::filters::AutoEscape as _;
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

        if expected != generated {
            let expected = prettyplease::unparse(&expected);
            let generated = prettyplease::unparse(&generated);

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

    // In this test, we ensure that `query` never is `self.query`.
    compare(
        "{% if let Some(query) = s && !query.is_empty() %}{{query}}{% endif %}",
        r#"if let Some(query,) = &self.s && !query.is_empty() {
    ::std::write!(
        writer,
        "{expr0}",
        expr0 = &(&&::rinja::filters::AutoEscaper::new(&(query), ::rinja::filters::Text)).rinja_auto_escape()?,
    )?;
}"#,
        3,
    );

    // In this test, we ensure that `s` is `self.s` only in the first `if let Some(s) = self.s`
    // condition.
    compare(
        "{% if let Some(s) = s %}{{ s }}{% endif %}",
        r#"if let Some(s,) = &self.s {
    ::std::write!(
        writer,
        "{expr0}",
        expr0 = &(&&::rinja::filters::AutoEscaper::new(&(s), ::rinja::filters::Text)).rinja_auto_escape()?,
    )?;
}"#,
        3,
    );

    // In this test, we ensure that `s` is `self.s` only in the first `if let Some(s) = self.s`
    // condition.
    compare(
        "{% if let Some(s) = s && !s.is_empty() %}{{s}}{% endif %}",
        r#"if let Some(s,) = &self.s && !s.is_empty() {
    ::std::write!(
        writer,
        "{expr0}",
        expr0 = &(&&::rinja::filters::AutoEscaper::new(&(s), ::rinja::filters::Text)).rinja_auto_escape()?,
    )?;
}"#,
        3,
    );

    // In this test we make sure that every used template gets referenced exactly once.
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("templates");
    let path1 = path.join("include1.html");
    let path2 = path.join("include2.html");
    let path3 = path.join("include3.html");
    compare(
        r#"{% include "include1.html" %}"#,
        // The order of the first 3 lines is non-deterministic!
        &format!(
            r#"const _: &[::core::primitive::u8] = ::core::include_bytes!({path3:#?});
            const _: &[::core::primitive::u8] = ::core::include_bytes!({path1:#?});
            const _: &[::core::primitive::u8] = ::core::include_bytes!({path2:#?});
            const _: &[::core::primitive::u8] = ::core::include_bytes!({path1:#?});
            const _: &[::core::primitive::u8] = ::core::include_bytes!({path2:#?});
            const _: &[::core::primitive::u8] = ::core::include_bytes!({path3:#?});
            writer.write_str("3")?;
            const _: &[::core::primitive::u8] = ::core::include_bytes!({path3:#?});
            writer.write_str("3")?;
            const _: &[::core::primitive::u8] = ::core::include_bytes!({path2:#?});
            const _: &[::core::primitive::u8] = ::core::include_bytes!({path3:#?});
            writer.write_str("3")?;
            const _: &[::core::primitive::u8] = ::core::include_bytes!({path3:#?});
            writer.write_str("3")?;"#
        ),
        4,
    );
}
