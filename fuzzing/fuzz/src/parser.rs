use std::borrow::Cow;
use std::fmt;

use arbitrary::{Arbitrary, Unstructured};
use askama_parser::{Ast, InnerSyntax, Syntax, SyntaxBuilder};

#[derive(Debug, Default)]
pub struct Scenario<'a> {
    syntax: Syntax<'a>,
    src: &'a str,
}

impl<'a> super::Scenario<'a> for Scenario<'a> {
    type RunError = askama_parser::ParseError;

    fn new(data: &'a [u8]) -> Result<Self, arbitrary::Error> {
        let mut data = Unstructured::new(data);

        let syntax = if let Some(syntax) = <Option<[Option<&'a str>; 6]>>::arbitrary(&mut data)? {
            SyntaxBuilder {
                name: "test",
                block_start: syntax[0],
                block_end: syntax[1],
                expr_start: syntax[2],
                expr_end: syntax[3],
                comment_start: syntax[4],
                comment_end: syntax[5],
            }
            .to_syntax()
            .map_err(|_| arbitrary::Error::IncorrectFormat)?
        } else {
            Syntax::default()
        };

        let src = <&str>::arbitrary_take_rest(data)?;

        Ok(Self { syntax, src })
    }

    fn run(&self) -> Result<(), Self::RunError> {
        let Scenario { syntax, src } = self;
        let _: Ast<'_> = Ast::from_str(src, None, syntax)?;
        Ok(())
    }
}

impl fmt::Display for Scenario<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Scenario { syntax, src } = self;
        let syntax = if *syntax == Syntax::default() {
            Cow::Borrowed("Syntax::default()")
        } else {
            let InnerSyntax {
                block_start,
                block_end,
                expr_start,
                expr_end,
                comment_start,
                comment_end,
            } = **syntax;
            Cow::Owned(format!(
                "\
SyntaxBuilder {{
        name: \"test\",
        block_start: {block_start:?},
        block_end: {block_end:?},
        expr_start: {expr_start:?},
        expr_end: {expr_end:?},
        comment_start: {comment_start:?},
        comment_end: {comment_end:?},
    }}.to_syntax().unwrap()",
            ))
        };
        write!(
            f,
            "\
use askama_parser::{{Ast, ParseError}};

#[test]
fn test() -> Result<(), ParseError> {{
    let src = {src:?};
    let syntax = {syntax};
    Ast::from_str(src, None, &syntax).map(|_| ())
}}\
            ",
        )
    }
}
