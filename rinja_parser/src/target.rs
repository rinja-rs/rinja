use winnow::Parser;
use winnow::branch::alt;
use winnow::bytes::one_of;
use winnow::combinator::{map_res, opt};
use winnow::multi::separated1;
use winnow::sequence::preceded;

use crate::{
    CharLit, ErrorContext, Num, ParseErr, ParseResult, PathOrIdentifier, State, StrLit, WithSpan,
    bool_lit, char_lit, identifier, keyword, num_lit, path_or_identifier, str_lit, ws,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Target<'a> {
    Name(&'a str),
    Tuple(Vec<&'a str>, Vec<Target<'a>>),
    Array(Vec<&'a str>, Vec<Target<'a>>),
    Struct(Vec<&'a str>, Vec<(&'a str, Target<'a>)>),
    NumLit(&'a str, Num<'a>),
    StrLit(StrLit<'a>),
    CharLit(CharLit<'a>),
    BoolLit(&'a str),
    Path(Vec<&'a str>),
    OrChain(Vec<Target<'a>>),
    Placeholder(&'a str),
    /// The `Option` is the variable name (if any) in `var_name @ ..`.
    Rest(WithSpan<'a, Option<&'a str>>),
}

impl<'a> Target<'a> {
    /// Parses multiple targets with `or` separating them
    pub(super) fn parse(i: &'a str, s: &State<'_>) -> ParseResult<'a, Self> {
        separated1(|i| s.nest(i, |i| Self::parse_one(i, s)), ws("or"))
            .map(|v: Vec<_>| v)
            .map(|mut opts| match opts.len() {
                1 => opts.pop().unwrap(),
                _ => Self::OrChain(opts),
            })
            .parse_next(i)
    }

    /// Parses a single target without an `or`, unless it is wrapped in parentheses.
    fn parse_one(i: &'a str, s: &State<'_>) -> ParseResult<'a, Self> {
        let mut opt_opening_paren = opt(ws('(')).map(|o| o.is_some());
        let mut opt_opening_brace = opt(ws('{')).map(|o| o.is_some());
        let mut opt_opening_bracket = opt(ws('[')).map(|o| o.is_some());

        let (i, lit) = opt(Self::lit).parse_next(i)?;
        if let Some(lit) = lit {
            return Ok((i, lit));
        }

        // match tuples and unused parentheses
        let (i, target_is_tuple) = opt_opening_paren.parse_next(i)?;
        if target_is_tuple {
            let (i, (singleton, mut targets)) = collect_targets(i, s, ')', Self::unnamed)?;
            if singleton {
                return Ok((i, targets.pop().unwrap()));
            }
            return Ok((
                i,
                Self::Tuple(Vec::new(), only_one_rest_pattern(targets, false, "tuple")?),
            ));
        }
        let (i, target_is_array) = opt_opening_bracket.parse_next(i)?;
        if target_is_array {
            let (i, (singleton, mut targets)) = collect_targets(i, s, ']', Self::unnamed)?;
            if singleton {
                return Ok((i, targets.pop().unwrap()));
            }
            return Ok((
                i,
                Self::Array(Vec::new(), only_one_rest_pattern(targets, true, "array")?),
            ));
        }

        let path = |i| {
            map_res(path_or_identifier, |v| match v {
                PathOrIdentifier::Path(v) => Ok(v),
                PathOrIdentifier::Identifier(v) => Err(v),
            })
            .parse_next(i)
        };

        // match structs
        let (i, path) = opt(path).parse_next(i)?;
        if let Some(path) = path {
            let i_before_matching_with = i;
            let (i, _) = opt(ws(keyword("with"))).parse_next(i)?;

            let (i, is_unnamed_struct) = opt_opening_paren.parse_next(i)?;
            if is_unnamed_struct {
                let (i, (_, targets)) = collect_targets(i, s, ')', Self::unnamed)?;
                return Ok((
                    i,
                    Self::Tuple(path, only_one_rest_pattern(targets, false, "struct")?),
                ));
            }

            let (i, is_named_struct) = opt_opening_brace.parse_next(i)?;
            if is_named_struct {
                let (i, (_, targets)) = collect_targets(i, s, '}', Self::named)?;
                return Ok((i, Self::Struct(path, targets)));
            }

            return Ok((i_before_matching_with, Self::Path(path)));
        }

        // neither literal nor struct nor path
        let (new_i, name) = identifier(i)?;
        let target = match name {
            "_" => Self::Placeholder(name),
            _ => verify_name(i, name)?,
        };
        Ok((new_i, target))
    }

    fn lit(i: &'a str) -> ParseResult<'a, Self> {
        alt((
            str_lit.map(Self::StrLit),
            char_lit.map(Self::CharLit),
            num_lit
                .with_recognized()
                .map(|(num, full)| Target::NumLit(full, num)),
            bool_lit.map(Self::BoolLit),
        ))
        .parse_next(i)
    }

    fn unnamed(i: &'a str, s: &State<'_>) -> ParseResult<'a, Self> {
        alt((Self::rest, |i| Self::parse(i, s))).parse_next(i)
    }

    fn named(init_i: &'a str, s: &State<'_>) -> ParseResult<'a, (&'a str, Self)> {
        let (i, rest) = opt(Self::rest.with_recognized()).parse_next(init_i)?;
        if let Some(rest) = rest {
            let (_, chr) = ws(opt(one_of(",:"))).parse_next(i)?;
            if let Some(chr) = chr {
                return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    format!(
                        "unexpected `{chr}` character after `..`\n\
                         note that in a named struct, `..` must come last to ignore other members"
                    ),
                    i,
                )));
            }
            if let Target::Rest(ref s) = rest.0 {
                if s.inner.is_some() {
                    return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                        "`@ ..` cannot be used in struct",
                        s.span,
                    )));
                }
            }
            return Ok((i, (rest.1, rest.0)));
        }

        let (i, (src, target)) =
            (identifier, opt(preceded(ws(':'), |i| Self::parse(i, s)))).parse_next(init_i)?;

        if src == "_" {
            return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                "cannot use placeholder `_` as source in named struct",
                init_i,
            )));
        }

        let target = match target {
            Some(target) => target,
            None => verify_name(init_i, src)?,
        };
        Ok((i, (src, target)))
    }

    fn rest(start: &'a str) -> ParseResult<'a, Self> {
        let (i, (ident, _)) = (opt((identifier, ws('@'))), "..").parse_next(start)?;
        Ok((
            i,
            Self::Rest(WithSpan::new(ident.map(|(ident, _)| ident), start)),
        ))
    }
}

fn verify_name<'a>(
    input: &'a str,
    name: &'a str,
) -> Result<Target<'a>, winnow::error::ErrMode<ErrorContext<'a>>> {
    match name {
        "self" | "writer" => Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            format!("cannot use `{name}` as a name"),
            input,
        ))),
        _ => Ok(Target::Name(name)),
    }
}

fn collect_targets<'a, T>(
    i: &'a str,
    s: &State<'_>,
    delim: char,
    mut one: impl FnMut(&'a str, &State<'_>) -> ParseResult<'a, T>,
) -> ParseResult<'a, (bool, Vec<T>)> {
    let opt_comma = |i| ws(opt(',')).map(|o| o.is_some()).parse_next(i);
    let mut opt_end = |i| ws(opt(one_of(delim))).map(|o| o.is_some()).parse_next(i);

    let (i, has_end) = opt_end.parse_next(i)?;
    if has_end {
        return Ok((i, (false, Vec::new())));
    }

    let (i, targets) = opt(separated1(|i| one(i, s), ws(',')).map(|v: Vec<_>| v)).parse_next(i)?;
    let Some(targets) = targets else {
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
            "expected comma separated list of members",
            i,
        )));
    };

    let (i, (has_comma, has_end)) = (opt_comma, opt_end).parse_next(i)?;
    if !has_end {
        let msg = match has_comma {
            true => format!("expected member, or `{delim}` as terminator"),
            false => format!("expected `,` for more members, or `{delim}` as terminator"),
        };
        return Err(winnow::error::ErrMode::Cut(ErrorContext::new(msg, i)));
    }

    let singleton = !has_comma && targets.len() == 1;
    Ok((i, (singleton, targets)))
}

fn only_one_rest_pattern<'a>(
    targets: Vec<Target<'a>>,
    allow_named_rest: bool,
    type_kind: &str,
) -> Result<Vec<Target<'a>>, ParseErr<'a>> {
    let mut found_rest = false;

    for target in &targets {
        if let Target::Rest(s) = target {
            if !allow_named_rest && s.inner.is_some() {
                return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    "`@ ..` is only allowed in slice patterns",
                    s.span,
                )));
            }
            if found_rest {
                return Err(winnow::error::ErrMode::Cut(ErrorContext::new(
                    format!("`..` can only be used once per {type_kind} pattern"),
                    s.span,
                )));
            }
            found_rest = true;
        }
    }
    Ok(targets)
}
