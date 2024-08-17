use arbitrary::{Arbitrary, Unstructured};
use rinja_parser::{Ast, Syntax};

#[derive(Debug, Default)]
pub struct Scenario<'a> {
    syntax: Syntax<'a>,
    src: &'a str,
}

impl<'a> super::Scenario<'a> for Scenario<'a> {
    type NewError = arbitrary::Error;

    type RunError = rinja_parser::ParseError;

    fn new(data: &'a [u8]) -> Result<Self, Self::NewError> {
        let mut data = Unstructured::new(data);

        let syntax = ArbitrarySyntax::arbitrary(&mut data)?;
        let _syntax = syntax.as_syntax();
        // FIXME: related issue: <https://github.com/rinja-rs/rinja/issues/138>
        let syntax = Syntax::default();

        let src = <&str>::arbitrary_take_rest(data)?;

        Ok(Self { syntax, src })
    }

    fn run(&self) -> Result<(), Self::RunError> {
        let Scenario { syntax, src } = self;
        Ast::from_str(src, None, syntax)?;
        Ok(())
    }
}

#[derive(Arbitrary, Default)]
struct ArbitrarySyntax<'a>(Option<[Option<&'a str>; 6]>);

impl<'a> ArbitrarySyntax<'a> {
    fn as_syntax(&self) -> Syntax<'a> {
        let default = Syntax::default();
        let values = self.0.unwrap_or_default();
        Syntax {
            block_start: values[0].unwrap_or(default.block_start),
            block_end: values[1].unwrap_or(default.block_end),
            expr_start: values[2].unwrap_or(default.expr_start),
            expr_end: values[3].unwrap_or(default.expr_end),
            comment_start: values[4].unwrap_or(default.comment_start),
            comment_end: values[5].unwrap_or(default.comment_end),
        }
    }
}
