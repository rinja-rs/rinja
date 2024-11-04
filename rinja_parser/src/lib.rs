#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(elided_lifetimes_in_paths)]
#![deny(unreachable_pub)]

use std::borrow::Cow;
use std::cell::Cell;
use std::env::current_dir;
use std::ops::{Deref, DerefMut};
use std::path::Path;
use std::sync::Arc;
use std::{fmt, str};

use winnow::ascii::escaped;
use winnow::combinator::{alt, cut_err, delimited, fail, not, opt, peek, preceded, repeat};
use winnow::error::{ErrorKind, FromExternalError};
use winnow::stream::{AsChar, Stream as _};
use winnow::token::{any, one_of, take_till1, take_while};
use winnow::{Parser, unpeek};

pub mod expr;
pub use expr::{Expr, Filter};
mod memchr_splitter;
pub mod node;
pub use node::Node;

mod target;
pub use target::Target;
#[cfg(test)]
mod tests;

mod _parsed {
    use std::path::Path;
    use std::sync::Arc;
    use std::{fmt, mem};

    use super::node::Node;
    use super::{Ast, ParseError, Syntax};

    pub struct Parsed {
        // `source` must outlive `ast`, so `ast` must be declared before `source`
        ast: Ast<'static>,
        #[allow(dead_code)]
        source: Arc<str>,
    }

    impl Parsed {
        /// If `file_path` is `None`, it means the `source` is an inline template. Therefore, if
        /// a parsing error occurs, we won't display the path as it wouldn't be useful.
        pub fn new(
            source: Arc<str>,
            file_path: Option<Arc<Path>>,
            syntax: &Syntax<'_>,
        ) -> Result<Self, ParseError> {
            // Self-referential borrowing: `self` will keep the source alive as `String`,
            // internally we will transmute it to `&'static str` to satisfy the compiler.
            // However, we only expose the nodes with a lifetime limited to `self`.
            let src = unsafe { mem::transmute::<&str, &'static str>(source.as_ref()) };
            let ast = Ast::from_str(src, file_path, syntax)?;
            Ok(Self { ast, source })
        }

        // The return value's lifetime must be limited to `self` to uphold the unsafe invariant.
        #[must_use]
        pub fn nodes(&self) -> &[Node<'_>] {
            &self.ast.nodes
        }

        #[must_use]
        pub fn source(&self) -> &str {
            &self.source
        }
    }

    impl fmt::Debug for Parsed {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("Parsed")
                .field("nodes", &self.ast.nodes)
                .finish_non_exhaustive()
        }
    }

    impl PartialEq for Parsed {
        fn eq(&self, other: &Self) -> bool {
            self.ast.nodes == other.ast.nodes
        }
    }

    impl Default for Parsed {
        fn default() -> Self {
            Self {
                ast: Ast::default(),
                source: "".into(),
            }
        }
    }
}

pub use _parsed::Parsed;

#[derive(Debug, Default)]
pub struct Ast<'a> {
    nodes: Vec<Node<'a>>,
}

impl<'a> Ast<'a> {
    /// If `file_path` is `None`, it means the `source` is an inline template. Therefore, if
    /// a parsing error occurs, we won't display the path as it wouldn't be useful.
    pub fn from_str(
        src: &'a str,
        file_path: Option<Arc<Path>>,
        syntax: &Syntax<'_>,
    ) -> Result<Self, ParseError> {
        match Node::parse_template(src, &State::new(syntax)) {
            Ok(("", nodes)) => Ok(Self { nodes }),
            Ok(_) | Err(winnow::error::ErrMode::Incomplete(_)) => unreachable!(),
            Err(
                winnow::error::ErrMode::Backtrack(ErrorContext { span, message, .. })
                | winnow::error::ErrMode::Cut(ErrorContext { span, message, .. }),
            ) => Err(ParseError {
                message,
                offset: span.offset_from(src).unwrap_or_default(),
                file_path,
            }),
        }
    }

    #[must_use]
    pub fn nodes(&self) -> &[Node<'a>] {
        &self.nodes
    }
}

/// Struct used to wrap types with their associated "span" which is used when generating errors
/// in the code generation.
pub struct WithSpan<'a, T> {
    inner: T,
    span: Span<'a>,
}

/// An location in `&'a str`
#[derive(Debug, Clone, Copy)]
pub struct Span<'a>(&'a [u8; 0]);

impl Default for Span<'static> {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

impl<'a> Span<'a> {
    #[inline]
    pub const fn empty() -> Self {
        Self(&[])
    }

    pub fn offset_from(self, start: &'a str) -> Option<usize> {
        let start_range = start.as_bytes().as_ptr_range();
        let this_ptr = self.0.as_slice().as_ptr();
        match start_range.contains(&this_ptr) {
            // SAFETY: we just checked that `this_ptr` is inside `start_range`
            true => Some(unsafe { this_ptr.offset_from(start_range.start) as usize }),
            false => None,
        }
    }

    pub fn as_suffix_of(self, start: &'a str) -> Option<&'a str> {
        let offset = self.offset_from(start)?;
        match start.is_char_boundary(offset) {
            true => Some(&start[offset..]),
            false => None,
        }
    }
}

impl<'a> From<&'a str> for Span<'a> {
    #[inline]
    fn from(value: &'a str) -> Self {
        Self(value[..0].as_bytes().try_into().unwrap())
    }
}

impl<'a, T> WithSpan<'a, T> {
    #[inline]
    pub fn new(inner: T, span: impl Into<Span<'a>>) -> Self {
        Self {
            inner,
            span: span.into(),
        }
    }

    #[inline]
    pub const fn new_without_span(inner: T) -> Self {
        Self {
            inner,
            span: Span::empty(),
        }
    }

    #[inline]
    pub fn span(&self) -> Span<'a> {
        self.span
    }

    #[inline]
    pub fn deconstruct(self) -> (T, Span<'a>) {
        let Self { inner, span } = self;
        (inner, span)
    }
}

impl<T> Deref for WithSpan<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for WithSpan<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: fmt::Debug> fmt::Debug for WithSpan<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

impl<T: Clone> Clone for WithSpan<'_, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            span: self.span,
        }
    }
}

impl<T: PartialEq> PartialEq for WithSpan<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        // We never want to compare the span information.
        self.inner == other.inner
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: Option<Cow<'static, str>>,
    pub offset: usize,
    pub file_path: Option<Arc<Path>>,
}

impl std::error::Error for ParseError {}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ParseError {
            message,
            file_path,
            offset,
        } = self;

        if let Some(message) = message {
            writeln!(f, "{message}")?;
        }

        let path = file_path
            .as_ref()
            .and_then(|path| Some(strip_common(&current_dir().ok()?, path)));
        match path {
            Some(path) => write!(f, "failed to parse template source\n  --> {path}@{offset}"),
            None => write!(f, "failed to parse template source near offset {offset}"),
        }
    }
}

pub(crate) type ParseErr<'a> = winnow::error::ErrMode<ErrorContext<'a>>;
pub(crate) type ParseResult<'a, T = &'a str> = Result<T, ParseErr<'a>>;
pub(crate) type InputParseResult<'a, T = &'a str> = Result<(&'a str, T), ParseErr<'a>>;

/// This type is used to handle `nom` errors and in particular to add custom error messages.
/// It used to generate `ParserError`.
///
/// It cannot be used to replace `ParseError` because it expects a generic, which would make
/// `rinja`'s users experience less good (since this generic is only needed for `nom`).
#[derive(Debug)]
pub(crate) struct ErrorContext<'a> {
    pub(crate) span: Span<'a>,
    pub(crate) message: Option<Cow<'static, str>>,
}

impl<'a> ErrorContext<'a> {
    fn unclosed(kind: &str, tag: &str, span: impl Into<Span<'a>>) -> Self {
        Self::new(format!("unclosed {kind}, missing {tag:?}"), span)
    }

    fn new(message: impl Into<Cow<'static, str>>, span: impl Into<Span<'a>>) -> Self {
        Self {
            span: span.into(),
            message: Some(message.into()),
        }
    }
}

impl<'a> winnow::error::ParserError<&'a str> for ErrorContext<'a> {
    fn from_error_kind(input: &&'a str, _code: ErrorKind) -> Self {
        Self {
            span: (*input).into(),
            message: None,
        }
    }

    fn append(self, _: &&'a str, _: ErrorKind) -> Self {
        self
    }
}

impl<'a, E: std::fmt::Display> FromExternalError<&'a str, E> for ErrorContext<'a> {
    fn from_external_error(input: &&'a str, _kind: ErrorKind, e: E) -> Self {
        Self {
            span: (*input).into(),
            message: Some(Cow::Owned(e.to_string())),
        }
    }
}

impl<'a> From<ErrorContext<'a>> for winnow::error::ErrMode<ErrorContext<'a>> {
    fn from(cx: ErrorContext<'a>) -> Self {
        Self::Cut(cx)
    }
}

#[inline]
fn skip_ws0(i: &str) -> InputParseResult<'_, ()> {
    Ok((i.trim_ascii_start(), ()))
}

#[inline]
fn skip_ws1(i: &str) -> InputParseResult<'_, ()> {
    let j = i.trim_ascii_start();
    if i.len() != j.len() {
        Ok((j, ()))
    } else {
        fail.parse_peek(i)
    }
}

fn ws<'a, O>(
    inner: impl Parser<&'a str, O, ErrorContext<'a>>,
) -> impl Parser<&'a str, O, ErrorContext<'a>> {
    delimited(unpeek(skip_ws0), inner, unpeek(skip_ws0))
}

/// Skips input until `end` was found, but does not consume it.
/// Returns tuple that would be returned when parsing `end`.
fn skip_till<'a, 'b, O>(
    candidate_finder: impl crate::memchr_splitter::Splitter,
    end: impl Parser<&'a str, O, ErrorContext<'a>>,
) -> impl Parser<&'a str, (&'a str, O), ErrorContext<'a>> {
    let mut next = alt((end.map(Some), any.map(|_| None)));
    unpeek(move |mut i: &'a str| {
        loop {
            i = match candidate_finder.split(i) {
                Some((_, i)) => i,
                None => {
                    return Err(winnow::error::ErrMode::Backtrack(ErrorContext::new(
                        "`end` not found`",
                        i,
                    )));
                }
            };
            i = match next.parse_peek(i)? {
                (inclusive, Some(lookahead)) => return Ok((i, (inclusive, lookahead))),
                (inclusive, None) => inclusive,
            };
        }
    })
}

fn keyword(k: &str) -> impl Parser<&str, &str, ErrorContext<'_>> {
    identifier.verify(move |v: &str| v == k)
}

fn identifier<'i>(input: &mut &'i str) -> ParseResult<'i> {
    let start = take_while(1.., |c: char| c.is_alpha() || c == '_' || c >= '\u{0080}');

    let tail = take_while(1.., |c: char| {
        c.is_alphanum() || c == '_' || c >= '\u{0080}'
    });

    (start, opt(tail)).recognize().parse_next(input)
}

fn bool_lit<'i>(i: &mut &'i str) -> ParseResult<'i> {
    alt((keyword("false"), keyword("true"))).parse_next(i)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Num<'a> {
    Int(&'a str, Option<IntKind>),
    Float(&'a str, Option<FloatKind>),
}

fn num_lit<'a>(i: &mut &'a str) -> ParseResult<'a, Num<'a>> {
    fn num_lit_suffix<'a, T: Copy>(
        kind: &'a str,
        list: &[(&str, T)],
        start: &'a str,
        i: &mut &'a str,
    ) -> ParseResult<'a, T> {
        let suffix = identifier.parse_next(i)?;
        if let Some(value) = list
            .iter()
            .copied()
            .find_map(|(name, value)| (name == suffix).then_some(value))
        {
            Ok(value)
        } else {
            Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                format!("unknown {kind} suffix `{suffix}`"),
                start,
            )))
        }
    }

    let start = *i;

    // Equivalent to <https://github.com/rust-lang/rust/blob/e3f909b2bbd0b10db6f164d466db237c582d3045/compiler/rustc_lexer/src/lib.rs#L587-L620>.
    let int_with_base = (opt('-'), |i: &mut _| {
        let (base, kind) = preceded('0', alt(('b'.value(2), 'o'.value(8), 'x'.value(16))))
            .with_recognized()
            .parse_next(i)?;
        match opt(separated_digits(base, false)).parse_next(i)? {
            Some(_) => Ok(()),
            None => Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                format!("expected digits after `{kind}`"),
                start,
            ))),
        }
    });

    // Equivalent to <https://github.com/rust-lang/rust/blob/e3f909b2bbd0b10db6f164d466db237c582d3045/compiler/rustc_lexer/src/lib.rs#L626-L653>:
    // no `_` directly after the decimal point `.`, or between `e` and `+/-`.
    let float = |i: &mut &'a str| -> ParseResult<'a, ()> {
        let has_dot = opt(('.', separated_digits(10, true))).parse_next(i)?;
        let has_exp = opt(|i: &mut _| {
            let (kind, op) = (one_of(['e', 'E']), opt(one_of(['+', '-']))).parse_next(i)?;
            match opt(separated_digits(10, op.is_none())).parse_next(i)? {
                Some(_) => Ok(()),
                None => Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    format!("expected decimal digits, `+` or `-` after exponent `{kind}`"),
                    start,
                ))),
            }
        })
        .parse_next(i)?;
        match (has_dot, has_exp) {
            (Some(_), _) | (_, Some(())) => Ok(()),
            _ => {
                *i = start;
                fail.parse_next(i)
            }
        }
    };

    let num = if let Ok(Some(num)) = opt(int_with_base.recognize()).parse_next(i) {
        let suffix =
            opt(|i: &mut _| num_lit_suffix("integer", INTEGER_TYPES, start, i)).parse_next(i)?;
        Num::Int(num, suffix)
    } else {
        let (float, num) = preceded((opt('-'), separated_digits(10, true)), opt(float))
            .with_recognized()
            .parse_next(i)?;
        if float.is_some() {
            let suffix =
                opt(|i: &mut _| num_lit_suffix("float", FLOAT_TYPES, start, i)).parse_next(i)?;
            Num::Float(num, suffix)
        } else {
            let suffix =
                opt(|i: &mut _| num_lit_suffix("number", NUM_TYPES, start, i)).parse_next(i)?;
            match suffix {
                Some(NumKind::Int(kind)) => Num::Int(num, Some(kind)),
                Some(NumKind::Float(kind)) => Num::Float(num, Some(kind)),
                None => Num::Int(num, None),
            }
        }
    };
    Ok(num)
}

/// Underscore separated digits of the given base, unless `start` is true this may start
/// with an underscore.
fn separated_digits<'a>(
    radix: u32,
    start: bool,
) -> impl Parser<&'a str, &'a str, ErrorContext<'a>> {
    (
        unpeek(move |i: &'a _| match start {
            true => Ok((i, ())),
            false => repeat(0.., '_').parse_peek(i),
        }),
        one_of(move |ch: char| ch.is_digit(radix)),
        repeat(0.., one_of(move |ch: char| ch == '_' || ch.is_digit(radix))).map(|()| ()),
    )
        .recognize()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StrPrefix {
    Binary,
    CLike,
}

impl StrPrefix {
    #[must_use]
    pub fn to_char(self) -> char {
        match self {
            Self::Binary => 'b',
            Self::CLike => 'c',
        }
    }
}

impl fmt::Display for StrPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use std::fmt::Write;

        f.write_char(self.to_char())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrLit<'a> {
    pub prefix: Option<StrPrefix>,
    pub content: &'a str,
}

fn str_lit_without_prefix<'a>(i: &mut &'a str) -> ParseResult<'a> {
    let s = delimited('"', opt(escaped(take_till1(['\\', '"']), '\\', any)), '"').parse_next(i)?;
    Ok(s.unwrap_or_default())
}

fn str_lit<'a>(i: &mut &'a str) -> ParseResult<'a, StrLit<'a>> {
    let (prefix, content) = (opt(alt(('b', 'c'))), str_lit_without_prefix).parse_next(i)?;
    let prefix = match prefix {
        Some('b') => Some(StrPrefix::Binary),
        Some('c') => Some(StrPrefix::CLike),
        _ => None,
    };
    Ok(StrLit { prefix, content })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CharPrefix {
    Binary,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CharLit<'a> {
    pub prefix: Option<CharPrefix>,
    pub content: &'a str,
}

// Information about allowed character escapes is available at:
// <https://doc.rust-lang.org/reference/tokens.html#character-literals>.
fn char_lit<'a>(i: &mut &'a str) -> ParseResult<'a, CharLit<'a>> {
    let start = i.checkpoint();
    let (b_prefix, s) = (
        opt('b'),
        delimited(
            '\'',
            opt(escaped(take_till1(['\\', '\'']), '\\', any)),
            '\'',
        ),
    )
        .parse_next(i)?;

    let Some(s) = s else {
        i.reset(start);
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            "empty character literal",
            *i,
        )));
    };
    let mut is = s;
    let Ok(c) = Char::parse(&mut is) else {
        i.reset(start);
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            "invalid character",
            *i,
        )));
    };

    let (nb, max_value, err1, err2) = match c {
        Char::Literal | Char::Escaped => {
            return Ok(CharLit {
                prefix: b_prefix.map(|_| CharPrefix::Binary),
                content: s,
            });
        }
        Char::AsciiEscape(nb) => (
            nb,
            // `0x7F` is the maximum value for a `\x` escaped character.
            0x7F,
            "invalid character in ascii escape",
            "must be a character in the range [\\x00-\\x7f]",
        ),
        Char::UnicodeEscape(nb) => (
            nb,
            // `0x10FFFF` is the maximum value for a `\u` escaped character.
            0x0010_FFFF,
            "invalid character in unicode escape",
            "unicode escape must be at most 10FFFF",
        ),
    };

    let Ok(nb) = u32::from_str_radix(nb, 16) else {
        i.reset(start);
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(err1, *i)));
    };
    if nb > max_value {
        i.reset(start);
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(err2, *i)));
    }

    Ok(CharLit {
        prefix: b_prefix.map(|_| CharPrefix::Binary),
        content: s,
    })
}

/// Represents the different kinds of char declarations:
#[derive(Copy, Clone)]
enum Char<'a> {
    /// Any character that is not escaped.
    Literal,
    /// An escaped character (like `\n`) which doesn't require any extra check.
    Escaped,
    /// Ascii escape (like `\x12`).
    AsciiEscape(&'a str),
    /// Unicode escape (like `\u{12}`).
    UnicodeEscape(&'a str),
}

impl<'a> Char<'a> {
    fn parse(i: &mut &'a str) -> ParseResult<'a, Self> {
        if i.chars().count() == 1 {
            return any.value(Self::Literal).parse_next(i);
        }
        (
            '\\',
            alt((
                'n'.value(Self::Escaped),
                'r'.value(Self::Escaped),
                't'.value(Self::Escaped),
                '\\'.value(Self::Escaped),
                '0'.value(Self::Escaped),
                '\''.value(Self::Escaped),
                // Not useful but supported by rust.
                '"'.value(Self::Escaped),
                ('x', take_while(2, |c: char| c.is_ascii_hexdigit()))
                    .map(|(_, s)| Self::AsciiEscape(s)),
                (
                    "u{",
                    take_while(1..=6, |c: char| c.is_ascii_hexdigit()),
                    '}',
                )
                    .map(|(_, s, _)| Self::UnicodeEscape(s)),
            )),
        )
            .map(|(_, ch)| ch)
            .parse_next(i)
    }
}

enum PathOrIdentifier<'a> {
    Path(Vec<&'a str>),
    Identifier(&'a str),
}

fn path_or_identifier<'a>(i: &mut &'a str) -> ParseResult<'a, PathOrIdentifier<'a>> {
    let root = ws(opt("::"));
    let tail = opt(repeat(1.., preceded(ws("::"), identifier)).map(|v: Vec<_>| v));

    let (root, start, rest) = (root, identifier, tail).parse_next(i)?;
    let rest = rest.as_deref().unwrap_or_default();

    // The returned identifier can be assumed to be path if:
    // - it is an absolute path (starts with `::`), or
    // - it has multiple components (at least one `::`), or
    // - the first letter is uppercase
    match (root, start, rest) {
        (Some(_), start, tail) => {
            let mut path = Vec::with_capacity(2 + tail.len());
            path.push("");
            path.push(start);
            path.extend(rest);
            Ok(PathOrIdentifier::Path(path))
        }
        (None, name, [])
            if name
                .chars()
                .next()
                .map_or(true, |c| c == '_' || c.is_lowercase()) =>
        {
            Ok(PathOrIdentifier::Identifier(name))
        }
        (None, start, tail) => {
            let mut path = Vec::with_capacity(1 + tail.len());
            path.push(start);
            path.extend(rest);
            Ok(PathOrIdentifier::Path(path))
        }
    }
}

struct State<'a> {
    syntax: &'a Syntax<'a>,
    loop_depth: Cell<usize>,
    level: Cell<Level>,
}

impl<'a> State<'a> {
    fn new(syntax: &'a Syntax<'a>) -> State<'a> {
        State {
            syntax,
            loop_depth: Cell::new(0),
            level: Cell::new(Level::default()),
        }
    }

    fn nest<'b, T, F: Parser<&'b str, T, ErrorContext<'b>>>(
        &self,
        i: &'b str,
        mut callback: F,
    ) -> InputParseResult<'b, T> {
        let prev_level = self.level.get();
        let (_, level) = prev_level.nest(i)?;
        self.level.set(level);
        let ret = callback.parse_peek(i);
        self.level.set(prev_level);
        ret
    }

    fn tag_block_start<'i>(&self, i: &mut &'i str) -> ParseResult<'i, ()> {
        self.syntax.block_start.value(()).parse_next(i)
    }

    fn tag_block_end<'i>(&self, i: &mut &'i str) -> ParseResult<'i, ()> {
        let control = alt((
            self.syntax.block_end.value(None),
            peek(delimited('%', alt(('-', '~', '+')).map(Some), '}')),
            fail, // rollback on partial matches in the previous line
        ))
        .parse_next(i)?;
        if let Some(control) = control {
            let message = format!(
                "unclosed block, you likely meant to apply whitespace control: {:?}",
                format!("{control}{}", self.syntax.block_end),
            );
            Err(ParseErr::backtrack(ErrorContext::new(message, *i).into()))
        } else {
            Ok(())
        }
    }

    fn tag_comment_start<'i>(&self, i: &mut &'i str) -> ParseResult<'i, ()> {
        self.syntax.comment_start.value(()).parse_next(i)
    }

    fn tag_comment_end<'i>(&self, i: &mut &'i str) -> ParseResult<'i, ()> {
        self.syntax.comment_end.value(()).parse_next(i)
    }

    fn tag_expr_start<'i>(&self, i: &mut &'i str) -> ParseResult<'i, ()> {
        self.syntax.expr_start.value(()).parse_next(i)
    }

    fn tag_expr_end<'i>(&self, i: &mut &'i str) -> ParseResult<'i, ()> {
        self.syntax.expr_end.value(()).parse_next(i)
    }

    fn enter_loop(&self) {
        self.loop_depth.set(self.loop_depth.get() + 1);
    }

    fn leave_loop(&self) {
        self.loop_depth.set(self.loop_depth.get() - 1);
    }

    fn is_in_loop(&self) -> bool {
        self.loop_depth.get() > 0
    }
}

#[derive(Default, Hash, PartialEq, Clone, Copy)]
pub struct Syntax<'a>(InnerSyntax<'a>);

// This abstraction ensures that the fields are readable, but not writable.
#[derive(Hash, PartialEq, Clone, Copy)]
pub struct InnerSyntax<'a> {
    pub block_start: &'a str,
    pub block_end: &'a str,
    pub expr_start: &'a str,
    pub expr_end: &'a str,
    pub comment_start: &'a str,
    pub comment_end: &'a str,
}

impl<'a> Deref for Syntax<'a> {
    type Target = InnerSyntax<'a>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for InnerSyntax<'static> {
    fn default() -> Self {
        Self {
            block_start: "{%",
            block_end: "%}",
            expr_start: "{{",
            expr_end: "}}",
            comment_start: "{#",
            comment_end: "#}",
        }
    }
}

impl fmt::Debug for Syntax<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_syntax("Syntax", self, f)
    }
}

impl fmt::Debug for InnerSyntax<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_syntax("InnerSyntax", self, f)
    }
}

fn fmt_syntax(name: &str, inner: &InnerSyntax<'_>, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct(name)
        .field("block_start", &inner.block_start)
        .field("block_end", &inner.block_end)
        .field("expr_start", &inner.expr_start)
        .field("expr_end", &inner.expr_end)
        .field("comment_start", &inner.comment_start)
        .field("comment_end", &inner.comment_end)
        .finish()
}

#[derive(Debug, Default, Clone, Copy, Hash, PartialEq)]
#[cfg_attr(feature = "config", derive(serde::Deserialize))]
pub struct SyntaxBuilder<'a> {
    pub name: &'a str,
    pub block_start: Option<&'a str>,
    pub block_end: Option<&'a str>,
    pub expr_start: Option<&'a str>,
    pub expr_end: Option<&'a str>,
    pub comment_start: Option<&'a str>,
    pub comment_end: Option<&'a str>,
}

impl<'a> SyntaxBuilder<'a> {
    pub fn to_syntax(&self) -> Result<Syntax<'a>, String> {
        let default = InnerSyntax::default();
        let syntax = Syntax(InnerSyntax {
            block_start: self.block_start.unwrap_or(default.block_start),
            block_end: self.block_end.unwrap_or(default.block_end),
            expr_start: self.expr_start.unwrap_or(default.expr_start),
            expr_end: self.expr_end.unwrap_or(default.expr_end),
            comment_start: self.comment_start.unwrap_or(default.comment_start),
            comment_end: self.comment_end.unwrap_or(default.comment_end),
        });

        for (s, k, is_closing) in [
            (syntax.block_start, "opening block", false),
            (syntax.block_end, "closing block", true),
            (syntax.expr_start, "opening expression", false),
            (syntax.expr_end, "closing expression", true),
            (syntax.comment_start, "opening comment", false),
            (syntax.comment_end, "closing comment", true),
        ] {
            if s.len() < 2 {
                return Err(format!(
                    "delimiters must be at least two characters long. \
                        The {k} delimiter ({s:?}) is too short",
                ));
            } else if s.len() > 32 {
                return Err(format!(
                    "delimiters must be at most 32 characters long. \
                        The {k} delimiter ({:?}...) is too long",
                    &s[..(16..=s.len())
                        .find(|&i| s.is_char_boundary(i))
                        .unwrap_or(s.len())],
                ));
            } else if s.chars().any(char::is_whitespace) {
                return Err(format!(
                    "delimiters may not contain white spaces. \
                        The {k} delimiter ({s:?}) contains white spaces",
                ));
            } else if is_closing
                && ['(', '-', '+', '~', '.', '>', '<', '&', '|', '!']
                    .contains(&s.chars().next().unwrap())
            {
                return Err(format!(
                    "closing delimiters may not start with operators. \
                        The {k} delimiter ({s:?}) starts with operator `{}`",
                    s.chars().next().unwrap(),
                ));
            }
        }

        for ((s1, k1), (s2, k2)) in [
            (
                (syntax.block_start, "block"),
                (syntax.expr_start, "expression"),
            ),
            (
                (syntax.block_start, "block"),
                (syntax.comment_start, "comment"),
            ),
            (
                (syntax.expr_start, "expression"),
                (syntax.comment_start, "comment"),
            ),
        ] {
            if s1.starts_with(s2) || s2.starts_with(s1) {
                let (s1, k1, s2, k2) = match s1.len() < s2.len() {
                    true => (s1, k1, s2, k2),
                    false => (s2, k2, s1, k1),
                };
                return Err(format!(
                    "an opening delimiter may not be the prefix of another delimiter. \
                        The {k1} delimiter ({s1:?}) clashes with the {k2} delimiter ({s2:?})",
                ));
            }
        }

        Ok(syntax)
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct Level(u8);

impl Level {
    fn nest(self, i: &str) -> InputParseResult<'_, Level> {
        if self.0 >= Self::MAX_DEPTH {
            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                "your template code is too deeply nested, or last expression is too complex",
                i,
            )));
        }

        Ok((i, Level(self.0 + 1)))
    }

    const MAX_DEPTH: u8 = 128;
}

fn filter<'a>(
    i: &mut &'a str,
    level: &mut Level,
) -> ParseResult<'a, (&'a str, Option<Vec<WithSpan<'a, Expr<'a>>>>)> {
    let start = *i;
    let _ = ws(('|', not('|'))).parse_next(i)?;

    *level = level.nest(start)?.1;
    cut_err((
        ws(identifier),
        opt(unpeek(|i| Expr::arguments(i, *level, false))),
    ))
    .parse_next(i)
}

/// Returns the common parts of two paths.
///
/// The goal of this function is to reduce the path length based on the `base` argument
/// (generally the path where the program is running into). For example:
///
/// ```text
/// current dir: /a/b/c
/// path:        /a/b/c/d/e.txt
/// ```
///
/// `strip_common` will return `d/e.txt`.
#[must_use]
pub fn strip_common(base: &Path, path: &Path) -> String {
    let path = match path.canonicalize() {
        Ok(path) => path,
        Err(_) => return path.display().to_string(),
    };
    let mut components_iter = path.components().peekable();

    for current_path_component in base.components() {
        let Some(path_component) = components_iter.peek() else {
            return path.display().to_string();
        };
        if current_path_component != *path_component {
            break;
        }
        components_iter.next();
    }
    let path_parts = components_iter
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>();
    if path_parts.is_empty() {
        path.display().to_string()
    } else {
        path_parts.join(std::path::MAIN_SEPARATOR_STR)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntKind {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatKind {
    F16,
    F32,
    F64,
    F128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumKind {
    Int(IntKind),
    Float(FloatKind),
}

/// Primitive integer types. Also used as number suffixes.
const INTEGER_TYPES: &[(&str, IntKind)] = &[
    ("i8", IntKind::I8),
    ("i16", IntKind::I16),
    ("i32", IntKind::I32),
    ("i64", IntKind::I64),
    ("i128", IntKind::I128),
    ("isize", IntKind::Isize),
    ("u8", IntKind::U8),
    ("u16", IntKind::U16),
    ("u32", IntKind::U32),
    ("u64", IntKind::U64),
    ("u128", IntKind::U128),
    ("usize", IntKind::Usize),
];

/// Primitive floating point types. Also used as number suffixes.
const FLOAT_TYPES: &[(&str, FloatKind)] = &[
    ("f16", FloatKind::F16),
    ("f32", FloatKind::F32),
    ("f64", FloatKind::F64),
    ("f128", FloatKind::F128),
];

/// Primitive numeric types. Also used as number suffixes.
const NUM_TYPES: &[(&str, NumKind)] = &{
    let mut list = [("", NumKind::Int(IntKind::I8)); INTEGER_TYPES.len() + FLOAT_TYPES.len()];
    let mut i = 0;
    let mut o = 0;
    while i < INTEGER_TYPES.len() {
        let (name, value) = INTEGER_TYPES[i];
        list[o] = (name, NumKind::Int(value));
        i += 1;
        o += 1;
    }
    let mut i = 0;
    while i < FLOAT_TYPES.len() {
        let (name, value) = FLOAT_TYPES[i];
        list[o] = (name, NumKind::Float(value));
        i += 1;
        o += 1;
    }
    list
};

/// Complete list of named primitive types.
const PRIMITIVE_TYPES: &[&str] = &{
    let mut list = [""; NUM_TYPES.len() + 1];
    let mut i = 0;
    let mut o = 0;
    while i < NUM_TYPES.len() {
        list[o] = NUM_TYPES[i].0;
        i += 1;
        o += 1;
    }
    list[o] = "bool";
    list
};

#[cfg(not(windows))]
#[cfg(test)]
mod test {
    use std::path::Path;

    use super::*;

    #[test]
    fn test_strip_common() {
        // Full path is returned instead of empty when the entire path is in common.
        assert_eq!(strip_common(Path::new("home"), Path::new("home")), "home");

        let cwd = std::env::current_dir().expect("current_dir failed");

        // We need actual existing paths for `canonicalize` to work, so let's do that.
        let entry = cwd
            .read_dir()
            .expect("read_dir failed")
            .filter_map(std::result::Result::ok)
            .find(|f| f.path().is_file())
            .expect("no entry");

        // Since they have the complete path in common except for the folder entry name, it should
        // return only the folder entry name.
        assert_eq!(
            strip_common(&cwd, &entry.path()),
            entry.file_name().to_string_lossy()
        );

        // In this case it cannot canonicalize `/a/b/c` so it returns the path as is.
        assert_eq!(strip_common(&cwd, Path::new("/a/b/c")), "/a/b/c");
    }

    #[test]
    fn test_num_lit() {
        // Should fail.
        assert!(num_lit.parse_peek(".").is_err());
        // Should succeed.
        assert_eq!(
            num_lit.parse_peek("1.2E-02").unwrap(),
            ("", Num::Float("1.2E-02", None))
        );
        assert_eq!(
            num_lit.parse_peek("4e3").unwrap(),
            ("", Num::Float("4e3", None)),
        );
        assert_eq!(
            num_lit.parse_peek("4e+_3").unwrap(),
            ("", Num::Float("4e+_3", None)),
        );
        // Not supported because Rust wants a number before the `.`.
        assert!(num_lit.parse_peek(".1").is_err());
        assert!(num_lit.parse_peek(".1E-02").is_err());
        // A `_` directly after the `.` denotes a field.
        assert_eq!(
            num_lit.parse_peek("1._0").unwrap(),
            ("._0", Num::Int("1", None))
        );
        assert_eq!(
            num_lit.parse_peek("1_.0").unwrap(),
            ("", Num::Float("1_.0", None))
        );
        // Not supported (voluntarily because of `1..` syntax).
        assert_eq!(
            num_lit.parse_peek("1.").unwrap(),
            (".", Num::Int("1", None))
        );
        assert_eq!(
            num_lit.parse_peek("1_.").unwrap(),
            (".", Num::Int("1_", None))
        );
        assert_eq!(
            num_lit.parse_peek("1_2.").unwrap(),
            (".", Num::Int("1_2", None))
        );
        // Numbers with suffixes
        assert_eq!(
            num_lit.parse_peek("-1usize").unwrap(),
            ("", Num::Int("-1", Some(IntKind::Usize)))
        );
        assert_eq!(
            num_lit.parse_peek("123_f32").unwrap(),
            ("", Num::Float("123_", Some(FloatKind::F32)))
        );
        assert_eq!(
            num_lit.parse_peek("1_.2_e+_3_f64|into_isize").unwrap(),
            (
                "|into_isize",
                Num::Float("1_.2_e+_3_", Some(FloatKind::F64))
            )
        );
        assert_eq!(
            num_lit.parse_peek("4e3f128").unwrap(),
            ("", Num::Float("4e3", Some(FloatKind::F128))),
        );
    }

    #[test]
    fn test_char_lit() {
        let lit = |s: &'static str| crate::CharLit {
            prefix: None,
            content: s,
        };

        assert_eq!(char_lit.parse_peek("'a'").unwrap(), ("", lit("a")));
        assert_eq!(char_lit.parse_peek("'字'").unwrap(), ("", lit("字")));

        // Escaped single characters.
        assert_eq!(char_lit.parse_peek("'\\\"'").unwrap(), ("", lit("\\\"")));
        assert_eq!(char_lit.parse_peek("'\\''").unwrap(), ("", lit("\\'")));
        assert_eq!(char_lit.parse_peek("'\\t'").unwrap(), ("", lit("\\t")));
        assert_eq!(char_lit.parse_peek("'\\n'").unwrap(), ("", lit("\\n")));
        assert_eq!(char_lit.parse_peek("'\\r'").unwrap(), ("", lit("\\r")));
        assert_eq!(char_lit.parse_peek("'\\0'").unwrap(), ("", lit("\\0")));
        // Escaped ascii characters (up to `0x7F`).
        assert_eq!(char_lit.parse_peek("'\\x12'").unwrap(), ("", lit("\\x12")));
        assert_eq!(char_lit.parse_peek("'\\x02'").unwrap(), ("", lit("\\x02")));
        assert_eq!(char_lit.parse_peek("'\\x6a'").unwrap(), ("", lit("\\x6a")));
        assert_eq!(char_lit.parse_peek("'\\x7F'").unwrap(), ("", lit("\\x7F")));
        // Escaped unicode characters (up to `0x10FFFF`).
        assert_eq!(
            char_lit.parse_peek("'\\u{A}'").unwrap(),
            ("", lit("\\u{A}"))
        );
        assert_eq!(
            char_lit.parse_peek("'\\u{10}'").unwrap(),
            ("", lit("\\u{10}"))
        );
        assert_eq!(
            char_lit.parse_peek("'\\u{aa}'").unwrap(),
            ("", lit("\\u{aa}"))
        );
        assert_eq!(
            char_lit.parse_peek("'\\u{10FFFF}'").unwrap(),
            ("", lit("\\u{10FFFF}"))
        );

        // Check with `b` prefix.
        assert_eq!(
            char_lit.parse_peek("b'a'").unwrap(),
            ("", crate::CharLit {
                prefix: Some(crate::CharPrefix::Binary),
                content: "a"
            })
        );

        // Should fail.
        assert!(char_lit.parse_peek("''").is_err());
        assert!(char_lit.parse_peek("'\\o'").is_err());
        assert!(char_lit.parse_peek("'\\x'").is_err());
        assert!(char_lit.parse_peek("'\\x1'").is_err());
        assert!(char_lit.parse_peek("'\\x80'").is_err());
        assert!(char_lit.parse_peek("'\\u'").is_err());
        assert!(char_lit.parse_peek("'\\u{}'").is_err());
        assert!(char_lit.parse_peek("'\\u{110000}'").is_err());
    }

    #[test]
    fn test_str_lit() {
        assert_eq!(
            str_lit.parse_peek(r#"b"hello""#).unwrap(),
            ("", StrLit {
                prefix: Some(StrPrefix::Binary),
                content: "hello"
            })
        );
        assert_eq!(
            str_lit.parse_peek(r#"c"hello""#).unwrap(),
            ("", StrLit {
                prefix: Some(StrPrefix::CLike),
                content: "hello"
            })
        );
        assert!(str_lit.parse_peek(r#"d"hello""#).is_err());
    }
}

#[test]
fn assert_span_size() {
    assert_eq!(
        std::mem::size_of::<Span<'static>>(),
        std::mem::size_of::<*const ()>()
    );
}
