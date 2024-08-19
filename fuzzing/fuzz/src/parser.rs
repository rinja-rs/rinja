use arbitrary::{Arbitrary, Unstructured};
use rinja_parser::{Ast, Syntax, SyntaxBuilder};

#[derive(Debug, Default)]
pub struct Scenario<'a> {
    syntax: Syntax<'a>,
    src: &'a str,
}

impl<'a> super::Scenario<'a> for Scenario<'a> {
    type RunError = rinja_parser::ParseError;

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
        Ast::from_str(src, None, syntax)?;
        Ok(())
    }
}
