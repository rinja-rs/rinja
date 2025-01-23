mod expr;
mod node;

use std::borrow::Cow;
use std::collections::hash_map::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::str;
use std::sync::Arc;

use parser::node::{Macro, Whitespace};
use parser::{
    CharLit, Expr, FloatKind, IntKind, MAX_RUST_KEYWORD_LEN, Num, RUST_KEYWORDS, StrLit, WithSpan,
};
use rustc_hash::FxBuildHasher;

use crate::heritage::{Context, Heritage};
use crate::html::write_escaped_str;
use crate::input::{Source, TemplateInput};
use crate::integration::{Buffer, impl_everything, write_header};
use crate::{CompileError, FileInfo};

pub(crate) fn template_to_string(
    buf: &mut Buffer,
    input: &TemplateInput<'_>,
    contexts: &HashMap<&Arc<Path>, Context<'_>, FxBuildHasher>,
    heritage: Option<&Heritage<'_, '_>>,
    target: Option<&str>,
) -> Result<usize, CompileError> {
    let ctx = &contexts[&input.path];
    let generator = Generator::new(
        input,
        contexts,
        heritage,
        MapChain::default(),
        input.block.is_some(),
        0,
    );
    let mut result = generator.build(ctx, buf, target);
    if let Err(err) = &mut result {
        if err.span.is_none() {
            err.span = input.source_span;
        }
    }
    result
}

struct Generator<'a, 'h> {
    /// The template input state: original struct AST and attributes
    input: &'a TemplateInput<'a>,
    /// All contexts, keyed by the package-relative template path
    contexts: &'a HashMap<&'a Arc<Path>, Context<'a>, FxBuildHasher>,
    /// The heritage contains references to blocks and their ancestry
    heritage: Option<&'h Heritage<'a, 'h>>,
    /// Variables accessible directly from the current scope (not redirected to context)
    locals: MapChain<'a>,
    /// Suffix whitespace from the previous literal. Will be flushed to the
    /// output buffer unless suppressed by whitespace suppression on the next
    /// non-literal.
    next_ws: Option<&'a str>,
    /// Whitespace suppression from the previous non-literal. Will be used to
    /// determine whether to flush prefix whitespace from the next literal.
    skip_ws: Whitespace,
    /// If currently in a block, this will contain the name of a potential parent block
    super_block: Option<(&'a str, usize)>,
    /// Buffer for writable
    buf_writable: WritableBuffer<'a>,
    /// Used in blocks to check if we are inside a filter block.
    is_in_filter_block: usize,
    /// Set of called macros we are currently in. Used to prevent (indirect) recursions.
    seen_macros: Vec<(&'a Macro<'a>, Option<FileInfo<'a>>)>,
}

impl<'a, 'h> Generator<'a, 'h> {
    fn new(
        input: &'a TemplateInput<'a>,
        contexts: &'a HashMap<&'a Arc<Path>, Context<'a>, FxBuildHasher>,
        heritage: Option<&'h Heritage<'a, 'h>>,
        locals: MapChain<'a>,
        buf_writable_discard: bool,
        is_in_filter_block: usize,
    ) -> Self {
        Self {
            input,
            contexts,
            heritage,
            locals,
            next_ws: None,
            skip_ws: Whitespace::Preserve,
            super_block: None,
            buf_writable: WritableBuffer {
                discard: buf_writable_discard,
                ..Default::default()
            },
            is_in_filter_block,
            seen_macros: Vec::new(),
        }
    }

    // Takes a Context and generates the relevant implementations.
    fn build(
        mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        target: Option<&str>,
    ) -> Result<usize, CompileError> {
        if target.is_none() {
            buf.write("const _: () = { extern crate rinja as rinja;");
        }
        let size_hint = self.impl_template(ctx, buf, target.unwrap_or("rinja::Template"))?;
        if target.is_none() {
            impl_everything(self.input.ast, buf);
            buf.write("};");
        }
        Ok(size_hint)
    }

    // Implement `Template` for the given context struct.
    fn impl_template(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        target: &str,
    ) -> Result<usize, CompileError> {
        write_header(self.input.ast, buf, target);
        buf.write(
            "fn render_into_with_values<RinjaW>(\
                &self,\
                __rinja_writer: &mut RinjaW,\
                __rinja_values: &dyn rinja::Values\
            ) -> rinja::Result<()>\
            where \
                RinjaW: rinja::helpers::core::fmt::Write + ?rinja::helpers::core::marker::Sized\
            {\
                use rinja::filters::{AutoEscape as _, WriteWritable as _};\
                use rinja::helpers::ResultConverter as _;\
                use rinja::helpers::core::fmt::Write as _;",
        );

        // Make sure the compiler understands that the generated code depends on the template files.
        let mut paths = self
            .contexts
            .keys()
            .map(|path| -> &Path { path })
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            // Skip the fake path of templates defined in rust source.
            let path_is_valid = match self.input.source {
                Source::Path(_) => true,
                Source::Source(_) => path != &*self.input.path,
            };
            if path_is_valid {
                buf.write(format_args!(
                    "const _: &[rinja::helpers::core::primitive::u8] =\
                        rinja::helpers::core::include_bytes!({:#?});",
                    path.canonicalize().as_deref().unwrap_or(path),
                ));
            }
        }

        let size_hint = self.impl_template_inner(ctx, buf)?;

        buf.write(format_args!(
            "\
                rinja::Result::Ok(())\
            }}\
            const SIZE_HINT: rinja::helpers::core::primitive::usize = {size_hint}usize;",
        ));

        buf.write('}');
        Ok(size_hint)
    }

    fn is_var_defined(&self, var_name: &str) -> bool {
        self.locals.get(var_name).is_some() || self.input.fields.iter().any(|f| f == var_name)
    }
}

#[cfg(target_pointer_width = "16")]
type TargetIsize = i16;
#[cfg(target_pointer_width = "32")]
type TargetIsize = i32;
#[cfg(target_pointer_width = "64")]
type TargetIsize = i64;

#[cfg(target_pointer_width = "16")]
type TargetUsize = u16;
#[cfg(target_pointer_width = "32")]
type TargetUsize = u32;
#[cfg(target_pointer_width = "64")]
type TargetUsize = u64;

#[cfg(not(any(
    target_pointer_width = "16",
    target_pointer_width = "32",
    target_pointer_width = "64"
)))]
const _: () = {
    panic!("unknown cfg!(target_pointer_width)");
};

/// In here, we inspect in the expression if it is a literal, and if it is, whether it
/// can be escaped at compile time.
fn compile_time_escape<'a>(expr: &Expr<'a>, escaper: &str) -> Option<Writable<'a>> {
    // we only optimize for known escapers
    enum OutputKind {
        Html,
        Text,
    }

    // we only optimize for known escapers
    let output = match escaper.strip_prefix("rinja::filters::")? {
        "Html" => OutputKind::Html,
        "Text" => OutputKind::Text,
        _ => return None,
    };

    // for now, we only escape strings, chars, numbers, and bools at compile time
    let value = match *expr {
        Expr::StrLit(StrLit {
            prefix: None,
            content,
        }) => {
            if content.find('\\').is_none() {
                // if the literal does not contain any backslashes, then it does not need unescaping
                Cow::Borrowed(content)
            } else {
                // the input could be string escaped if it contains any backslashes
                let input = format!(r#""{content}""#);
                let input = input.parse().ok()?;
                let input = syn::parse2::<syn::LitStr>(input).ok()?;
                Cow::Owned(input.value())
            }
        }
        Expr::CharLit(CharLit {
            prefix: None,
            content,
        }) => {
            if content.find('\\').is_none() {
                // if the literal does not contain any backslashes, then it does not need unescaping
                Cow::Borrowed(content)
            } else {
                // the input could be string escaped if it contains any backslashes
                let input = format!(r#"'{content}'"#);
                let input = input.parse().ok()?;
                let input = syn::parse2::<syn::LitChar>(input).ok()?;
                Cow::Owned(input.value().to_string())
            }
        }
        Expr::NumLit(_, value) => {
            enum NumKind {
                Int(Option<IntKind>),
                Float(Option<FloatKind>),
            }

            let (orig_value, kind) = match value {
                Num::Int(value, kind) => (value, NumKind::Int(kind)),
                Num::Float(value, kind) => (value, NumKind::Float(kind)),
            };
            let value = match orig_value.chars().any(|c| c == '_') {
                true => Cow::Owned(orig_value.chars().filter(|&c| c != '_').collect()),
                false => Cow::Borrowed(orig_value),
            };

            fn int<T: ToString, E>(
                from_str_radix: impl Fn(&str, u32) -> Result<T, E>,
                value: &str,
            ) -> Option<String> {
                Some(from_str_radix(value, 10).ok()?.to_string())
            }

            let value = match kind {
                NumKind::Int(Some(IntKind::I8)) => int(i8::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::I16)) => int(i16::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::I32)) => int(i32::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::I64)) => int(i64::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::I128)) => int(i128::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::Isize)) => int(TargetIsize::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::U8)) => int(u8::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::U16)) => int(u16::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::U32)) => int(u32::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::U64)) => int(u64::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::U128)) => int(u128::from_str_radix, &value)?,
                NumKind::Int(Some(IntKind::Usize)) => int(TargetUsize::from_str_radix, &value)?,
                NumKind::Int(None) => match value.starts_with('-') {
                    true => int(i128::from_str_radix, &value)?,
                    false => int(u128::from_str_radix, &value)?,
                },
                NumKind::Float(Some(FloatKind::F32)) => value.parse::<f32>().ok()?.to_string(),
                NumKind::Float(Some(FloatKind::F64) | None) => {
                    value.parse::<f64>().ok()?.to_string()
                }
                // FIXME: implement once `f16` and `f128` are available
                NumKind::Float(Some(FloatKind::F16 | FloatKind::F128)) => return None,
            };
            match value == orig_value {
                true => Cow::Borrowed(orig_value),
                false => Cow::Owned(value),
            }
        }
        Expr::BoolLit(true) => Cow::Borrowed("true"),
        Expr::BoolLit(false) => Cow::Borrowed("false"),
        _ => return None,
    };

    // escape the un-string-escaped input using the selected escaper
    Some(Writable::Lit(match output {
        OutputKind::Text => value,
        OutputKind::Html => {
            let mut escaped = String::with_capacity(value.len() + 20);
            write_escaped_str(&mut escaped, &value).ok()?;
            match escaped == value {
                true => value,
                false => Cow::Owned(escaped),
            }
        }
    }))
}

#[derive(Clone, Default)]
struct LocalMeta {
    refs: Option<String>,
    initialized: bool,
}

impl LocalMeta {
    fn initialized() -> Self {
        Self {
            refs: None,
            initialized: true,
        }
    }

    fn with_ref(refs: String) -> Self {
        Self {
            refs: Some(refs),
            initialized: true,
        }
    }
}

struct MapChain<'a> {
    scopes: Vec<HashMap<Cow<'a, str>, LocalMeta, FxBuildHasher>>,
}

impl<'a> MapChain<'a> {
    fn new_empty() -> Self {
        Self { scopes: vec![] }
    }

    /// Iterates the scopes in reverse and returns `Some(LocalMeta)`
    /// from the first scope where `key` exists.
    fn get<'b>(&'b self, key: &str) -> Option<&'b LocalMeta> {
        self.scopes.iter().rev().find_map(|set| set.get(key))
    }

    fn is_current_empty(&self) -> bool {
        self.scopes.last().unwrap().is_empty()
    }

    fn insert(&mut self, key: Cow<'a, str>, val: LocalMeta) {
        self.scopes.last_mut().unwrap().insert(key, val);

        // Note that if `insert` returns `Some` then it implies
        // an identifier is reused. For e.g. `{% macro f(a, a) %}`
        // and `{% let (a, a) = ... %}` then this results in a
        // generated template, which when compiled fails with the
        // compile error "identifier `a` used more than once".
    }

    fn insert_with_default(&mut self, key: Cow<'a, str>) {
        self.insert(key, LocalMeta::default());
    }

    fn resolve(&self, name: &str) -> Option<String> {
        let name = normalize_identifier(name);
        self.get(&Cow::Borrowed(name)).map(|meta| match &meta.refs {
            Some(expr) => expr.clone(),
            None => name.to_string(),
        })
    }

    fn resolve_or_self(&self, name: &str) -> String {
        let name = normalize_identifier(name);
        self.resolve(name).unwrap_or_else(|| format!("self.{name}"))
    }
}

impl Default for MapChain<'_> {
    fn default() -> Self {
        Self {
            scopes: vec![HashMap::default()],
        }
    }
}

/// Returns `true` if enough assumptions can be made,
/// to determine that `self` is copyable.
fn is_copyable(expr: &Expr<'_>) -> bool {
    is_copyable_within_op(expr, false)
}

fn is_copyable_within_op(expr: &Expr<'_>, within_op: bool) -> bool {
    match expr {
        Expr::BoolLit(_)
        | Expr::NumLit(_, _)
        | Expr::StrLit(_)
        | Expr::CharLit(_)
        | Expr::BinOp(_, _, _) => true,
        Expr::Unary(.., expr) => is_copyable_within_op(expr, true),
        Expr::Range(..) => true,
        // The result of a call likely doesn't need to be borrowed,
        // as in that case the call is more likely to return a
        // reference in the first place then.
        Expr::Call { .. } | Expr::Path(..) | Expr::Filter(..) | Expr::RustMacro(..) => true,
        // If the `expr` is within a `Unary` or `BinOp` then
        // an assumption can be made that the operand is copy.
        // If not, then the value is moved and adding `.clone()`
        // will solve that issue. However, if the operand is
        // implicitly borrowed, then it's likely not even possible
        // to get the template to compile.
        _ => within_op && is_attr_self(expr),
    }
}

/// Returns `true` if this is an `Attr` where the `obj` is `"self"`.
fn is_attr_self(mut expr: &Expr<'_>) -> bool {
    loop {
        match expr {
            Expr::Attr(obj, _) if matches!(***obj, Expr::Var("self")) => return true,
            Expr::Attr(obj, _) if matches!(***obj, Expr::Attr(..)) => expr = obj,
            _ => return false,
        }
    }
}

const FILTER_SOURCE: &str = "__rinja_filter_block";

#[derive(Clone, Copy, Debug)]
enum DisplayWrap {
    Wrapped,
    Unwrapped,
}

#[derive(Default, Debug)]
struct WritableBuffer<'a> {
    buf: Vec<Writable<'a>>,
    discard: bool,
}

impl<'a> WritableBuffer<'a> {
    fn push(&mut self, writable: Writable<'a>) {
        if !self.discard {
            self.buf.push(writable);
        }
    }
}

impl<'a> Deref for WritableBuffer<'a> {
    type Target = [Writable<'a>];

    fn deref(&self) -> &Self::Target {
        &self.buf[..]
    }
}

#[derive(Debug)]
enum Writable<'a> {
    Lit(Cow<'a, str>),
    Expr(&'a WithSpan<'a, Expr<'a>>),
}

/// Identifiers to be replaced with raw identifiers, so as to avoid
/// collisions between template syntax and Rust's syntax. In particular
/// [Rust keywords](https://doc.rust-lang.org/reference/keywords.html)
/// should be replaced, since they're not reserved words in Rinja
/// syntax but have a high probability of causing problems in the
/// generated code.
///
/// This list excludes the Rust keywords *self*, *Self*, and *super*
/// because they are not allowed to be raw identifiers, and *loop*
/// because it's used something like a keyword in the template
/// language.
fn normalize_identifier(ident: &str) -> &str {
    // This table works for as long as the replacement string is the original string
    // prepended with "r#". The strings get right-padded to the same length with b'_'.
    // While the code does not need it, please keep the list sorted when adding new
    // keywords.

    if ident.len() > MAX_RUST_KEYWORD_LEN {
        return ident;
    }
    let kws = RUST_KEYWORDS[ident.len()];

    let mut padded_ident = [b'_'; MAX_RUST_KEYWORD_LEN];
    padded_ident[..ident.len()].copy_from_slice(ident.as_bytes());

    // Since the individual buckets are quite short, a linear search is faster than a binary search.
    let replacement = match kws
        .iter()
        .find(|probe| padded_ident == <[u8; MAX_RUST_KEYWORD_LEN]>::try_from(&probe[2..]).unwrap())
    {
        Some(replacement) => replacement,
        None => return ident,
    };

    // SAFETY: We know that the input byte slice is pure-ASCII.
    unsafe { std::str::from_utf8_unchecked(&replacement[..ident.len() + 2]) }
}
