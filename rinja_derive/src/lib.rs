#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![deny(elided_lifetimes_in_paths)]
#![deny(unreachable_pub)]

mod config;
mod generator;
mod heritage;
mod html;
mod input;
mod integration;
#[cfg(test)]
mod tests;

use std::borrow::{Borrow, Cow};
use std::collections::hash_map::{Entry, HashMap};
use std::fmt;
use std::hash::{BuildHasher, Hash};
use std::path::Path;
use std::sync::Mutex;

use config::{Config, read_config_file};
use generator::{TmplKind, template_to_string};
use heritage::{Context, Heritage};
use input::{AnyTemplateArgs, Print, TemplateArgs, TemplateInput};
use integration::{Buffer, build_template_enum};
use parser::{Parsed, strip_common};
#[cfg(not(feature = "__standalone"))]
use proc_macro::TokenStream as TokenStream12;
#[cfg(feature = "__standalone")]
use proc_macro2::TokenStream as TokenStream12;
use proc_macro2::{Span, TokenStream};
use quote::quote_spanned;
use rustc_hash::FxBuildHasher;

/// The `Template` derive macro and its `template()` attribute.
///
/// Rinja works by generating one or more trait implementations for any
/// `struct` type decorated with the `#[derive(Template)]` attribute. The
/// code generation process takes some options that can be specified through
/// the `template()` attribute.
///
/// ## Attributes
///
/// The following sub-attributes are currently recognized:
///
/// ### path
///
/// E.g. `path = "foo.html"`
///
/// Sets the path to the template file.
/// The path is interpreted as relative to the configured template directories
/// (by default, this is a `templates` directory next to your `Cargo.toml`).
/// The file name extension is used to infer an escape mode (see below). In
/// web framework integrations, the path's extension may also be used to
/// infer the content type of the resulting response.
/// Cannot be used together with `source`.
///
/// ### source
///
/// E.g. `source = "{{ foo }}"`
///
/// Directly sets the template source.
/// This can be useful for test cases or short templates. The generated path
/// is undefined, which generally makes it impossible to refer to this
/// template from other templates. If `source` is specified, `ext` must also
/// be specified (see below). Cannot be used together with `path`.
/// `ext` (e.g. `ext = "txt"`): lets you specify the content type as a file
/// extension. This is used to infer an escape mode (see below), and some
/// web framework integrations use it to determine the content type.
/// Cannot be used together with `path`.
///
/// ### `in_doc`
///
/// E.g. `in_doc = true`
///
/// As an alternative to supplying the code template code in an external file (as `path` argument),
/// or as a string (as `source` argument), you can also enable the `"code-in-doc"` feature.
/// With this feature, you can specify the template code directly in the documentation
/// of the template `struct`.
///
/// Instead of `path = "…"` or `source = "…"`, specify `in_doc = true` in the `#[template]`
/// attribute, and in the struct's documentation add a `rinja` code block:
///
/// ```rust,ignore
/// /// ```rinja
/// /// <div>{{ lines|linebreaksbr }}</div>
/// /// ```
/// #[derive(Template)]
/// #[template(ext = "html", in_doc = true)]
/// struct Example<'a> {
///     lines: &'a str,
/// }
/// ```
///
/// ### print
///
/// E.g. `print = "code"`
///
/// Enable debugging by printing nothing (`none`), the parsed syntax tree (`ast`),
/// the generated code (`code`) or `all` for both.
/// The requested data will be printed to stdout at compile time.
///
/// ### escape
///
/// E.g. `escape = "none"`
///
/// Override the template's extension used for the purpose of determining the escaper for
/// this template. See the section on configuring custom escapers for more information.
///
/// ### syntax
///
/// E.g. `syntax = "foo"`
///
/// Set the syntax name for a parser defined in the configuration file.
/// The default syntax, `"default"`,  is the one provided by Rinja.
#[allow(clippy::useless_conversion)] // To be compatible with both `TokenStream`s
#[cfg_attr(
    not(feature = "__standalone"),
    proc_macro_derive(Template, attributes(template))
)]
#[must_use]
pub fn derive_template(input: TokenStream12) -> TokenStream12 {
    let ast = match syn::parse2(input.into()) {
        Ok(ast) => ast,
        Err(err) => {
            let msgs = err.into_iter().map(|err| err.to_string());
            return compile_error(msgs, Span::call_site()).into();
        }
    };

    let mut buf = Buffer::new();
    if let Err(CompileError { msg, span }) = build_template(&mut buf, &ast) {
        let mut ts = compile_error(std::iter::once(msg), span.unwrap_or(ast.ident.span()));
        buf.clear();
        if build_skeleton(&mut buf, &ast).is_ok() {
            let source: TokenStream = buf.into_string().parse().unwrap();
            ts.extend(source);
        }
        ts.into()
    } else {
        buf.into_string().parse().unwrap()
    }
}

fn compile_error(msgs: impl Iterator<Item = String>, span: Span) -> TokenStream {
    quote_spanned! {
        span =>
        const _: () = {
            extern crate rinja as rinja;
            #(rinja::helpers::core::compile_error!(#msgs);)*
        };
    }
}

fn build_skeleton(buf: &mut Buffer, ast: &syn::DeriveInput) -> Result<usize, CompileError> {
    let template_args = TemplateArgs::fallback();
    let config = Config::new("", None, None, None)?;
    let input = TemplateInput::new(ast, config, &template_args)?;
    let mut contexts = HashMap::default();
    let parsed = parser::Parsed::default();
    contexts.insert(&input.path, Context::empty(&parsed));
    template_to_string(buf, &input, &contexts, None, TmplKind::Struct)
}

/// Takes a `syn::DeriveInput` and generates source code for it
///
/// Reads the metadata from the `template()` attribute to get the template
/// metadata, then fetches the source from the filesystem. The source is
/// parsed, and the parse tree is fed to the code generator. Will print
/// the parse tree and/or generated source according to the `print` key's
/// value as passed to the `template()` attribute.
pub(crate) fn build_template(
    buf: &mut Buffer,
    ast: &syn::DeriveInput,
) -> Result<usize, CompileError> {
    let err_span;
    let mut result = match AnyTemplateArgs::new(ast)? {
        AnyTemplateArgs::Struct(item) => {
            err_span = item.source.1.or(item.template_span);
            build_template_item(buf, ast, &item, TmplKind::Struct)
        }
        AnyTemplateArgs::Enum {
            enum_args,
            vars_args,
            has_default_impl,
        } => {
            err_span = enum_args
                .as_ref()
                .and_then(|v| v.source.as_ref())
                .map(|s| s.span())
                .or_else(|| enum_args.as_ref().map(|v| v.template.span()));
            build_template_enum(buf, ast, enum_args, vars_args, has_default_impl)
        }
    };
    if let Err(err) = &mut result {
        if err.span.is_none() {
            err.span = err_span;
        }
    }
    result
}

fn build_template_item(
    buf: &mut Buffer,
    ast: &syn::DeriveInput,
    template_args: &TemplateArgs,
    tmpl_kind: TmplKind,
) -> Result<usize, CompileError> {
    let config_path = template_args.config_path();
    let s = read_config_file(config_path, template_args.config_span)?;
    let config = Config::new(
        &s,
        config_path,
        template_args.whitespace,
        template_args.config_span,
    )?;
    let input = TemplateInput::new(ast, config, template_args)?;

    let mut templates = HashMap::default();
    input.find_used_templates(&mut templates)?;

    let mut contexts = HashMap::default();
    for (path, parsed) in &templates {
        contexts.insert(path, Context::new(input.config, path, parsed)?);
    }

    let ctx = &contexts[&input.path];
    let heritage = if !ctx.blocks.is_empty() || ctx.extends.is_some() {
        let heritage = Heritage::new(ctx, &contexts);

        if let Some(block_name) = input.block {
            if !heritage.blocks.contains_key(&block_name) {
                return Err(CompileError::no_file_info(
                    format!("cannot find block {block_name}"),
                    None,
                ));
            }
        }

        Some(heritage)
    } else {
        None
    };

    if input.print == Print::Ast || input.print == Print::All {
        eprintln!("{:?}", templates[&input.path].nodes());
    }

    let mark = buf.get_mark();
    let size_hint = template_to_string(buf, &input, &contexts, heritage.as_ref(), tmpl_kind)?;
    if input.print == Print::Code || input.print == Print::All {
        eprintln!("{}", buf.marked_text(mark));
    }
    Ok(size_hint)
}

#[derive(Debug, Clone)]
struct CompileError {
    msg: String,
    span: Option<Span>,
}

impl CompileError {
    fn new<S: fmt::Display>(msg: S, file_info: Option<FileInfo<'_>>) -> Self {
        Self::new_with_span(msg, file_info, None)
    }

    fn new_with_span<S: fmt::Display>(
        msg: S,
        file_info: Option<FileInfo<'_>>,
        span: Option<Span>,
    ) -> Self {
        let msg = match file_info {
            Some(file_info) => format!("{msg}{file_info}"),
            None => msg.to_string(),
        };
        Self { msg, span }
    }

    fn no_file_info<S: ToString>(msg: S, span: Option<Span>) -> Self {
        Self {
            msg: msg.to_string(),
            span,
        }
    }
}

impl std::error::Error for CompileError {}

impl fmt::Display for CompileError {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str(&self.msg)
    }
}

#[derive(Debug, Clone, Copy)]
struct FileInfo<'a> {
    path: &'a Path,
    source: Option<&'a str>,
    node_source: Option<&'a str>,
}

impl<'a> FileInfo<'a> {
    fn new(path: &'a Path, source: Option<&'a str>, node_source: Option<&'a str>) -> Self {
        Self {
            path,
            source,
            node_source,
        }
    }

    fn of(node: parser::Span<'a>, path: &'a Path, parsed: &'a Parsed) -> Self {
        let source = parsed.source();
        Self {
            path,
            source: Some(source),
            node_source: node.as_suffix_of(source),
        }
    }
}

impl fmt::Display for FileInfo<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let (Some(source), Some(node_source)) = (self.source, self.node_source) {
            let (error_info, file_path) = generate_error_info(source, node_source, self.path);
            write!(
                f,
                "\n  --> {file_path}:{row}:{column}\n{source_after}",
                row = error_info.row,
                column = error_info.column,
                source_after = error_info.source_after,
            )
        } else {
            write!(f, "\n --> {}", match std::env::current_dir() {
                Ok(cwd) => fmt_left!(move "{}", strip_common(&cwd, self.path)),
                Err(_) => fmt_right!("{}", self.path.display()),
            })
        }
    }
}

struct ErrorInfo {
    row: usize,
    column: usize,
    source_after: String,
}

fn generate_row_and_column(src: &str, input: &str) -> ErrorInfo {
    const MAX_LINE_LEN: usize = 80;

    let offset = src.len() - input.len();
    let (source_before, source_after) = src.split_at(offset);

    let source_after = match source_after
        .char_indices()
        .enumerate()
        .take(MAX_LINE_LEN + 1)
        .last()
    {
        Some((MAX_LINE_LEN, (i, _))) => format!("{:?}...", &source_after[..i]),
        _ => format!("{source_after:?}"),
    };

    let (row, last_line) = source_before.lines().enumerate().last().unwrap_or_default();
    let column = last_line.chars().count();
    ErrorInfo {
        row: row + 1,
        column,
        source_after,
    }
}

/// Return the error related information and its display file path.
fn generate_error_info(src: &str, input: &str, file_path: &Path) -> (ErrorInfo, String) {
    let file_path = match std::env::current_dir() {
        Ok(cwd) => strip_common(&cwd, file_path),
        Err(_) => file_path.display().to_string(),
    };
    let error_info = generate_row_and_column(src, input);
    (error_info, file_path)
}

struct MsgValidEscapers<'a>(&'a [(Vec<Cow<'a, str>>, Cow<'a, str>)]);

impl fmt::Display for MsgValidEscapers<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut exts = self
            .0
            .iter()
            .flat_map(|(exts, _)| exts)
            .map(|x| format!("{x:?}"))
            .collect::<Vec<_>>();
        exts.sort();
        write!(f, "The available extensions are: {}", exts.join(", "))
    }
}

#[derive(Debug)]
struct OnceMap<K, V>([Mutex<HashMap<K, V, FxBuildHasher>>; 8]);

impl<K, V> Default for OnceMap<K, V> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<K: Hash + Eq, V> OnceMap<K, V> {
    // The API of this function was copied, and adapted from the `once_map` crate
    // <https://crates.io/crates/once_map/0.4.18>.
    fn get_or_try_insert<T, Q, E>(
        &self,
        key: &Q,
        make_key_value: impl FnOnce(&Q) -> Result<(K, V), E>,
        to_value: impl FnOnce(&V) -> T,
    ) -> Result<T, E>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        let shard_idx = (FxBuildHasher.hash_one(key) % self.0.len() as u64) as usize;
        let mut shard = self.0[shard_idx].lock().unwrap();
        Ok(to_value(if let Some(v) = shard.get(key) {
            v
        } else {
            let (k, v) = make_key_value(key)?;
            match shard.entry(k) {
                Entry::Vacant(entry) => entry.insert(v),
                Entry::Occupied(_) => unreachable!("key in map when it should not have been"),
            }
        }))
    }
}

enum EitherFormat<L, R>
where
    L: for<'a, 'b> Fn(&'a mut fmt::Formatter<'b>) -> fmt::Result,
    R: for<'a, 'b> Fn(&'a mut fmt::Formatter<'b>) -> fmt::Result,
{
    Left(L),
    Right(R),
}

impl<L, R> fmt::Display for EitherFormat<L, R>
where
    L: for<'a, 'b> Fn(&'a mut fmt::Formatter<'b>) -> fmt::Result,
    R: for<'a, 'b> Fn(&'a mut fmt::Formatter<'b>) -> fmt::Result,
{
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Left(v) => v(f),
            Self::Right(v) => v(f),
        }
    }
}

macro_rules! fmt_left {
    (move $fmt:literal $($tt:tt)*) => {
        $crate::EitherFormat::Left(move |f: &mut std::fmt::Formatter<'_>| {
            write!(f, $fmt $($tt)*)
        })
    };
    ($fmt:literal $($tt:tt)*) => {
        $crate::EitherFormat::Left(|f: &mut std::fmt::Formatter<'_>| {
            write!(f, $fmt $($tt)*)
        })
    };
}

macro_rules! fmt_right {
    (move $fmt:literal $($tt:tt)*) => {
        $crate::EitherFormat::Right(move |f: &mut std::fmt::Formatter<'_>| {
            write!(f, $fmt $($tt)*)
        })
    };
    ($fmt:literal $($tt:tt)*) => {
        $crate::EitherFormat::Right(|f: &mut std::fmt::Formatter<'_>| {
            write!(f, $fmt $($tt)*)
        })
    };
}

pub(crate) use {fmt_left, fmt_right};
