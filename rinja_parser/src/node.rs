use std::collections::HashSet;
use std::str;

use winnow::Parser;
use winnow::combinator::{
    alt, cut_err, delimited, eof, fail, not, opt, peek, preceded, repeat, separated1, terminated,
};
use winnow::token::{any, tag};

use crate::memchr_splitter::{Splitter1, Splitter2, Splitter3};
use crate::{
    ErrorContext, Expr, Filter, ParseResult, State, Target, WithSpan, filter, identifier, keyword,
    skip_till, skip_ws0, str_lit_without_prefix, trim_ascii_end, trim_ascii_start, ws,
};

#[derive(Debug, PartialEq)]
pub enum Node<'a> {
    Lit(WithSpan<'a, Lit<'a>>),
    Comment(WithSpan<'a, Comment<'a>>),
    Expr(Ws, WithSpan<'a, Expr<'a>>),
    Call(WithSpan<'a, Call<'a>>),
    Let(WithSpan<'a, Let<'a>>),
    If(WithSpan<'a, If<'a>>),
    Match(WithSpan<'a, Match<'a>>),
    Loop(Box<WithSpan<'a, Loop<'a>>>),
    Extends(WithSpan<'a, Extends<'a>>),
    BlockDef(WithSpan<'a, BlockDef<'a>>),
    Include(WithSpan<'a, Include<'a>>),
    Import(WithSpan<'a, Import<'a>>),
    Macro(WithSpan<'a, Macro<'a>>),
    Raw(WithSpan<'a, Raw<'a>>),
    Break(WithSpan<'a, Ws>),
    Continue(WithSpan<'a, Ws>),
    FilterBlock(WithSpan<'a, FilterBlock<'a>>),
}

impl<'a> Node<'a> {
    pub(super) fn parse_template(i: &'a str, s: &State<'_>) -> ParseResult<'a, Vec<Self>> {
        let (i, result) = match (|i| Self::many(i, s)).parse_next(i) {
            Ok((i, result)) => (i, result),
            Err(err) => {
                if let winnow::error::ErrMode::Backtrack(err) | winnow::error::ErrMode::Cut(err) =
                    &err
                {
                    if err.message.is_none() {
                        opt(|i| unexpected_tag(i, s)).parse_next(err.input)?;
                    }
                }
                return Err(err);
            }
        };
        let (i, _) = opt(|i| unexpected_tag(i, s)).parse_next(i)?;
        let (i, is_eof) = opt(eof).parse_next(i)?;
        if is_eof.is_none() {
            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                "cannot parse entire template\n\
                 you should never encounter this error\n\
                 please report this error to <https://github.com/rinja-rs/rinja/issues>",
                i,
            )));
        }
        Ok((i, result))
    }

    fn many(i: &'a str, s: &State<'_>) -> ParseResult<'a, Vec<Self>> {
        repeat(
            0..,
            alt((
                (|i| Lit::parse(i, s)).map(Self::Lit),
                (|i| Comment::parse(i, s)).map(Self::Comment),
                |i| Self::expr(i, s),
                |i| Self::parse(i, s),
            )),
        )
        .map(|v: Vec<_>| v)
        .parse_next(i)
    }

    fn parse(i: &'a str, s: &State<'_>) -> ParseResult<'a, Self> {
        #[inline]
        fn wrap<'a, T>(
            func: impl FnOnce(T) -> Node<'a>,
            result: ParseResult<'a, T>,
        ) -> ParseResult<'a, Node<'a>> {
            result.map(|(i, n)| (i, func(n)))
        }

        let start = i;
        let (i, tag) = preceded(
            |i| s.tag_block_start(i),
            peek(preceded((opt(Whitespace::parse), skip_ws0), identifier)),
        )
        .parse_next(i)?;

        let func = match tag {
            "call" => |i, s| wrap(Self::Call, Call::parse(i, s)),
            "let" | "set" => |i, s| wrap(Self::Let, Let::parse(i, s)),
            "if" => |i, s| wrap(Self::If, If::parse(i, s)),
            "for" => |i, s| wrap(|n| Self::Loop(Box::new(n)), Loop::parse(i, s)),
            "match" => |i, s| wrap(Self::Match, Match::parse(i, s)),
            "extends" => |i, _s| wrap(Self::Extends, Extends::parse(i)),
            "include" => |i, _s| wrap(Self::Include, Include::parse(i)),
            "import" => |i, _s| wrap(Self::Import, Import::parse(i)),
            "block" => |i, s| wrap(Self::BlockDef, BlockDef::parse(i, s)),
            "macro" => |i, s| wrap(Self::Macro, Macro::parse(i, s)),
            "raw" => |i, s| wrap(Self::Raw, Raw::parse(i, s)),
            "break" => |i, s| Self::r#break(i, s),
            "continue" => |i, s| Self::r#continue(i, s),
            "filter" => |i, s| wrap(Self::FilterBlock, FilterBlock::parse(i, s)),
            _ => return fail.parse_next(start),
        };

        let (i, node) = s.nest(i, |i| func(i, s))?;

        let (i, closed) = cut_node(
            None,
            alt((ws(eof).value(false), (|i| s.tag_block_end(i)).value(true))),
        )
        .parse_next(i)?;
        match closed {
            true => Ok((i, node)),
            false => Err(ErrorContext::unclosed("block", s.syntax.block_end, start).into()),
        }
    }

    fn r#break(i: &'a str, s: &State<'_>) -> ParseResult<'a, Self> {
        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("break")),
            opt(Whitespace::parse),
        );
        let (j, (pws, _, nws)) = p.parse_next(i)?;
        if !s.is_in_loop() {
            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                "you can only `break` inside a `for` loop",
                i,
            )));
        }
        Ok((j, Self::Break(WithSpan::new(Ws(pws, nws), i))))
    }

    fn r#continue(i: &'a str, s: &State<'_>) -> ParseResult<'a, Self> {
        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("continue")),
            opt(Whitespace::parse),
        );
        let (j, (pws, _, nws)) = p.parse_next(i)?;
        if !s.is_in_loop() {
            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                "you can only `continue` inside a `for` loop",
                i,
            )));
        }
        Ok((j, Self::Continue(WithSpan::new(Ws(pws, nws), i))))
    }

    fn expr(i: &'a str, s: &State<'_>) -> ParseResult<'a, Self> {
        let start = i;
        let (i, (pws, expr)) = preceded(
            |i| s.tag_expr_start(i),
            cut_node(
                None,
                (
                    opt(Whitespace::parse),
                    ws(|i| Expr::parse(i, s.level.get())),
                ),
            ),
        )
        .parse_next(i)?;

        let (i, (nws, closed)) = cut_node(
            None,
            (
                opt(Whitespace::parse),
                alt(((|i| s.tag_expr_end(i)).value(true), ws(eof).value(false))),
            ),
        )
        .parse_next(i)?;
        match closed {
            true => Ok((i, Self::Expr(Ws(pws, nws), expr))),
            false => Err(ErrorContext::unclosed("expression", s.syntax.expr_end, start).into()),
        }
    }

    #[must_use]
    pub fn span(&self) -> &str {
        match self {
            Self::Lit(span) => span.span,
            Self::Comment(span) => span.span,
            Self::Expr(_, span) => span.span,
            Self::Call(span) => span.span,
            Self::Let(span) => span.span,
            Self::If(span) => span.span,
            Self::Match(span) => span.span,
            Self::Loop(span) => span.span,
            Self::Extends(span) => span.span,
            Self::BlockDef(span) => span.span,
            Self::Include(span) => span.span,
            Self::Import(span) => span.span,
            Self::Macro(span) => span.span,
            Self::Raw(span) => span.span,
            Self::Break(span) => span.span,
            Self::Continue(span) => span.span,
            Self::FilterBlock(span) => span.span,
        }
    }
}

fn cut_node<'a, O>(
    kind: Option<&'static str>,
    inner: impl Parser<&'a str, O, ErrorContext<'a>>,
) -> impl Parser<&'a str, O, ErrorContext<'a>> {
    let mut inner = cut_err(inner);
    move |i: &'a str| {
        let result = inner.parse_next(i);
        if let Err(winnow::error::ErrMode::Cut(err) | winnow::error::ErrMode::Backtrack(err)) =
            &result
        {
            if err.message.is_none() {
                opt(|i| unexpected_raw_tag(kind, i)).parse_next(err.input)?;
            }
        }
        result
    }
}

fn unexpected_tag<'a>(i: &'a str, s: &State<'_>) -> ParseResult<'a, ()> {
    (
        |i| s.tag_block_start(i),
        opt(Whitespace::parse),
        |i| unexpected_raw_tag(None, i),
    )
        .void()
        .parse_next(i)
}

fn unexpected_raw_tag<'a>(kind: Option<&'static str>, i: &'a str) -> ParseResult<'a, ()> {
    let (_, tag) = ws(identifier).parse_next(i)?;
    let msg = match tag {
        "end" | "elif" | "else" | "when" => match kind {
            Some(kind) => {
                format!("node `{tag}` was not expected in the current context: `{kind}` block")
            }
            None => format!("node `{tag}` was not expected in the current context"),
        },
        tag if tag.starts_with("end") => format!("unexpected closing tag `{tag}`"),
        tag => format!("unknown node `{tag}`"),
    };
    Err(winnow::error::ErrMode::Cut(ErrorContext::new(msg, i)))
}

#[derive(Debug, PartialEq)]
pub struct When<'a> {
    pub ws: Ws,
    pub target: Vec<Target<'a>>,
    pub nodes: Vec<Node<'a>>,
}

impl<'a> When<'a> {
    fn r#else(i: &'a str, s: &State<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let mut p = (
            |i| s.tag_block_start(i),
            opt(Whitespace::parse),
            ws(keyword("else")),
            cut_node(
                Some("match-else"),
                (
                    opt(Whitespace::parse),
                    |i| s.tag_block_end(i),
                    cut_node(Some("match-else"), |i| Node::many(i, s)),
                ),
            ),
        );
        let (i, (_, pws, _, (nws, _, nodes))) = p.parse_next(i)?;
        Ok((
            i,
            WithSpan::new(
                Self {
                    ws: Ws(pws, nws),
                    target: vec![Target::Placeholder("_")],
                    nodes,
                },
                start,
            ),
        ))
    }

    #[allow(clippy::self_named_constructors)]
    fn when(i: &'a str, s: &State<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let endwhen = ws((
            delimited(
                |i| s.tag_block_start(i),
                opt(Whitespace::parse),
                ws(keyword("endwhen")),
            ),
            cut_node(
                Some("match-endwhen"),
                (
                    opt(Whitespace::parse),
                    |i| s.tag_block_end(i),
                    repeat(0.., ws(|i| Comment::parse(i, s))).map(|()| ()),
                ),
            ),
        ))
        .with_recognized()
        .map(|((pws, _), span)| {
            // A comment node is used to pass the whitespace suppressing information to the
            // generator. This way we don't have to fix up the next `when` node or the closing
            // `endmatch`. Any whitespaces after `endwhen` are to be suppressed. Actually, they
            // don't wind up in the AST anyway.
            Node::Comment(WithSpan::new(
                Comment {
                    ws: Ws(pws, Some(Whitespace::Suppress)),
                    content: "",
                },
                span,
            ))
        });
        let mut p = (
            |i| s.tag_block_start(i),
            opt(Whitespace::parse),
            ws(keyword("when")),
            cut_node(
                Some("match-when"),
                (
                    separated1(ws(|i| Target::parse(i, s)), '|'),
                    opt(Whitespace::parse),
                    |i| s.tag_block_end(i),
                    cut_node(Some("match-when"), |i| Node::many(i, s)),
                    opt(endwhen),
                ),
            ),
        );
        let (i, (_, pws, _, (target, nws, _, mut nodes, endwhen))) = p.parse_next(i)?;
        if let Some(endwhen) = endwhen {
            nodes.push(endwhen);
        }
        Ok((
            i,
            WithSpan::new(
                Self {
                    ws: Ws(pws, nws),
                    target,
                    nodes,
                },
                start,
            ),
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct Cond<'a> {
    pub ws: Ws,
    pub cond: Option<CondTest<'a>>,
    pub nodes: Vec<Node<'a>>,
}

impl<'a> Cond<'a> {
    fn parse(i: &'a str, s: &State<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let (i, (_, pws, cond, nws, _, nodes)) = (
            |i| s.tag_block_start(i),
            opt(Whitespace::parse),
            alt((
                preceded(ws(keyword("else")), opt(|i| CondTest::parse(i, s))),
                preceded(
                    ws(keyword("elif")),
                    cut_node(Some("if-elif"), (|i| CondTest::parse_cond(i, s)).map(Some)),
                ),
            )),
            opt(Whitespace::parse),
            cut_node(Some("if"), |i| s.tag_block_end(i)),
            cut_node(Some("if"), |i| Node::many(i, s)),
        )
            .parse_next(i)?;
        Ok((
            i,
            WithSpan::new(
                Self {
                    ws: Ws(pws, nws),
                    cond,
                    nodes,
                },
                start,
            ),
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct CondTest<'a> {
    pub target: Option<Target<'a>>,
    pub expr: WithSpan<'a, Expr<'a>>,
    pub contains_bool_lit_or_is_defined: bool,
}

impl<'a> CondTest<'a> {
    fn parse(i: &'a str, s: &State<'_>) -> ParseResult<'a, Self> {
        preceded(
            ws(keyword("if")),
            cut_node(Some("if"), |i| Self::parse_cond(i, s)),
        )
        .parse_next(i)
    }

    fn parse_cond(i: &'a str, s: &State<'_>) -> ParseResult<'a, Self> {
        let (i, (target, expr)) = (
            opt(delimited(
                ws(alt((keyword("let"), keyword("set")))),
                ws(|i| Target::parse(i, s)),
                ws('='),
            )),
            ws(|i| Expr::parse(i, s.level.get())),
        )
            .parse_next(i)?;
        let contains_bool_lit_or_is_defined = expr.contains_bool_lit_or_is_defined();
        Ok((i, Self {
            target,
            expr,
            contains_bool_lit_or_is_defined,
        }))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Whitespace {
    Preserve,
    Suppress,
    Minimize,
}

impl Whitespace {
    fn parse(i: &str) -> ParseResult<'_, Self> {
        any.verify_map(Self::parse_char).parse_next(i)
    }

    fn parse_char(c: char) -> Option<Self> {
        match c {
            '+' => Some(Self::Preserve),
            '-' => Some(Self::Suppress),
            '~' => Some(Self::Minimize),
            _ => None,
        }
    }
}

fn check_block_start<'a>(
    i: &'a str,
    start: &'a str,
    s: &State<'_>,
    node: &str,
    expected: &str,
) -> ParseResult<'a, ()> {
    if i.is_empty() {
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            format!("expected `{expected}` to terminate `{node}` node, found nothing"),
            start,
        )));
    }
    s.tag_block_start(i)
}

#[derive(Debug, PartialEq)]
pub struct Loop<'a> {
    pub ws1: Ws,
    pub var: Target<'a>,
    pub iter: WithSpan<'a, Expr<'a>>,
    pub cond: Option<WithSpan<'a, Expr<'a>>>,
    pub body: Vec<Node<'a>>,
    pub ws2: Ws,
    pub else_nodes: Vec<Node<'a>>,
    pub ws3: Ws,
}

impl<'a> Loop<'a> {
    fn parse(i: &'a str, s: &State<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        fn content<'a>(i: &'a str, s: &State<'_>) -> ParseResult<'a, Vec<Node<'a>>> {
            s.enter_loop();
            let result = Node::many(i, s);
            s.leave_loop();
            result
        }

        let start = i;
        let if_cond = preceded(
            ws(keyword("if")),
            cut_node(Some("for-if"), ws(|i| Expr::parse(i, s.level.get()))),
        );

        let else_block = |i| {
            let mut p = preceded(
                ws(keyword("else")),
                cut_node(
                    Some("for-else"),
                    (
                        opt(Whitespace::parse),
                        delimited(
                            |i| s.tag_block_end(i),
                            |i| Node::many(i, s),
                            |i| s.tag_block_start(i),
                        ),
                        opt(Whitespace::parse),
                    ),
                ),
            );
            let (i, (pws, nodes, nws)) = p.parse_next(i)?;
            Ok((i, (pws, nodes, nws)))
        };

        let body_and_end = |i| {
            let (i, (body, (_, pws, else_block, _, nws))) = cut_node(
                Some("for"),
                (
                    |i| content(i, s),
                    cut_node(
                        Some("for"),
                        (
                            |i| check_block_start(i, start, s, "for", "endfor"),
                            opt(Whitespace::parse),
                            opt(else_block),
                            end_node("for", "endfor"),
                            opt(Whitespace::parse),
                        ),
                    ),
                ),
            )
            .parse_next(i)?;
            Ok((i, (body, pws, else_block, nws)))
        };

        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("for")),
            cut_node(
                Some("for"),
                (
                    ws(|i| Target::parse(i, s)),
                    ws(keyword("in")),
                    cut_node(
                        Some("for"),
                        (
                            ws(|i| Expr::parse(i, s.level.get())),
                            opt(if_cond),
                            opt(Whitespace::parse),
                            |i| s.tag_block_end(i),
                            body_and_end,
                        ),
                    ),
                ),
            ),
        );
        let (i, (pws1, _, (var, _, (iter, cond, nws1, _, (body, pws2, else_block, nws2))))) =
            p.parse_next(i)?;
        let (nws3, else_nodes, pws3) = else_block.unwrap_or_default();
        Ok((
            i,
            WithSpan::new(
                Self {
                    ws1: Ws(pws1, nws1),
                    var,
                    iter,
                    cond,
                    body,
                    ws2: Ws(pws2, nws3),
                    else_nodes,
                    ws3: Ws(pws3, nws2),
                },
                start,
            ),
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct Macro<'a> {
    pub ws1: Ws,
    pub name: &'a str,
    pub args: Vec<(&'a str, Option<WithSpan<'a, Expr<'a>>>)>,
    pub nodes: Vec<Node<'a>>,
    pub ws2: Ws,
}

fn check_duplicated_name<'a>(
    names: &mut HashSet<&'a str>,
    arg_name: &'a str,
    i: &'a str,
) -> Result<(), crate::ParseErr<'a>> {
    if !names.insert(arg_name) {
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            format!("duplicated argument `{arg_name}`"),
            i,
        )));
    }
    Ok(())
}

impl<'a> Macro<'a> {
    fn parse(i: &'a str, s: &State<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let level = s.level.get();
        #[allow(clippy::type_complexity)]
        let parameters =
            |i| -> ParseResult<'_, Option<Vec<(&str, Option<WithSpan<'_, Expr<'_>>>)>>> {
                let (i, args) = opt(preceded(
                    '(',
                    (
                        opt(terminated(
                            separated1(
                                (
                                    ws(identifier),
                                    opt(preceded('=', ws(|i| Expr::parse(i, level)))),
                                ),
                                ',',
                            ),
                            opt(','),
                        )),
                        ws(opt(')')),
                    ),
                ))
                .parse_next(i)?;
                match args {
                    Some((args, Some(_))) => Ok((i, args)),
                    Some((_, None)) => {
                        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                            "expected `)` to close macro argument list",
                            i,
                        )));
                    }
                    None => Ok((i, None)),
                }
            };

        let start_s = i;
        let mut start = (
            opt(Whitespace::parse),
            ws(keyword("macro")),
            cut_node(
                Some("macro"),
                (ws(identifier), parameters, opt(Whitespace::parse), |i| {
                    s.tag_block_end(i)
                }),
            ),
        );
        let (j, (pws1, _, (name, params, nws1, _))) = start.parse_next(i)?;
        if is_rust_keyword(name) {
            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                format!("'{name}' is not a valid name for a macro"),
                i,
            )));
        }

        if let Some(ref params) = params {
            let mut names = HashSet::new();

            let mut iter = params.iter();
            while let Some((arg_name, default_value)) = iter.next() {
                check_duplicated_name(&mut names, arg_name, i)?;
                if default_value.is_some() {
                    for (new_arg_name, default_value) in iter.by_ref() {
                        check_duplicated_name(&mut names, new_arg_name, i)?;
                        if default_value.is_none() {
                            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                                format!(
                                    "all arguments following `{arg_name}` should have a default \
                                         value, `{new_arg_name}` doesn't have a default value"
                                ),
                                i,
                            )));
                        }
                    }
                }
            }
        }

        let mut end = cut_node(
            Some("macro"),
            (
                |i| Node::many(i, s),
                cut_node(
                    Some("macro"),
                    (
                        |i| check_block_start(i, start_s, s, "macro", "endmacro"),
                        opt(Whitespace::parse),
                        end_node("macro", "endmacro"),
                        cut_node(
                            Some("macro"),
                            preceded(
                                opt(|before| {
                                    let (after, end_name) = ws(identifier).parse_next(before)?;
                                    check_end_name(before, after, name, end_name, "macro")
                                }),
                                opt(Whitespace::parse),
                            ),
                        ),
                    ),
                ),
            ),
        );
        let (i, (contents, (_, pws2, _, nws2))) = end.parse_next(j)?;

        Ok((
            i,
            WithSpan::new(
                Self {
                    ws1: Ws(pws1, nws1),
                    name,
                    args: params.unwrap_or_default(),
                    nodes: contents,
                    ws2: Ws(pws2, nws2),
                },
                start_s,
            ),
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct FilterBlock<'a> {
    pub ws1: Ws,
    pub filters: Filter<'a>,
    pub nodes: Vec<Node<'a>>,
    pub ws2: Ws,
}

impl<'a> FilterBlock<'a> {
    fn parse(i: &'a str, s: &State<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let mut level = s.level.get();
        let start_s = i;
        let mut start = (
            opt(Whitespace::parse),
            ws(keyword("filter")),
            cut_node(
                Some("filter"),
                (
                    ws(identifier),
                    opt(|i| Expr::arguments(i, s.level.get(), false)),
                    repeat(0.., |i| {
                        filter(i, &mut level).map(|(j, (name, params))| (j, (name, params, i)))
                    })
                    .map(|v: Vec<_>| v),
                    ws(|i| Ok((i, ()))),
                    opt(Whitespace::parse),
                    |i| s.tag_block_end(i),
                ),
            ),
        );
        let (i, (pws1, _, (filter_name, params, extra_filters, (), nws1, _))) =
            start.parse_next(i)?;

        let mut arguments = params.unwrap_or_default();
        arguments.insert(0, WithSpan::new(Expr::FilterSource, start_s));
        let mut filters = Filter {
            name: filter_name,
            arguments,
        };
        for (filter_name, args, span) in extra_filters {
            filters = Filter {
                name: filter_name,
                arguments: {
                    let mut args = args.unwrap_or_default();
                    args.insert(0, WithSpan::new(Expr::Filter(filters), span));
                    args
                },
            };
        }

        let mut end = cut_node(
            Some("filter"),
            (
                |i| Node::many(i, s),
                cut_node(
                    Some("filter"),
                    (
                        |i| check_block_start(i, start_s, s, "filter", "endfilter"),
                        opt(Whitespace::parse),
                        end_node("filter", "endfilter"),
                        opt(Whitespace::parse),
                    ),
                ),
            ),
        );
        let (i, (nodes, (_, pws2, _, nws2))) = end.parse_next(i)?;

        Ok((
            i,
            WithSpan::new(
                Self {
                    ws1: Ws(pws1, nws1),
                    filters,
                    nodes,
                    ws2: Ws(pws2, nws2),
                },
                start_s,
            ),
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct Import<'a> {
    pub ws: Ws,
    pub path: &'a str,
    pub scope: &'a str,
}

impl<'a> Import<'a> {
    fn parse(i: &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("import")),
            cut_node(
                Some("import"),
                (
                    ws(str_lit_without_prefix),
                    ws(keyword("as")),
                    cut_node(Some("import"), (ws(identifier), opt(Whitespace::parse))),
                ),
            ),
        );
        let (i, (pws, _, (path, _, (scope, nws)))) = p.parse_next(i)?;
        Ok((
            i,
            WithSpan::new(
                Self {
                    ws: Ws(pws, nws),
                    path,
                    scope,
                },
                start,
            ),
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct Call<'a> {
    pub ws: Ws,
    pub scope: Option<&'a str>,
    pub name: &'a str,
    pub args: Vec<WithSpan<'a, Expr<'a>>>,
}

impl<'a> Call<'a> {
    fn parse(i: &'a str, s: &State<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("call")),
            cut_node(
                Some("call"),
                (
                    opt((ws(identifier), ws("::"))),
                    ws(identifier),
                    opt(ws(|nested| Expr::arguments(nested, s.level.get(), true))),
                    opt(Whitespace::parse),
                ),
            ),
        );
        let (i, (pws, _, (scope, name, args, nws))) = p.parse_next(i)?;
        let scope = scope.map(|(scope, _)| scope);
        let args = args.unwrap_or_default();
        Ok((
            i,
            WithSpan::new(
                Self {
                    ws: Ws(pws, nws),
                    scope,
                    name,
                    args,
                },
                start,
            ),
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct Match<'a> {
    pub ws1: Ws,
    pub expr: WithSpan<'a, Expr<'a>>,
    pub arms: Vec<WithSpan<'a, When<'a>>>,
    pub ws2: Ws,
}

impl<'a> Match<'a> {
    fn parse(i: &'a str, s: &State<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("match")),
            cut_node(
                Some("match"),
                (
                    ws(|i| Expr::parse(i, s.level.get())),
                    opt(Whitespace::parse),
                    |i| s.tag_block_end(i),
                    cut_node(
                        Some("match"),
                        (
                            ws(repeat(0.., ws(|i| Comment::parse(i, s)))).map(|()| ()),
                            repeat(0.., |i| When::when(i, s)).map(|v: Vec<_>| v),
                            cut_node(
                                Some("match"),
                                (
                                    opt(|i| When::r#else(i, s)),
                                    cut_node(
                                        Some("match"),
                                        (
                                            ws(|i| {
                                                check_block_start(i, start, s, "match", "endmatch")
                                            }),
                                            opt(Whitespace::parse),
                                            end_node("match", "endmatch"),
                                            opt(Whitespace::parse),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        );
        let (i, (pws1, _, (expr, nws1, _, (_, mut arms, (else_arm, (_, pws2, _, nws2)))))) =
            p.parse_next(i)?;

        if let Some(arm) = else_arm {
            arms.push(arm);
        }
        if arms.is_empty() {
            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                "`match` nodes must contain at least one `when` node and/or an `else` case",
                start,
            )));
        }

        Ok((
            i,
            WithSpan::new(
                Self {
                    ws1: Ws(pws1, nws1),
                    expr,
                    arms,
                    ws2: Ws(pws2, nws2),
                },
                start,
            ),
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct BlockDef<'a> {
    pub ws1: Ws,
    pub name: &'a str,
    pub nodes: Vec<Node<'a>>,
    pub ws2: Ws,
}

impl<'a> BlockDef<'a> {
    fn parse(i: &'a str, s: &State<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start_s = i;
        let mut start = (
            opt(Whitespace::parse),
            ws(keyword("block")),
            cut_node(
                Some("block"),
                (ws(identifier), opt(Whitespace::parse), |i| {
                    s.tag_block_end(i)
                }),
            ),
        );
        let (i, (pws1, _, (name, nws1, _))) = start.parse_next(i)?;

        let mut end = cut_node(
            Some("block"),
            (
                |i| Node::many(i, s),
                cut_node(
                    Some("block"),
                    (
                        |i| check_block_start(i, start_s, s, "block", "endblock"),
                        opt(Whitespace::parse),
                        end_node("block", "endblock"),
                        cut_node(
                            Some("block"),
                            (
                                opt(|before| {
                                    let (after, end_name) = ws(identifier).parse_next(before)?;
                                    check_end_name(before, after, name, end_name, "block")
                                }),
                                opt(Whitespace::parse),
                            ),
                        ),
                    ),
                ),
            ),
        );
        let (i, (nodes, (_, pws2, _, (_, nws2)))) = end.parse_next(i)?;

        Ok((
            i,
            WithSpan::new(
                BlockDef {
                    ws1: Ws(pws1, nws1),
                    name,
                    nodes,
                    ws2: Ws(pws2, nws2),
                },
                start_s,
            ),
        ))
    }
}

fn check_end_name<'a>(
    before: &'a str,
    after: &'a str,
    name: &'a str,
    end_name: &'a str,
    kind: &str,
) -> ParseResult<'a> {
    if name == end_name {
        return Ok((after, end_name));
    }

    Err(winnow::error::ErrMode::Cut(ErrorContext::new(
        match name.is_empty() && !end_name.is_empty() {
            true => format!("unexpected name `{end_name}` in `end{kind}` tag for unnamed `{kind}`"),
            false => format!("expected name `{name}` in `end{kind}` tag, found `{end_name}`"),
        },
        before,
    )))
}

#[derive(Debug, PartialEq)]
pub struct Lit<'a> {
    pub lws: &'a str,
    pub val: &'a str,
    pub rws: &'a str,
}

impl<'a> Lit<'a> {
    fn parse(i: &'a str, s: &State<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let (i, ()) = not(eof).parse_next(i)?;

        let candidate_finder = Splitter3::new(
            s.syntax.block_start,
            s.syntax.comment_start,
            s.syntax.expr_start,
        );
        let p_start = alt((
            tag(s.syntax.block_start),
            tag(s.syntax.comment_start),
            tag(s.syntax.expr_start),
        ));

        let (i, content) = opt(skip_till(candidate_finder, p_start).recognize()).parse_next(i)?;
        let (i, content) = match content {
            Some("") => {
                // {block,comment,expr}_start follows immediately.
                return fail.parse_next(i);
            }
            Some(content) => (i, content),
            None => ("", i), // there is no {block,comment,expr}_start: take everything
        };
        Ok((i, WithSpan::new(Self::split_ws_parts(content), start)))
    }

    pub(crate) fn split_ws_parts(s: &'a str) -> Self {
        let trimmed_start = trim_ascii_start(s);
        let len_start = s.len() - trimmed_start.len();
        let trimmed = trim_ascii_end(trimmed_start);
        Self {
            lws: &s[..len_start],
            val: trimmed,
            rws: &trimmed_start[trimmed.len()..],
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Raw<'a> {
    pub ws1: Ws,
    pub lit: Lit<'a>,
    pub ws2: Ws,
}

impl<'a> Raw<'a> {
    fn parse(i: &'a str, s: &State<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let endraw = (
            |i| s.tag_block_start(i),
            opt(Whitespace::parse),
            ws(keyword("endraw")), // sic: ignore `{% end %}` in raw blocks
            opt(Whitespace::parse),
            peek(|i| s.tag_block_end(i)),
        );

        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("raw")),
            cut_node(
                Some("raw"),
                (
                    opt(Whitespace::parse),
                    |i| s.tag_block_end(i),
                    skip_till(Splitter1::new(s.syntax.block_start), endraw).with_recognized(),
                ),
            ),
        );

        let (_, (pws1, _, (nws1, _, ((i, (_, pws2, _, nws2, _)), contents)))) = p.parse_next(i)?;
        let lit = Lit::split_ws_parts(contents);
        let ws1 = Ws(pws1, nws1);
        let ws2 = Ws(pws2, nws2);
        Ok((i, WithSpan::new(Self { ws1, lit, ws2 }, start)))
    }
}

#[derive(Debug, PartialEq)]
pub struct Let<'a> {
    pub ws: Ws,
    pub var: Target<'a>,
    pub val: Option<WithSpan<'a, Expr<'a>>>,
}

impl<'a> Let<'a> {
    fn parse(i: &'a str, s: &State<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let mut p = (
            opt(Whitespace::parse),
            ws(alt((keyword("let"), keyword("set")))),
            cut_node(
                Some("let"),
                (
                    ws(|i| Target::parse(i, s)),
                    opt(preceded(ws('='), ws(|i| Expr::parse(i, s.level.get())))),
                    opt(Whitespace::parse),
                ),
            ),
        );
        let (i, (pws, _, (var, val, nws))) = p.parse_next(i)?;

        Ok((
            i,
            WithSpan::new(
                Let {
                    ws: Ws(pws, nws),
                    var,
                    val,
                },
                start,
            ),
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct If<'a> {
    pub ws: Ws,
    pub branches: Vec<WithSpan<'a, Cond<'a>>>,
}

impl<'a> If<'a> {
    fn parse(i: &'a str, s: &State<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let mut p = (
            opt(Whitespace::parse),
            |i| CondTest::parse(i, s),
            cut_node(
                Some("if"),
                (
                    opt(Whitespace::parse),
                    |i| s.tag_block_end(i),
                    cut_node(
                        Some("if"),
                        (
                            |i| Node::many(i, s),
                            repeat(0.., |i| Cond::parse(i, s)).map(|v: Vec<_>| v),
                            cut_node(
                                Some("if"),
                                (
                                    |i| check_block_start(i, start, s, "if", "endif"),
                                    opt(Whitespace::parse),
                                    end_node("if", "endif"),
                                    opt(Whitespace::parse),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        );

        let (i, (pws1, cond, (nws1, _, (nodes, elifs, (_, pws2, _, nws2))))) = p.parse_next(i)?;
        let mut branches = vec![WithSpan::new(
            Cond {
                ws: Ws(pws1, nws1),
                cond: Some(cond),
                nodes,
            },
            start,
        )];
        branches.extend(elifs);

        Ok((
            i,
            WithSpan::new(
                Self {
                    ws: Ws(pws2, nws2),
                    branches,
                },
                start,
            ),
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct Include<'a> {
    pub ws: Ws,
    pub path: &'a str,
}

impl<'a> Include<'a> {
    fn parse(i: &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;
        let mut p = (
            opt(Whitespace::parse),
            ws(keyword("include")),
            cut_node(
                Some("include"),
                (ws(str_lit_without_prefix), opt(Whitespace::parse)),
            ),
        );
        let (i, (pws, _, (path, nws))) = p.parse_next(i)?;
        Ok((
            i,
            WithSpan::new(
                Self {
                    ws: Ws(pws, nws),
                    path,
                },
                start,
            ),
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct Extends<'a> {
    pub path: &'a str,
}

impl<'a> Extends<'a> {
    fn parse(i: &'a str) -> ParseResult<'a, WithSpan<'a, Self>> {
        let start = i;

        let (i, (pws, _, (path, nws))) = (
            opt(Whitespace::parse),
            ws(keyword("extends")),
            cut_node(
                Some("extends"),
                (ws(str_lit_without_prefix), opt(Whitespace::parse)),
            ),
        )
            .parse_next(i)?;
        match (pws, nws) {
            (None, None) => Ok((i, WithSpan::new(Self { path }, start))),
            (_, _) => Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                "whitespace control is not allowed on `extends`",
                start,
            ))),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Comment<'a> {
    pub ws: Ws,
    pub content: &'a str,
}

impl<'a> Comment<'a> {
    fn parse(i: &'a str, s: &State<'_>) -> ParseResult<'a, WithSpan<'a, Self>> {
        #[derive(Debug, Clone, Copy)]
        enum Tag {
            Open,
            Close,
        }

        fn tag<'a>(i: &'a str, s: &State<'_>) -> ParseResult<'a, Tag> {
            alt((
                (|i| s.tag_comment_start(i)).value(Tag::Open),
                (|i| s.tag_comment_end(i)).value(Tag::Close),
            ))
            .parse_next(i)
        }

        fn content<'a>(mut i: &'a str, s: &State<'_>) -> ParseResult<'a> {
            let mut depth = 0usize;
            let start = i;
            loop {
                let splitter = Splitter2::new(s.syntax.comment_start, s.syntax.comment_end);
                let (k, tag) = opt(skip_till(splitter, |i| tag(i, s))).parse_next(i)?;
                let Some((j, tag)) = tag else {
                    return Err(
                        ErrorContext::unclosed("comment", s.syntax.comment_end, start).into(),
                    );
                };
                match tag {
                    Tag::Open => match depth.checked_add(1) {
                        Some(new_depth) => depth = new_depth,
                        None => {
                            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                                "too deeply nested comments",
                                start,
                            )));
                        }
                    },
                    Tag::Close => match depth.checked_sub(1) {
                        Some(new_depth) => depth = new_depth,
                        None => return Ok((j, &start[..start.len() - k.len()])),
                    },
                }
                i = j;
            }
        }

        let start = i;
        let (i, content) = preceded(
            |i| s.tag_comment_start(i),
            cut_node(Some("comment"), |i| content(i, s)),
        )
        .parse_next(i)?;

        let mut ws = Ws(None, None);
        if content.len() == 1 && matches!(content, "-" | "+" | "~") {
            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                format!(
                    "ambiguous whitespace stripping\n\
                     use `{}{content} {content}{}` to apply the same whitespace stripping on both \
                     sides",
                    s.syntax.comment_start, s.syntax.comment_end,
                ),
                start,
            )));
        } else if content.len() >= 2 {
            ws.0 = Whitespace::parse_char(content.chars().next().unwrap_or_default());
            ws.1 = Whitespace::parse_char(content.chars().next_back().unwrap_or_default());
        }

        Ok((i, WithSpan::new(Self { ws, content }, start)))
    }
}

/// First field is "minus/plus sign was used on the left part of the item".
///
/// Second field is "minus/plus sign was used on the right part of the item".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ws(pub Option<Whitespace>, pub Option<Whitespace>);

fn end_node<'a, 'g: 'a>(
    node: &'g str,
    expected: &'g str,
) -> impl Parser<&'a str, &'a str, ErrorContext<'a>> + 'g {
    move |start| {
        let (i, actual) = ws(identifier).parse_next(start)?;
        if actual == expected {
            Ok((i, actual))
        } else if actual.starts_with("end") {
            Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                format!("expected `{expected}` to terminate `{node}` node, found `{actual}`"),
                start,
            )))
        } else {
            fail.parse_next(start)
        }
    }
}

#[doc(hidden)]
pub const MAX_KW_LEN: usize = 8;
const MAX_REPL_LEN: usize = MAX_KW_LEN + 2;
#[doc(hidden)]
pub const KWS: &[&[[u8; MAX_REPL_LEN]]] = {
    // FIXME: Replace `u8` with `[core:ascii::Char; MAX_REPL_LEN]` once
    //        <https://github.com/rust-lang/rust/issues/110998> is stable.

    const KW2: &[[u8; MAX_REPL_LEN]] = &[
        *b"r#as______",
        *b"r#do______",
        *b"r#fn______",
        *b"r#if______",
        *b"r#in______",
    ];
    const KW3: &[[u8; MAX_REPL_LEN]] = &[
        *b"r#box_____",
        *b"r#dyn_____",
        *b"r#for_____",
        *b"r#let_____",
        *b"r#mod_____",
        *b"r#mut_____",
        *b"r#pub_____",
        *b"r#ref_____",
        *b"r#try_____",
        *b"r#use_____",
    ];
    const KW4: &[[u8; MAX_REPL_LEN]] = &[
        *b"r#else____",
        *b"r#enum____",
        *b"r#impl____",
        *b"r#move____",
        *b"r#priv____",
        *b"r#true____",
        *b"r#type____",
    ];
    const KW5: &[[u8; MAX_REPL_LEN]] = &[
        *b"r#async___",
        *b"r#await___",
        *b"r#break___",
        *b"r#const___",
        *b"r#crate___",
        *b"r#false___",
        *b"r#final___",
        *b"r#macro___",
        *b"r#match___",
        *b"r#trait___",
        *b"r#where___",
        *b"r#while___",
        *b"r#yield___",
    ];
    const KW6: &[[u8; MAX_REPL_LEN]] = &[
        *b"r#become__",
        *b"r#extern__",
        *b"r#return__",
        *b"r#static__",
        *b"r#struct__",
        *b"r#typeof__",
        *b"r#unsafe__",
    ];
    const KW7: &[[u8; MAX_REPL_LEN]] = &[*b"r#unsized_", *b"r#virtual_"];
    const KW8: &[[u8; MAX_REPL_LEN]] = &[*b"r#abstract", *b"r#continue", *b"r#override"];

    &[&[], &[], KW2, KW3, KW4, KW5, KW6, KW7, KW8]
};

// These ones are only used in the parser, hence why they're private.
const KWS_EXTRA: &[&[[u8; MAX_REPL_LEN]]] = {
    const KW4: &[[u8; MAX_REPL_LEN]] = &[*b"r#loop____", *b"r#self____", *b"r#Self____"];
    const KW5: &[[u8; MAX_REPL_LEN]] = &[*b"r#super___", *b"r#union___"];

    &[&[], &[], &[], &[], KW4, KW5, &[], &[], &[]]
};

fn is_rust_keyword(ident: &str) -> bool {
    fn is_rust_keyword_inner(
        kws: &[&[[u8; MAX_REPL_LEN]]],
        padded_ident: &[u8; MAX_KW_LEN],
        ident_len: usize,
    ) -> bool {
        // Since the individual buckets are quite short, a linear search is faster than a binary search.
        kws[ident_len]
            .iter()
            .any(|&probe| padded_ident == &probe[2..])
    }
    if ident.len() > MAX_KW_LEN {
        return false;
    }
    let ident_len = ident.len();

    let mut padded_ident = [b'_'; MAX_KW_LEN];
    padded_ident[..ident.len()].copy_from_slice(ident.as_bytes());

    is_rust_keyword_inner(KWS, &padded_ident, ident_len)
        || is_rust_keyword_inner(KWS_EXTRA, &padded_ident, ident_len)
}

#[cfg(test)]
mod kws_tests {
    use super::{KWS, KWS_EXTRA, MAX_REPL_LEN, is_rust_keyword};

    fn ensure_utf8_inner(entry: &[&[[u8; MAX_REPL_LEN]]]) {
        for kws in entry {
            for kw in *kws {
                assert!(std::str::from_utf8(kw).is_ok(), "not UTF-8: {kw:?}");
            }
        }
    }

    // Ensure that all strings are UTF-8, because we use `from_utf8_unchecked()` further down.
    #[test]
    fn ensure_utf8() {
        assert_eq!(KWS.len(), KWS_EXTRA.len());
        ensure_utf8_inner(KWS);
        ensure_utf8_inner(KWS_EXTRA);
    }

    #[test]
    fn test_is_rust_keyword() {
        assert!(is_rust_keyword("super"));
        assert!(is_rust_keyword("become"));
        assert!(!is_rust_keyword("supeeeer"));
        assert!(!is_rust_keyword("sur"));
    }
}
