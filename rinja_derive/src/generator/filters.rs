use std::borrow::Cow;
use std::fmt;

use parser::{Expr, IntKind, Num, Span, StrLit, StrPrefix, TyGenerics, WithSpan};

use super::{Buffer, Context, DisplayWrap, Generator};
use crate::generator::{TargetIsize, TargetUsize};
use crate::{CompileError, MsgValidEscapers};

impl<'a> Generator<'a, '_> {
    pub(crate) fn visit_filter(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        name: &str,
        args: &[WithSpan<'_, Expr<'a>>],
        generics: &[WithSpan<'_, TyGenerics<'_>>],
        node: Span<'_>,
    ) -> Result<DisplayWrap, CompileError> {
        /// Wraps and shadows `$fn` with a function that checks that no generics were supplied,
        /// then calls the original `$fn` without `generics`.
        macro_rules! filters_without_generics {
            ($($fn:ident),+ $(,)?) => { $(
                fn $fn<'a>(
                    this: &mut Generator<'a, '_>,
                    ctx: &Context<'_>,
                    buf: &mut Buffer,
                    name: &str,
                    args: &[WithSpan<'_, Expr<'a>>],
                    generics: &[WithSpan<'_, TyGenerics<'_>>],
                    node: Span<'_>,
                ) -> Result<DisplayWrap, CompileError> {
                    ensure_filter_has_no_generics(ctx, name, generics, node)?;
                    self::$fn(this, ctx, buf, name, args, node)
                }
            )+ };
        }

        filters_without_generics! {
            deref,
            escape,
            humansize,
            fmt,
            format,
            join,
            json,
            linebreaks,
            pluralize,
            r#ref,
            safe,
            urlencode,
            builtin,
        }

        let filter = match name {
            "capitalize" | "center" | "indent" | "lower" | "lowercase" | "title" | "trim"
            | "truncate" | "upper" | "uppercase" | "wordcount" => builtin,
            "deref" => deref,
            "e" | "escape" => escape,
            "filesizeformat" => humansize,
            "fmt" => fmt,
            "format" => format,
            "join" => join,
            "json" | "tojson" => json,
            "linebreaks" | "linebreaksbr" | "paragraphbreaks" => linebreaks,
            "pluralize" => pluralize,
            "ref" => r#ref,
            "safe" => safe,
            "urlencode" | "urlencode_strict" => urlencode,
            "value" => value,
            _ => custom,
        };
        filter(self, ctx, buf, name, args, generics, node)
    }
}

fn custom<'a>(
    this: &mut Generator<'a, '_>,
    ctx: &Context<'_>,
    buf: &mut Buffer,
    name: &str,
    args: &[WithSpan<'_, Expr<'a>>],
    generics: &[WithSpan<'_, TyGenerics<'_>>],
    _node: Span<'_>,
) -> Result<DisplayWrap, CompileError> {
    buf.write(format_args!("filters::{name}"));
    this.visit_call_generics(buf, generics);
    buf.write('(');
    this._visit_args(ctx, buf, args)?;
    buf.write(")?");
    Ok(DisplayWrap::Unwrapped)
}

fn value<'a>(
    this: &mut Generator<'a, '_>,
    ctx: &Context<'_>,
    buf: &mut Buffer,
    _name: &str,
    args: &[WithSpan<'_, Expr<'a>>],
    generics: &[WithSpan<'_, TyGenerics<'_>>],
    node: Span<'_>,
) -> Result<DisplayWrap, CompileError> {
    this._visit_value(ctx, buf, args, generics, node, "`value` filter")
}

fn builtin<'a>(
    this: &mut Generator<'a, '_>,
    ctx: &Context<'_>,
    buf: &mut Buffer,
    name: &str,
    args: &[WithSpan<'_, Expr<'a>>],
    node: Span<'_>,
) -> Result<DisplayWrap, CompileError> {
    if matches!(name, "center" | "truncate") {
        ensure_filter_has_feature_alloc(ctx, name, node)?;
    }
    buf.write(format_args!("rinja::filters::{name}("));
    this._visit_args(ctx, buf, args)?;
    buf.write(")?");
    Ok(DisplayWrap::Unwrapped)
}

fn urlencode<'a>(
    this: &mut Generator<'a, '_>,
    ctx: &Context<'_>,
    buf: &mut Buffer,
    name: &str,
    args: &[WithSpan<'_, Expr<'a>>],
    node: Span<'_>,
) -> Result<DisplayWrap, CompileError> {
    if cfg!(not(feature = "urlencode")) {
        return Err(ctx.generate_error(
            format_args!("the `{name}` filter requires the `urlencode` feature to be enabled"),
            node,
        ));
    }

    let arg = get_filter_argument(ctx, name, args, node)?;
    // Both filters return HTML-safe strings.
    buf.write(format_args!(
        "rinja::filters::HtmlSafeOutput(rinja::filters::{name}(",
    ));
    this._visit_arg(ctx, buf, arg)?;
    buf.write(")?)");
    Ok(DisplayWrap::Unwrapped)
}

fn humansize<'a>(
    this: &mut Generator<'a, '_>,
    ctx: &Context<'_>,
    buf: &mut Buffer,
    name: &str,
    args: &[WithSpan<'_, Expr<'a>>],
    node: Span<'_>,
) -> Result<DisplayWrap, CompileError> {
    let arg = get_filter_argument(ctx, name, args, node)?;
    // All filters return numbers, and any default formatted number is HTML safe.
    buf.write(format_args!(
        "rinja::filters::HtmlSafeOutput(rinja::filters::filesizeformat(\
             rinja::helpers::get_primitive_value(&("
    ));
    this._visit_arg(ctx, buf, arg)?;
    buf.write(")) as rinja::helpers::core::primitive::f32)?)");
    Ok(DisplayWrap::Unwrapped)
}

fn pluralize<'a>(
    this: &mut Generator<'a, '_>,
    ctx: &Context<'_>,
    buf: &mut Buffer,
    name: &str,
    args: &[WithSpan<'_, Expr<'a>>],
    node: Span<'_>,
) -> Result<DisplayWrap, CompileError> {
    const SINGULAR: &WithSpan<'static, Expr<'static>> =
        &WithSpan::new_without_span(Expr::StrLit(StrLit {
            prefix: None,
            content: "",
        }));
    const PLURAL: &WithSpan<'static, Expr<'static>> =
        &WithSpan::new_without_span(Expr::StrLit(StrLit {
            prefix: None,
            content: "s",
        }));

    let (count, sg, pl) = match args {
        [count] => (count, SINGULAR, PLURAL),
        [count, sg] => (count, sg, PLURAL),
        [count, sg, pl] => (count, sg, pl),
        _ => return Err(unexpected_filter_arguments(ctx, name, args, node, 2)),
    };
    if let Some(is_singular) = expr_is_int_lit_plus_minus_one(count) {
        let value = if is_singular { sg } else { pl };
        this._visit_auto_escaped_arg(ctx, buf, value)?;
    } else {
        buf.write("rinja::filters::pluralize(");
        this._visit_arg(ctx, buf, count)?;
        for value in [sg, pl] {
            buf.write(',');
            this._visit_auto_escaped_arg(ctx, buf, value)?;
        }
        buf.write(")?");
    }
    Ok(DisplayWrap::Wrapped)
}

fn linebreaks<'a>(
    this: &mut Generator<'a, '_>,
    ctx: &Context<'_>,
    buf: &mut Buffer,
    name: &str,
    args: &[WithSpan<'_, Expr<'a>>],
    node: Span<'_>,
) -> Result<DisplayWrap, CompileError> {
    ensure_filter_has_feature_alloc(ctx, name, node)?;
    let arg = get_filter_argument(ctx, name, args, node)?;
    buf.write(format_args!(
        "rinja::filters::{name}(&(&&rinja::filters::AutoEscaper::new(&(",
    ));
    this._visit_arg(ctx, buf, arg)?;
    // The input is always HTML escaped, regardless of the selected escaper:
    buf.write("), rinja::filters::Html)).rinja_auto_escape()?)?");
    // The output is marked as HTML safe, not safe in all contexts:
    Ok(DisplayWrap::Unwrapped)
}

fn r#ref<'a>(
    this: &mut Generator<'a, '_>,
    ctx: &Context<'_>,
    buf: &mut Buffer,
    name: &str,
    args: &[WithSpan<'_, Expr<'a>>],
    node: Span<'_>,
) -> Result<DisplayWrap, CompileError> {
    let arg = get_filter_argument(ctx, name, args, node)?;
    buf.write('&');
    this.visit_expr(ctx, buf, arg)?;
    Ok(DisplayWrap::Unwrapped)
}

fn deref<'a>(
    this: &mut Generator<'a, '_>,
    ctx: &Context<'_>,
    buf: &mut Buffer,
    name: &str,
    args: &[WithSpan<'_, Expr<'a>>],
    node: Span<'_>,
) -> Result<DisplayWrap, CompileError> {
    let arg = get_filter_argument(ctx, name, args, node)?;
    buf.write('*');
    this.visit_expr(ctx, buf, arg)?;
    Ok(DisplayWrap::Unwrapped)
}

fn json<'a>(
    this: &mut Generator<'a, '_>,
    ctx: &Context<'_>,
    buf: &mut Buffer,
    name: &str,
    args: &[WithSpan<'_, Expr<'a>>],
    node: Span<'_>,
) -> Result<DisplayWrap, CompileError> {
    if cfg!(not(feature = "serde_json")) {
        return Err(ctx.generate_error(
            "the `json` filter requires the `serde_json` feature to be enabled",
            node,
        ));
    }

    let filter = match args.len() {
        1 => "json",
        2 => "json_pretty",
        _ => return Err(unexpected_filter_arguments(ctx, name, args, node, 1)),
    };
    buf.write(format_args!("rinja::filters::{filter}("));
    this._visit_args(ctx, buf, args)?;
    buf.write(")?");
    Ok(DisplayWrap::Unwrapped)
}

fn safe<'a>(
    this: &mut Generator<'a, '_>,
    ctx: &Context<'_>,
    buf: &mut Buffer,
    name: &str,
    args: &[WithSpan<'_, Expr<'a>>],
    node: Span<'_>,
) -> Result<DisplayWrap, CompileError> {
    let arg = get_filter_argument(ctx, name, args, node)?;
    buf.write("rinja::filters::safe(");
    this._visit_arg(ctx, buf, arg)?;
    buf.write(format_args!(", {})?", this.input.escaper));
    Ok(DisplayWrap::Wrapped)
}

fn escape<'a>(
    this: &mut Generator<'a, '_>,
    ctx: &Context<'_>,
    buf: &mut Buffer,
    name: &str,
    args: &[WithSpan<'_, Expr<'a>>],
    node: Span<'_>,
) -> Result<DisplayWrap, CompileError> {
    let (arg, escaper) = match args {
        [arg] => (arg, this.input.escaper),
        [arg, escaper] => {
            let Expr::StrLit(StrLit {
                ref prefix,
                content,
            }) = **escaper
            else {
                return Err(ctx.generate_error(
                    format_args!("expected string literal for `{name}` filter"),
                    node,
                ));
            };
            if let Some(prefix) = prefix {
                let kind = match prefix {
                    StrPrefix::Binary => "slice",
                    StrPrefix::CLike => "CStr",
                };
                return Err(ctx.generate_error(
                    format_args!("expected string literal for `{name}` filter, got a {kind}"),
                    args[1].span(),
                ));
            }
            let escaper = this
                .input
                .config
                .escapers
                .iter()
                .find_map(|(extensions, path)| {
                    extensions
                        .contains(&Cow::Borrowed(content))
                        .then_some(path.as_ref())
                })
                .ok_or_else(|| {
                    ctx.generate_error(
                        format_args!(
                            "invalid escaper '{content}' for `{name}` filter. {}",
                            MsgValidEscapers(&this.input.config.escapers),
                        ),
                        node,
                    )
                })?;
            (arg, escaper)
        }
        args => return Err(unexpected_filter_arguments(ctx, name, args, node, 1)),
    };
    buf.write("rinja::filters::escape(");
    this._visit_arg(ctx, buf, arg)?;
    buf.write(format_args!(", {escaper})?"));
    Ok(DisplayWrap::Wrapped)
}

fn format<'a>(
    this: &mut Generator<'a, '_>,
    ctx: &Context<'_>,
    buf: &mut Buffer,
    name: &str,
    args: &[WithSpan<'_, Expr<'a>>],
    node: Span<'_>,
) -> Result<DisplayWrap, CompileError> {
    ensure_filter_has_feature_alloc(ctx, name, node)?;
    if let [fmt, args @ ..] = args {
        if let Expr::StrLit(fmt) = &**fmt {
            buf.write("rinja::helpers::alloc::format!(");
            this.visit_str_lit(buf, fmt);
            if !args.is_empty() {
                buf.write(',');
                this._visit_args(ctx, buf, args)?;
            }
            buf.write(')');
            return Ok(DisplayWrap::Unwrapped);
        }
    }
    Err(ctx.generate_error(r#"use filter format like `"a={} b={}"|format(a, b)`"#, node))
}

fn fmt<'a>(
    this: &mut Generator<'a, '_>,
    ctx: &Context<'_>,
    buf: &mut Buffer,
    name: &str,
    args: &[WithSpan<'_, Expr<'a>>],
    node: Span<'_>,
) -> Result<DisplayWrap, CompileError> {
    ensure_filter_has_feature_alloc(ctx, name, node)?;
    if let [arg, fmt] = args {
        if let Expr::StrLit(fmt) = &**fmt {
            buf.write("rinja::helpers::alloc::format!(");
            this.visit_str_lit(buf, fmt);
            buf.write(',');
            this._visit_arg(ctx, buf, arg)?;
            buf.write(')');
            return Ok(DisplayWrap::Unwrapped);
        }
    }
    Err(ctx.generate_error(r#"use filter fmt like `value|fmt("{:?}")`"#, node))
}

fn join<'a>(
    this: &mut Generator<'a, '_>,
    ctx: &Context<'_>,
    buf: &mut Buffer,
    _name: &str,
    args: &[WithSpan<'_, Expr<'a>>],
    _node: Span<'_>,
) -> Result<DisplayWrap, CompileError> {
    buf.write("rinja::filters::join((&");
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            buf.write(", &");
        }
        this.visit_expr(ctx, buf, arg)?;
        if i == 0 {
            buf.write(").into_iter()");
        }
    }
    buf.write(")?");
    Ok(DisplayWrap::Unwrapped)
}

fn ensure_filter_has_feature_alloc(
    ctx: &Context<'_>,
    name: &str,
    node: Span<'_>,
) -> Result<(), CompileError> {
    if !cfg!(feature = "alloc") {
        return Err(ctx.generate_error(
            format_args!("the `{name}` filter requires the `alloc` feature to be enabled"),
            node,
        ));
    }
    Ok(())
}

#[inline]
fn ensure_filter_has_no_generics(
    ctx: &Context<'_>,
    name: &str,
    generics: &[WithSpan<'_, TyGenerics<'_>>],
    node: Span<'_>,
) -> Result<(), CompileError> {
    match generics {
        [] => Ok(()),
        _ => Err(unexpected_filter_generics(ctx, name, node)),
    }
}

#[cold]
fn unexpected_filter_generics(ctx: &Context<'_>, name: &str, node: Span<'_>) -> CompileError {
    ctx.generate_error(format_args!("unexpected generics on filter `{name}`"), node)
}

#[inline]
fn get_filter_argument<'a, 'b>(
    ctx: &Context<'_>,
    name: &str,
    args: &'b [WithSpan<'b, Expr<'a>>],
    node: Span<'_>,
) -> Result<&'b WithSpan<'b, Expr<'a>>, CompileError> {
    match args {
        [arg] => Ok(arg),
        _ => Err(unexpected_filter_arguments(ctx, name, args, node, 0)),
    }
}

#[cold]
fn unexpected_filter_arguments(
    ctx: &Context<'_>,
    name: &str,
    args: &[WithSpan<'_, Expr<'_>>],
    node: Span<'_>,
    at_most: usize,
) -> CompileError {
    #[derive(Debug, Clone, Copy)]
    struct Error<'a> {
        name: &'a str,
        count: usize,
        at_most: usize,
    }

    impl fmt::Display for Error<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "filter `{}` expects ", self.name)?;
            match self.at_most {
                0 => f.write_str("no arguments"),
                1 => f.write_str("at most one optional argument"),
                n => write!(f, "at most {n} optional arguments"),
            }?;
            write!(f, ", got {}", self.count - 1)
        }
    }

    ctx.generate_error(
        Error {
            name,
            count: args.len(),
            at_most,
        },
        node,
    )
}

fn expr_is_int_lit_plus_minus_one(expr: &WithSpan<'_, Expr<'_>>) -> Option<bool> {
    fn is_signed_singular<T: Eq + Default, E>(
        from_str_radix: impl Fn(&str, u32) -> Result<T, E>,
        value: &str,
        plus_one: T,
        minus_one: T,
    ) -> Option<bool> {
        Some([plus_one, minus_one].contains(&from_str_radix(value, 10).ok()?))
    }

    fn is_unsigned_singular<T: Eq + Default, E>(
        from_str_radix: impl Fn(&str, u32) -> Result<T, E>,
        value: &str,
        plus_one: T,
    ) -> Option<bool> {
        Some(from_str_radix(value, 10).ok()? == plus_one)
    }

    macro_rules! impl_match {
        (
            $kind:ident $value:ident;
            $($svar:ident => $sty:ident),*;
            $($uvar:ident => $uty:ident),*;
        ) => {
            match $kind {
                $(
                    Some(IntKind::$svar) => is_signed_singular($sty::from_str_radix, $value, 1, -1),
                )*
                $(
                    Some(IntKind::$uvar) => is_unsigned_singular($uty::from_str_radix, $value, 1),
                )*
                None => match $value.starts_with('-') {
                    true => is_signed_singular(i128::from_str_radix, $value, 1, -1),
                    false => is_unsigned_singular(u128::from_str_radix, $value, 1),
                },
            }
        };
    }

    let Expr::NumLit(_, Num::Int(value, kind)) = **expr else {
        return None;
    };
    impl_match! {
        kind value;
        I8 => i8,
        I16 => i16,
        I32 => i32,
        I64 => i64,
        I128 => i128,
        Isize => TargetIsize;
        U8 => u8,
        U16 => u16,
        U32 => u32,
        U64 => u64,
        U128 => u128,
        Usize => TargetUsize;
    }
}
