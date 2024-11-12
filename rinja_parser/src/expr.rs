use std::collections::HashSet;
use std::str;

use winnow::ascii::digit1;
use winnow::combinator::{
    alt, cut_err, fail, fold_repeat, not, opt, peek, preceded, repeat, separated0, separated1,
    terminated,
};
use winnow::error::{ErrorKind, ParserError as _};
use winnow::stream::Stream as _;
use winnow::{Parser, unpeek};

use crate::{
    CharLit, ErrorContext, InputParseResult, Level, Num, ParseErr, ParseResult, PathOrIdentifier,
    Span, StrLit, WithSpan, char_lit, filter, identifier, keyword, num_lit, path_or_identifier,
    skip_ws0, skip_ws1, str_lit, ws,
};

macro_rules! expr_prec_layer {
    ( $name:ident, $inner:ident, $op:expr ) => {
        fn $name(i: &'a str, level: Level) -> InputParseResult<'a, WithSpan<'a, Self>> {
            let level = level.nest(i)?;
            let start = i;
            let (i, left) = Self::$inner(i, level)?;
            let (i, right) = repeat(0.., (ws($op), unpeek(|i| Self::$inner(i, level))))
                .map(|v: Vec<_>| v)
                .parse_peek(i)?;
            Ok((
                i,
                right.into_iter().fold(left, |left, (op, right)| {
                    WithSpan::new(Self::BinOp(op, Box::new(left), Box::new(right)), start)
                }),
            ))
        }
    };
}

fn check_expr<'a>(
    expr: &WithSpan<'a, Expr<'a>>,
    allow_underscore: bool,
) -> Result<(), ParseErr<'a>> {
    match &expr.inner {
        Expr::Var("_") if !allow_underscore => Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            "reserved keyword `_` cannot be used here",
            expr.span,
        ))),
        Expr::IsDefined(var) | Expr::IsNotDefined(var) => {
            if *var == "_" {
                Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    "reserved keyword `_` cannot be used here",
                    expr.span,
                )))
            } else {
                Ok(())
            }
        }
        Expr::BoolLit(_)
        | Expr::NumLit(_, _)
        | Expr::StrLit(_)
        | Expr::CharLit(_)
        | Expr::Path(_)
        | Expr::Attr(_, _)
        | Expr::Filter(_)
        | Expr::NamedArgument(_, _)
        | Expr::Var(_)
        | Expr::RustMacro(_, _)
        | Expr::Try(_)
        | Expr::FilterSource => Ok(()),
        Expr::Array(elems) | Expr::Tuple(elems) | Expr::Concat(elems) => {
            for elem in elems {
                check_expr(elem, allow_underscore)?;
            }
            Ok(())
        }
        Expr::Index(elem1, elem2) | Expr::BinOp(_, elem1, elem2) => {
            check_expr(elem1, false)?;
            check_expr(elem2, false)
        }
        Expr::Range(_, elem1, elem2) => {
            if let Some(elem1) = elem1 {
                check_expr(elem1, false)?;
            }
            if let Some(elem2) = elem2 {
                check_expr(elem2, false)?;
            }
            Ok(())
        }
        Expr::As(elem, _) | Expr::Unary(_, elem) | Expr::Group(elem) => check_expr(elem, false),
        Expr::Call(call, args) => {
            check_expr(call, false)?;
            for arg in args {
                check_expr(arg, false)?;
            }
            Ok(())
        }
    }
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
    Concat(Vec<WithSpan<'a, Expr<'a>>>),
}

impl<'a> Expr<'a> {
    pub(super) fn arguments(
        i: &'a str,
        level: Level,
        is_template_macro: bool,
    ) -> InputParseResult<'a, Vec<WithSpan<'a, Self>>> {
        let level = level.nest(i)?;
        let mut named_arguments = HashSet::new();
        let start = i;

        preceded(
            ws('('),
            cut_err(terminated(
                separated0(
                    ws(unpeek(move |i| {
                        // Needed to prevent borrowing it twice between this closure and the one
                        // calling `Self::named_arguments`.
                        let named_arguments = &mut named_arguments;
                        let has_named_arguments = !named_arguments.is_empty();

                        let (i, expr) = alt((
                            unpeek(move |i| {
                                Self::named_argument(
                                    i,
                                    level,
                                    named_arguments,
                                    start,
                                    is_template_macro,
                                )
                            }),
                            unpeek(move |i| Self::parse(i, level, false)),
                        ))
                        .parse_peek(i)?;
                        if has_named_arguments && !matches!(*expr, Self::NamedArgument(_, _)) {
                            Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                                "named arguments must always be passed last",
                                start,
                            )))
                        } else {
                            Ok((i, expr))
                        }
                    })),
                    ',',
                ),
                (opt(ws(',')), ')'),
            )),
        )
        .parse_peek(i)
    }

    fn named_argument(
        i: &'a str,
        level: Level,
        named_arguments: &mut HashSet<&'a str>,
        start: &'a str,
        is_template_macro: bool,
    ) -> InputParseResult<'a, WithSpan<'a, Self>> {
        if !is_template_macro {
            // If this is not a template macro, we don't want to parse named arguments so
            // we instead return an error which will allow to continue the parsing.
            return fail.parse_peek(i);
        }

        let level = level.nest(i)?;
        let (i, (argument, _, value)) = (
            identifier,
            ws('='),
            unpeek(move |i| Self::parse(i, level, false)),
        )
            .parse_peek(i)?;
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

    pub(super) fn parse(
        i: &'a str,
        level: Level,
        allow_underscore: bool,
    ) -> InputParseResult<'a, WithSpan<'a, Self>> {
        let level = level.nest(i)?;
        let start = Span::from(i);
        let range_right = move |i| {
            (
                ws(alt(("..=", ".."))),
                opt(unpeek(move |i| Self::or(i, level))),
            )
                .parse_peek(i)
        };
        let (i, expr) = alt((
            unpeek(range_right).map(move |(op, right)| {
                WithSpan::new(Self::Range(op, None, right.map(Box::new)), start)
            }),
            (
                unpeek(move |i| Self::or(i, level)),
                opt(unpeek(range_right)),
            )
                .map(move |(left, right)| match right {
                    Some((op, right)) => WithSpan::new(
                        Self::Range(op, Some(Box::new(left)), right.map(Box::new)),
                        start,
                    ),
                    None => left,
                }),
        ))
        .parse_peek(i)?;
        check_expr(&expr, allow_underscore)?;
        Ok((i, expr))
    }

    expr_prec_layer!(or, and, "||");
    expr_prec_layer!(and, compare, "&&");
    expr_prec_layer!(compare, bor, alt(("==", "!=", ">=", ">", "<=", "<",)));
    expr_prec_layer!(bor, bxor, "bitor".value("|"));
    expr_prec_layer!(bxor, band, token_xor);
    expr_prec_layer!(band, shifts, token_bitand);
    expr_prec_layer!(shifts, addsub, alt((">>", "<<")));
    expr_prec_layer!(addsub, concat, alt(("+", "-")));

    fn concat(i: &'a str, level: Level) -> InputParseResult<'a, WithSpan<'a, Self>> {
        fn concat_expr(
            i: &str,
            level: Level,
        ) -> InputParseResult<'_, Option<WithSpan<'_, Expr<'_>>>> {
            let ws1 = |i| opt(skip_ws1).parse_peek(i);

            let start = i;
            let (i, data) = opt((
                unpeek(ws1),
                '~',
                unpeek(ws1),
                unpeek(|i| Expr::muldivmod(i, level)),
            ))
            .parse_peek(i)?;
            if let Some((t1, _, t2, expr)) = data {
                if t1.is_none() || t2.is_none() {
                    return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                        "the concat operator `~` must be surrounded by spaces",
                        start,
                    )));
                }
                Ok((i, Some(expr)))
            } else {
                Ok((i, None))
            }
        }

        let start = i;
        let (i, expr) = Self::muldivmod(i, level)?;
        let (mut i, expr2) = concat_expr(i, level)?;
        if let Some(expr2) = expr2 {
            let mut exprs = vec![expr, expr2];
            while let (j, Some(expr)) = concat_expr(i, level)? {
                i = j;
                exprs.push(expr);
            }
            Ok((i, WithSpan::new(Self::Concat(exprs), start)))
        } else {
            Ok((i, expr))
        }
    }

    expr_prec_layer!(muldivmod, is_as, alt(("*", "/", "%")));

    fn is_as(i: &'a str, level: Level) -> InputParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let (i, lhs) = Self::filtered(i, level)?;
        let before_keyword = i;
        let (i, rhs) = opt(ws(identifier)).parse_peek(i)?;
        let i = match rhs {
            Some("is") => i,
            Some("as") => {
                let (i, target) = opt(identifier).parse_peek(i)?;
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
            _ => {
                let i = before_keyword;
                return Ok((i, lhs));
            }
        };

        let (i, rhs) =
            opt(terminated(opt(keyword("not")), ws(keyword("defined")))).parse_peek(i)?;
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

    fn filtered(i: &'a str, mut level: Level) -> InputParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let (mut i, mut res) = Self::prefix(i, level)?;
        while let (j, Some((name, args))) = opt(|i: &mut _| filter(i, &mut level)).parse_peek(i)? {
            i = j;

            let mut arguments = args.unwrap_or_else(|| Vec::with_capacity(1));
            arguments.insert(0, res);

            res = WithSpan::new(Self::Filter(Filter { name, arguments }), start);
        }
        Ok((i, res))
    }

    fn prefix(i: &'a str, mut level: Level) -> InputParseResult<'a, WithSpan<'a, Self>> {
        let nested = level.nest(i)?;
        let start = i;
        let (i, (ops, mut expr)) = (
            repeat(0.., ws(alt(("!", "-", "*", "&")))).map(|v: Vec<_>| v),
            unpeek(|i| Suffix::parse(i, nested)),
        )
            .parse_peek(i)?;

        for op in ops.iter().rev() {
            // This is a rare place where we create recursion in the parsed AST
            // without recursing the parser call stack. However, this can lead
            // to stack overflows in drop glue when the AST is very deep.
            level = level.nest(i)?;
            expr = WithSpan::new(Self::Unary(op, Box::new(expr)), start);
        }

        Ok((i, expr))
    }

    fn single(i: &'a str, level: Level) -> InputParseResult<'a, WithSpan<'a, Self>> {
        let level = level.nest(i)?;
        alt((
            Self::num,
            Self::str,
            Self::char,
            unpeek(Self::path_var_bool),
            unpeek(move |i| Self::array(i, level)),
            unpeek(move |i| Self::group(i, level)),
        ))
        .parse_peek(i)
    }

    fn group(i: &'a str, level: Level) -> InputParseResult<'a, WithSpan<'a, Self>> {
        let level = level.nest(i)?;
        let start = i;
        let (i, expr) =
            preceded(ws('('), opt(unpeek(|i| Self::parse(i, level, true)))).parse_peek(i)?;
        let Some(expr) = expr else {
            let (i, _) = ')'.parse_peek(i)?;
            return Ok((i, WithSpan::new(Self::Tuple(vec![]), start)));
        };

        let (i, comma) = ws(opt(peek(','))).parse_peek(i)?;
        if comma.is_none() {
            let (i, _) = ')'.parse_peek(i)?;
            return Ok((i, WithSpan::new(Self::Group(Box::new(expr)), start)));
        }

        let mut exprs = vec![expr];
        let (i, ()) = fold_repeat(
            0..,
            preceded(',', ws(unpeek(|i| Self::parse(i, level, true)))),
            || (),
            |(), expr| {
                exprs.push(expr);
            },
        )
        .parse_peek(i)?;
        let (i, _) = (ws(opt(',')), ')').parse_peek(i)?;
        Ok((i, WithSpan::new(Self::Tuple(exprs), start)))
    }

    fn array(i: &'a str, level: Level) -> InputParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let level = level.nest(i)?;
        let (i, array) = preceded(
            ws('['),
            cut_err(terminated(
                opt(terminated(
                    separated1(ws(unpeek(move |i| Self::parse(i, level, true))), ','),
                    ws(opt(',')),
                )),
                ']',
            )),
        )
        .parse_peek(i)?;
        Ok((
            i,
            WithSpan::new(Self::Array(array.unwrap_or_default()), start),
        ))
    }

    fn path_var_bool(i: &'a str) -> InputParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        path_or_identifier
            .map(|v| match v {
                PathOrIdentifier::Path(v) => Self::Path(v),
                PathOrIdentifier::Identifier("true") => Self::BoolLit(true),
                PathOrIdentifier::Identifier("false") => Self::BoolLit(false),
                PathOrIdentifier::Identifier(v) => Self::Var(v),
            })
            .parse_peek(i)
            .map(|(i, expr)| (i, WithSpan::new(expr, start)))
    }

    fn str(i: &mut &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        str_lit
            .map(|i| WithSpan::new(Self::StrLit(i), start))
            .parse_next(i)
    }

    fn num(i: &mut &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        let (num, full) = num_lit.with_recognized().parse_next(i)?;
        Ok(WithSpan::new(Expr::NumLit(full, num), start))
    }

    fn char(i: &mut &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = *i;
        char_lit
            .map(|i| WithSpan::new(Self::CharLit(i), start))
            .parse_next(i)
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
            | Self::Path(_)
            | Self::Concat(_) => false,
        }
    }
}

fn token_xor<'a>(i: &mut &'a str) -> ParseResult<'a> {
    let good = alt((keyword("xor").value(true), '^'.value(false))).parse_next(i)?;
    if good {
        Ok("^")
    } else {
        Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            "the binary XOR operator is called `xor` in rinja",
            *i,
        )))
    }
}

fn token_bitand<'a>(i: &mut &'a str) -> ParseResult<'a> {
    let good = alt((keyword("bitand").value(true), ('&', not('&')).value(false))).parse_next(i)?;
    if good {
        Ok("&")
    } else {
        Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            "the binary AND operator is called `bitand` in rinja",
            *i,
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
    fn parse(i: &'a str, level: Level) -> InputParseResult<'a, WithSpan<'a, Expr<'a>>> {
        let level = level.nest(i)?;
        let (mut i, mut expr) = Expr::single(i, level)?;
        loop {
            let before_suffix = i;
            let (j, suffix) = opt(alt((
                Self::attr,
                |i: &mut _| Self::index(i, level),
                |i: &mut _| Self::call(i, level),
                Self::r#try,
                Self::r#macro,
            )))
            .parse_peek(i)?;
            i = j;

            match suffix {
                Some(Self::Attr(attr)) => {
                    expr = WithSpan::new(Expr::Attr(expr.into(), attr), before_suffix)
                }
                Some(Self::Index(index)) => {
                    expr = WithSpan::new(Expr::Index(expr.into(), index.into()), before_suffix);
                }
                Some(Self::Call(args)) => {
                    expr = WithSpan::new(Expr::Call(expr.into(), args), before_suffix)
                }
                Some(Self::Try) => expr = WithSpan::new(Expr::Try(expr.into()), before_suffix),
                Some(Self::MacroCall(args)) => match expr.inner {
                    Expr::Path(path) => {
                        expr = WithSpan::new(Expr::RustMacro(path, args), before_suffix)
                    }
                    Expr::Var(name) => {
                        expr = WithSpan::new(Expr::RustMacro(vec![name], args), before_suffix)
                    }
                    _ => {
                        return Err(winnow::error::ErrMode::from_error_kind(
                            &before_suffix,
                            ErrorKind::Tag,
                        )
                        .cut());
                    }
                },
                None => break,
            }
        }
        Ok((i, expr))
    }

    fn r#macro(i: &mut &'a str) -> ParseResult<'a, Self> {
        fn nested_parenthesis<'a>(input: &mut &'a str) -> ParseResult<'a, ()> {
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
                let _ = input.next_slice(last);
                Ok(())
            } else {
                fail.parse_next(input)
            }
        }

        preceded(
            (ws('!'), '('),
            cut_err(terminated(
                nested_parenthesis.recognize().map(Self::MacroCall),
                ')',
            )),
        )
        .parse_next(i)
    }

    fn attr(i: &mut &'a str) -> ParseResult<'a, Self> {
        preceded(ws(('.', not('.'))), cut_err(alt((digit1, identifier))))
            .map(Self::Attr)
            .parse_next(i)
    }

    fn index(i: &mut &'a str, level: Level) -> ParseResult<'a, Self> {
        let level = level.nest(i)?;
        preceded(
            ws('['),
            cut_err(terminated(
                ws(unpeek(move |i| Expr::parse(i, level, true))),
                ']',
            )),
        )
        .map(Self::Index)
        .parse_next(i)
    }

    fn call(i: &mut &'a str, level: Level) -> ParseResult<'a, Self> {
        let level = level.nest(i)?;
        unpeek(move |i| Expr::arguments(i, level, false))
            .map(Self::Call)
            .parse_next(i)
    }

    fn r#try(i: &mut &'a str) -> ParseResult<'a, Self> {
        preceded(skip_ws0, '?').map(|_| Self::Try).parse_next(i)
    }
}
