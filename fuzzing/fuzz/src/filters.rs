use arbitrary::{Arbitrary, Unstructured};
use rinja::filters;

#[derive(Arbitrary, Debug, Clone, Copy)]
pub enum Scenario<'a> {
    Text(Text<'a>),
}

impl<'a> super::Scenario<'a> for Scenario<'a> {
    type RunError = rinja::Error;

    fn new(data: &'a [u8]) -> Result<Self, arbitrary::Error> {
        Self::arbitrary_take_rest(Unstructured::new(data))
    }

    fn run(&self) -> Result<(), Self::RunError> {
        match *self {
            Self::Text(text) => run_text(text),
        }
    }
}

fn run_text(filter: Text<'_>) -> Result<(), rinja::Error> {
    let Text { input, filter } = filter;
    let _ = match filter {
        TextFilter::Capitalize => filters::capitalize(input)?.to_string(),
        TextFilter::Center(a) => filters::center(input, a)?.to_string(),
        TextFilter::Indent(a) => filters::indent(input, a)?.to_string(),
        TextFilter::Linebreaks => filters::linebreaks(input)?.to_string(),
        TextFilter::LinebreaksBr => filters::linebreaksbr(input)?.to_string(),
        TextFilter::Lowercase => filters::lowercase(input)?.to_string(),
        TextFilter::ParagraphBreaks => filters::paragraphbreaks(input)?.to_string(),
        TextFilter::Safe(e) => match e {
            Escaper::Html => filters::safe(input, filters::Html)?.to_string(),
            Escaper::Text => filters::safe(input, filters::Text)?.to_string(),
        },
        TextFilter::Title => filters::title(input)?.to_string(),
        TextFilter::Trim => filters::trim(input)?.to_string(),
        TextFilter::Truncate(a) => filters::truncate(input, a)?.to_string(),
        TextFilter::Uppercase => filters::uppercase(input)?.to_string(),
        TextFilter::Urlencode => filters::urlencode(input)?.to_string(),
        TextFilter::UrlencodeStrict => filters::urlencode_strict(input)?.to_string(),
    };
    Ok(())
}

#[derive(Arbitrary, Debug, Clone, Copy)]
pub struct Text<'a> {
    input: &'a str,
    filter: TextFilter,
}

#[derive(Arbitrary, Debug, Clone, Copy)]
enum TextFilter {
    Capitalize,
    Center(usize),
    Indent(usize),
    Linebreaks,
    LinebreaksBr,
    Lowercase,
    ParagraphBreaks,
    Safe(Escaper),
    Title,
    Trim,
    Truncate(usize),
    Uppercase,
    Urlencode,
    UrlencodeStrict,
}

#[derive(Arbitrary, Debug, Clone, Copy)]
enum Escaper {
    Html,
    Text,
}

// TODO:
// abs
// escape,
// filesizeformat
// fmt
// format
// into_f64
// into_isize
// join
// json
// json_pretty
// wordcount
