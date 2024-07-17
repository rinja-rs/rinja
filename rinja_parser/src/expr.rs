use std::collections::HashSet;
use std::str;

use winnow::Parser;
use winnow::branch::alt;
use winnow::bytes::take_till0;
use winnow::character::complete::digit1;
use winnow::combinator::{consumed, cut_err, fail, map, not, opt, peek, recognize, value};
use winnow::error::{ErrorKind, ParseError as _};
use winnow::multi::{fold_many0, many0, separated0, separated1};
use winnow::sequence::{preceded, terminated};

use crate::{
    CharLit, ErrorContext, Level, Num, ParseResult, PathOrIdentifier, StrLit, WithSpan, char_lit,
    filter, identifier, keyword, not_ws, num_lit, path_or_identifier, str_lit, ws,
};

macro_rules! expr_prec_layer {
    ( $name:ident, $inner:ident, $op:expr ) => {
        fn $name(i: &'a str, level: Level) -> ParseResult<'a, WithSpan<'a, Self>> {
            let (_, level) = level.nest(i)?;
            let start = i;
            let (i, left) = Self::$inner(i, level)?;
            let (i, right) = many0((ws($op), |i| Self::$inner(i, level)))
                .map(|v: Vec<_>| v)
                .parse_next(i)?;
            Ok((
                i,
                right.into_iter().fold(left, |left, (op, right)| {
                    WithSpan::new(Self::BinOp(op, Box::new(left), Box::new(right)), start)
                }),
            ))
        }
    };
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr<'a> {
    BoolLit(bool),
    NumLit(&'a str, Num<'a>),
    StrLit(StrLit<'a>),
    CharLit(CharLit<'a>),
    Var(&'a str),
    Path(Vec<&'a str>),
    Array(Vec<WithSpan<'a, Expr<'a>>>),
    Attr(Box<WithSpan<'a, Expr<'a>>>, &'a str),
    Index(Box<WithSpan<'a, Expr<'a>>>, Box<WithSpan<'a, Expr<'a>>>),
    Filter(Filter<'a>),
    As(Box<WithSpan<'a, Expr<'a>>>, &'a str),
    NamedArgument(&'a str, Box<WithSpan<'a, Expr<'a>>>),
    Unary(&'a str, Box<WithSpan<'a, Expr<'a>>>),
    BinOp(
        &'a str,
        Box<WithSpan<'a, Expr<'a>>>,
        Box<WithSpan<'a, Expr<'a>>>,
    ),
    Range(
        &'a str,
        Option<Box<WithSpan<'a, Expr<'a>>>>,
        Option<Box<WithSpan<'a, Expr<'a>>>>,
    ),
    Group(Box<WithSpan<'a, Expr<'a>>>),
    Tuple(Vec<WithSpan<'a, Expr<'a>>>),
    Call(Box<WithSpan<'a, Expr<'a>>>, Vec<WithSpan<'a, Expr<'a>>>),
    RustMacro(Vec<&'a str>, &'a str),
    Try(Box<WithSpan<'a, Expr<'a>>>),
    /// This variant should never be used directly. It is created when generating filter blocks.
    FilterSource,
    IsDefined(&'a str),
    IsNotDefined(&'a str),
}

impl<'a> Expr<'a> {
    pub(super) fn arguments(
        i: &'a str,
        level: Level,
        is_template_macro: bool,
    ) -> ParseResult<'a, Vec<WithSpan<'a, Self>>> {
        let (_, level) = level.nest(i)?;
        let mut named_arguments = HashSet::new();
        let start = i;

        preceded(
            ws('('),
            cut_err(terminated(
                separated0(
                    ws(move |i| {
                        // Needed to prevent borrowing it twice between this closure and the one
                        // calling `Self::named_arguments`.
                        let named_arguments = &mut named_arguments;
                        let has_named_arguments = !named_arguments.is_empty();

                        let (i, expr) = alt((
                            move |i| {
                                Self::named_argument(
                                    i,
                                    level,
                                    named_arguments,
                                    start,
                                    is_template_macro,
                                )
                            },
                            move |i| Self::parse(i, level),
                        ))
                        .parse_next(i)?;
                        if has_named_arguments && !matches!(*expr, Self::NamedArgument(_, _)) {
                            Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                                "named arguments must always be passed last",
                                start,
                            )))
                        } else {
                            Ok((i, expr))
                        }
                    }),
                    ',',
                ),
                (opt(ws(',')), ')'),
            )),
        )
        .parse_next(i)
    }

    fn named_argument(
        i: &'a str,
        level: Level,
        named_arguments: &mut HashSet<&'a str>,
        start: &'a str,
        is_template_macro: bool,
    ) -> ParseResult<'a, WithSpan<'a, Self>> {
        if !is_template_macro {
            // If this is not a template macro, we don't want to parse named arguments so
            // we instead return an error which will allow to continue the parsing.
            return fail(i);
        }

        let (_, level) = level.nest(i)?;
        let (i, (argument, _, value)) =
            (identifier, ws('='), move |i| Self::parse(i, level)).parse_next(i)?;
        if named_arguments.insert(argument) {
            Ok((
                i,
                WithSpan::new(Self::NamedArgument(argument, Box::new(value)), start),
            ))
        } else {
            Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                format!("named argument `{argument}` was passed more than once"),
                start,
            )))
        }
    }

    pub(super) fn parse(i: &'a str, level: Level) -> ParseResult<'a, WithSpan<'a, Self>> {
        let (_, level) = level.nest(i)?;
        let start = i;
        let range_right =
            move |i| (ws(alt(("..=", ".."))), opt(move |i| Self::or(i, level))).parse_next(i);
        alt((
            map(range_right, |(op, right)| {
                WithSpan::new(Self::Range(op, None, right.map(Box::new)), start)
            }),
            map(
                (move |i| Self::or(i, level), opt(range_right)),
                |(left, right)| match right {
                    Some((op, right)) => WithSpan::new(
                        Self::Range(op, Some(Box::new(left)), right.map(Box::new)),
                        start,
                    ),
                    None => left,
                },
            ),
        ))
        .parse_next(i)
    }

    expr_prec_layer!(or, and, "||");
    expr_prec_layer!(and, compare, "&&");
    expr_prec_layer!(compare, bor, alt(("==", "!=", ">=", ">", "<=", "<",)));
    expr_prec_layer!(bor, bxor, value("|", "bitor"));
    expr_prec_layer!(bxor, band, token_xor);
    expr_prec_layer!(band, shifts, token_bitand);
    expr_prec_layer!(shifts, addsub, alt((">>", "<<")));
    expr_prec_layer!(addsub, muldivmod, alt(("+", "-")));
    expr_prec_layer!(muldivmod, is_as, alt(("*", "/", "%")));

    fn is_as(i: &'a str, level: Level) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let (before_keyword, lhs) = Self::filtered(i, level)?;
        let (j, rhs) = opt(ws(identifier)).parse_next(before_keyword)?;
        let i = match rhs {
            Some("is") => j,
            Some("as") => {
                let (i, target) = opt(identifier).parse_next(j)?;
                let target = target.unwrap_or_default();
                if crate::PRIMITIVE_TYPES.contains(&target) {
                    return Ok((i, WithSpan::new(Self::As(Box::new(lhs), target), start)));
                } else if target.is_empty() {
                    return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                        "`as` operator expects the name of a primitive type on its right-hand side",
                        before_keyword.trim_start(),
                    )));
                } else {
                    return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                        format!(
                            "`as` operator expects the name of a primitive type on its right-hand \
                              side, found `{target}`"
                        ),
                        before_keyword.trim_start(),
                    )));
                }
            }
            _ => return Ok((before_keyword, lhs)),
        };

        let (i, rhs) =
            opt(terminated(opt(keyword("not")), ws(keyword("defined")))).parse_next(i)?;
        let ctor = match rhs {
            None => {
                return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    "expected `defined` or `not defined` after `is`",
                    // We use `start` to show the whole `var is` thing instead of the current token.
                    start,
                )));
            }
            Some(None) => Self::IsDefined,
            Some(Some(_)) => Self::IsNotDefined,
        };
        let var_name = match *lhs {
            Self::Var(var_name) => var_name,
            Self::Attr(_, _) => {
                return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    "`is defined` operator can only be used on variables, not on their fields",
                    start,
                )));
            }
            _ => {
                return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    "`is defined` operator can only be used on variables",
                    start,
                )));
            }
        };
        Ok((i, WithSpan::new(ctor(var_name), start)))
    }

    fn filtered(i: &'a str, mut level: Level) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let (mut i, mut res) = Self::prefix(i, level)?;
        while let (j, Some((name, args))) = opt(|i| filter(i, &mut level)).parse_next(i)? {
            i = j;

            let mut arguments = args.unwrap_or_else(|| Vec::with_capacity(1));
            arguments.insert(0, res);

            res = WithSpan::new(Self::Filter(Filter { name, arguments }), start);
        }
        Ok((i, res))
    }

    fn prefix(i: &'a str, mut level: Level) -> ParseResult<'a, WithSpan<'a, Self>> {
        let (_, nested) = level.nest(i)?;
        let start = i;
        let (i, (ops, mut expr)) = (
            many0(ws(alt(("!", "-", "*", "&")))).map(|v: Vec<_>| v),
            |i| Suffix::parse(i, nested),
        )
            .parse_next(i)?;

        for op in ops.iter().rev() {
            // This is a rare place where we create recursion in the parsed AST
            // without recursing the parser call stack. However, this can lead
            // to stack overflows in drop glue when the AST is very deep.
            level = level.nest(i)?.1;
            expr = WithSpan::new(Self::Unary(op, Box::new(expr)), start);
        }

        Ok((i, expr))
    }

    fn single(i: &'a str, level: Level) -> ParseResult<'a, WithSpan<'a, Self>> {
        let (_, level) = level.nest(i)?;
        alt((
            Self::num,
            Self::str,
            Self::char,
            Self::path_var_bool,
            move |i| Self::array(i, level),
            move |i| Self::group(i, level),
        ))
        .parse_next(i)
    }

    fn group(i: &'a str, level: Level) -> ParseResult<'a, WithSpan<'a, Self>> {
        let (_, level) = level.nest(i)?;
        let start = i;
        let (i, expr) = preceded(ws('('), opt(|i| Self::parse(i, level))).parse_next(i)?;
        let Some(expr) = expr else {
            let (i, _) = ')'.parse_next(i)?;
            return Ok((i, WithSpan::new(Self::Tuple(vec![]), start)));
        };

        let (i, comma) = ws(opt(peek(','))).parse_next(i)?;
        if comma.is_none() {
            let (i, _) = ')'.parse_next(i)?;
            return Ok((i, WithSpan::new(Self::Group(Box::new(expr)), start)));
        }

        let mut exprs = vec![expr];
        let (i, ()) = fold_many0(
            preceded(',', ws(|i| Self::parse(i, level))),
            || (),
            |(), expr| {
                exprs.push(expr);
            },
        )
        .parse_next(i)?;
        let (i, _) = (ws(opt(',')), ')').parse_next(i)?;
        Ok((i, WithSpan::new(Self::Tuple(exprs), start)))
    }

    fn array(i: &'a str, level: Level) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let (i, level) = level.nest(i)?;
        let (i, array) = preceded(
            ws('['),
            cut_err(terminated(
                opt(terminated(
                    separated1(ws(move |i| Self::parse(i, level)), ','),
                    ws(opt(',')),
                )),
                ']',
            )),
        )
        .parse_next(i)?;
        Ok((
            i,
            WithSpan::new(Self::Array(array.unwrap_or_default()), start),
        ))
    }

    fn path_var_bool(i: &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        map(path_or_identifier, |v| match v {
            PathOrIdentifier::Path(v) => Self::Path(v),
            PathOrIdentifier::Identifier("true") => Self::BoolLit(true),
            PathOrIdentifier::Identifier("false") => Self::BoolLit(false),
            PathOrIdentifier::Identifier(v) => Self::Var(v),
        })
        .parse_next(i)
        .map(|(i, expr)| (i, WithSpan::new(expr, start)))
    }

    fn str(i: &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        map(str_lit, |i| WithSpan::new(Self::StrLit(i), start)).parse_next(i)
    }

    fn num(i: &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let (i, (full, num)) = consumed(num_lit).parse_next(i)?;
        Ok((i, WithSpan::new(Expr::NumLit(full, num), start)))
    }

    fn char(i: &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        map(char_lit, |i| WithSpan::new(Self::CharLit(i), start)).parse_next(i)
    }

    #[must_use]
    pub fn contains_bool_lit_or_is_defined(&self) -> bool {
        match self {
            Self::BoolLit(_) | Self::IsDefined(_) | Self::IsNotDefined(_) => true,
            Self::Unary(_, expr) | Self::Group(expr) => expr.contains_bool_lit_or_is_defined(),
            Self::BinOp("&&" | "||", left, right) => {
                left.contains_bool_lit_or_is_defined() || right.contains_bool_lit_or_is_defined()
            }
            Self::NumLit(_, _)
            | Self::StrLit(_)
            | Self::CharLit(_)
            | Self::Var(_)
            | Self::FilterSource
            | Self::RustMacro(_, _)
            | Self::As(_, _)
            | Self::Call(_, _)
            | Self::Range(_, _, _)
            | Self::Try(_)
            | Self::NamedArgument(_, _)
            | Self::Filter(_)
            | Self::Attr(_, _)
            | Self::Index(_, _)
            | Self::Tuple(_)
            | Self::Array(_)
            | Self::BinOp(_, _, _)
            | Self::Path(_) => false,
        }
    }
}

fn token_xor(i: &str) -> ParseResult<'_> {
    let (i, good) = alt((value(true, keyword("xor")), value(false, '^'))).parse_next(i)?;
    if good {
        Ok((i, "^"))
    } else {
        Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            "the binary XOR operator is called `xor` in rinja",
            i,
        )))
    }
}

fn token_bitand(i: &str) -> ParseResult<'_> {
    let (i, good) = alt((
        value(true, keyword("bitand")),
        value(false, ('&', not('&'))),
    ))
    .parse_next(i)?;
    if good {
        Ok((i, "&"))
    } else {
        Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            "the binary AND operator is called `bitand` in rinja",
            i,
        )))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Filter<'a> {
    pub name: &'a str,
    pub arguments: Vec<WithSpan<'a, Expr<'a>>>,
}

enum Suffix<'a> {
    Attr(&'a str),
    Index(WithSpan<'a, Expr<'a>>),
    Call(Vec<WithSpan<'a, Expr<'a>>>),
    // The value is the arguments of the macro call.
    MacroCall(&'a str),
    Try,
}

impl<'a> Suffix<'a> {
    fn parse(i: &'a str, level: Level) -> ParseResult<'a, WithSpan<'a, Expr<'a>>> {
        let (_, level) = level.nest(i)?;
        let (mut i, mut expr) = Expr::single(i, level)?;
        loop {
            let (j, suffix) = opt(alt((
                Self::attr,
                |i| Self::index(i, level),
                |i| Self::call(i, level),
                Self::r#try,
                Self::r#macro,
            )))
            .parse_next(i)?;

            match suffix {
                Some(Self::Attr(attr)) => expr = WithSpan::new(Expr::Attr(expr.into(), attr), i),
                Some(Self::Index(index)) => {
                    expr = WithSpan::new(Expr::Index(expr.into(), index.into()), i);
                }
                Some(Self::Call(args)) => expr = WithSpan::new(Expr::Call(expr.into(), args), i),
                Some(Self::Try) => expr = WithSpan::new(Expr::Try(expr.into()), i),
                Some(Self::MacroCall(args)) => match expr.inner {
                    Expr::Path(path) => expr = WithSpan::new(Expr::RustMacro(path, args), i),
                    Expr::Var(name) => expr = WithSpan::new(Expr::RustMacro(vec![name], args), i),
                    _ => {
                        return Err(
                            winnow::error::ErrMode::from_error_kind(i, ErrorKind::Tag).cut()
                        );
                    }
                },
                None => break,
            }

            i = j;
        }
        Ok((i, expr))
    }

    fn r#macro(i: &'a str) -> ParseResult<'a, Self> {
        fn nested_parenthesis(input: &str) -> ParseResult<'_, ()> {
            let mut nested = 0;
            let mut last = 0;
            let mut in_str = false;
            let mut escaped = false;

            for (i, c) in input.char_indices() {
                if !(c == '(' || c == ')') || !in_str {
                    match c {
                        '(' => nested += 1,
                        ')' => {
                            if nested == 0 {
                                last = i;
                                break;
                            }
                            nested -= 1;
                        }
                        '"' => {
                            if in_str {
                                if !escaped {
                                    in_str = false;
                                }
                            } else {
                                in_str = true;
                            }
                        }
                        '\\' => {
                            escaped = !escaped;
                        }
                        _ => (),
                    }
                }

                if escaped && c != '\\' {
                    escaped = false;
                }
            }

            if nested == 0 {
                Ok((&input[last..], ()))
            } else {
                fail(input)
            }
        }

        preceded(
            (ws('!'), '('),
            cut_err(terminated(
                map(recognize(nested_parenthesis), Self::MacroCall),
                ')',
            )),
        )
        .parse_next(i)
    }

    fn attr(i: &'a str) -> ParseResult<'a, Self> {
        map(
            preceded(ws(('.', not('.'))), cut_err(alt((digit1, identifier)))),
            Self::Attr,
        )
        .parse_next(i)
    }

    fn index(i: &'a str, level: Level) -> ParseResult<'a, Self> {
        let (_, level) = level.nest(i)?;
        map(
            preceded(
                ws('['),
                cut_err(terminated(ws(move |i| Expr::parse(i, level)), ']')),
            ),
            Self::Index,
        )
        .parse_next(i)
    }

    fn call(i: &'a str, level: Level) -> ParseResult<'a, Self> {
        let (_, level) = level.nest(i)?;
        map(move |i| Expr::arguments(i, level, false), Self::Call).parse_next(i)
    }

    fn r#try(i: &'a str) -> ParseResult<'a, Self> {
        map(preceded(take_till0(not_ws), '?'), |_| Self::Try).parse_next(i)
    }
}
