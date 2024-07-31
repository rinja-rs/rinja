#![deny(elided_lifetimes_in_paths)]
#![deny(unreachable_pub)]

mod config;
mod generator;
mod heritage;
mod html;
mod input;
#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use annotate_snippets::{Level, Renderer, Snippet};
use config::{read_config_file, Config};
use generator::{Generator, MapChain};
use heritage::{Context, Heritage};
use input::{Print, TemplateArgs, TemplateInput};
use parser::{strip_common, Parsed, WithSpan};
#[cfg(not(feature = "__standalone"))]
use proc_macro::TokenStream as TokenStream12;
#[cfg(feature = "__standalone")]
use proc_macro2::TokenStream as TokenStream12;
use proc_macro2::{Span, TokenStream};
use syn::parse_quote_spanned;

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
pub fn derive_template(input: TokenStream12) -> TokenStream12 {
    let ast = syn::parse2(input.into()).unwrap();
    match build_template(&ast) {
        Ok(source) => source.parse().unwrap(),
        Err(CompileError {
            msg,
            span,
            rendered,
        }) => {
            let msg = if rendered {
                eprintln!("{msg}");
                "the previous template error derives from"
            } else {
                &msg
            };
            let mut ts: TokenStream = parse_quote_spanned! {
                span.unwrap_or(ast.ident.span()) =>
                ::core::compile_error!(#msg);
            };
            if let Ok(source) = build_skeleton(&ast) {
                let source: TokenStream = source.parse().unwrap();
                ts.extend(source);
            }
            ts.into()
        }
    }
}

fn build_skeleton(ast: &syn::DeriveInput) -> Result<String, CompileError> {
    let template_args = TemplateArgs::fallback();
    let config = Config::new("", None, None, None)?;
    let input = TemplateInput::new(ast, config, &template_args)?;
    let mut contexts = HashMap::new();
    let parsed = parser::Parsed::default();
    contexts.insert(&input.path, Context::empty(&parsed));
    Generator::new(
        &input,
        &contexts,
        None,
        MapChain::default(),
        input.block.is_some(),
        0,
    )
    .build(&contexts[&input.path])
}

/// Takes a `syn::DeriveInput` and generates source code for it
///
/// Reads the metadata from the `template()` attribute to get the template
/// metadata, then fetches the source from the filesystem. The source is
/// parsed, and the parse tree is fed to the code generator. Will print
/// the parse tree and/or generated source according to the `print` key's
/// value as passed to the `template()` attribute.
pub(crate) fn build_template(ast: &syn::DeriveInput) -> Result<String, CompileError> {
    let template_args = TemplateArgs::new(ast)?;
    let mut result = build_template_inner(ast, &template_args);
    if let Err(err) = &mut result {
        if err.span.is_none() {
            err.span = template_args
                .source
                .as_ref()
                .and_then(|(_, span)| *span)
                .or(template_args.template_span);
        }
    }
    result
}

fn build_template_inner(
    ast: &syn::DeriveInput,
    template_args: &TemplateArgs,
) -> Result<String, CompileError> {
    let config_path = template_args.config_path();
    let s = read_config_file(config_path, template_args.config_span)?;
    let config = Config::new(
        &s,
        config_path,
        template_args.whitespace.as_deref(),
        template_args.config_span,
    )?;
    let input = TemplateInput::new(ast, config, template_args)?;

    let mut templates = HashMap::new();
    input.find_used_templates(&mut templates)?;

    let mut contexts = HashMap::new();
    for (path, parsed) in &templates {
        contexts.insert(path, Context::new(input.config, path, parsed)?);
    }

    let ctx = &contexts[&input.path];
    let heritage = if !ctx.blocks.is_empty() || ctx.extends.is_some() {
        let heritage = Heritage::new(ctx, &contexts);

        if let Some(block_name) = input.block {
            if !heritage.blocks.contains_key(&block_name) {
                return Err(CompileError::no_file_info(
                    format!("cannot find block {}", block_name),
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

    let code = Generator::new(
        &input,
        &contexts,
        heritage.as_ref(),
        MapChain::default(),
        input.block.is_some(),
        0,
    )
    .build(&contexts[&input.path])?;
    if input.print == Print::Code || input.print == Print::All {
        eprintln!("{code}");
    }
    Ok(code)
}

#[derive(Debug, Clone)]
struct CompileError {
    msg: String,
    span: Option<Span>,
    rendered: bool,
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
        if let Some(FileInfo {
            path,
            source: Some(source),
            node_source: Some(node_source),
        }) = file_info
        {
            if source
                .as_bytes()
                .as_ptr_range()
                .contains(&node_source.as_ptr())
            {
                let label = msg.to_string();
                let path = match std::env::current_dir() {
                    Ok(cwd) => strip_common(&cwd, path),
                    Err(_) => path.display().to_string(),
                };

                let start = node_source.as_ptr() as usize - source.as_ptr() as usize;
                let annotation = Level::Error.span(start..start).label("close to this token");
                let snippet = Snippet::source(source)
                    .origin(&path)
                    .fold(true)
                    .annotation(annotation);
                let message = Level::Error.title(&label).snippet(snippet);
                return Self {
                    msg: Renderer::styled().render(message).to_string(),
                    span,
                    rendered: true,
                };
            }
        }

        let msg = match file_info {
            Some(file_info) => format!("{msg}{file_info}"),
            None => msg.to_string(),
        };
        Self {
            msg,
            span,
            rendered: false,
        }
    }

    fn no_file_info<S: fmt::Display>(msg: S, span: Option<Span>) -> Self {
        Self {
            msg: msg.to_string(),
            span,
            rendered: false,
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

    fn of<T>(node: &WithSpan<'a, T>, path: &'a Path, parsed: &'a Parsed) -> Self {
        Self {
            path,
            source: Some(parsed.source()),
            node_source: Some(node.span()),
        }
    }
}

impl fmt::Display for FileInfo<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.source, self.node_source) {
            (Some(source), Some(node_source)) => {
                let (error_info, file_path) = generate_error_info(source, node_source, self.path);
                write!(
                    f,
                    "\n  --> {file_path}:{row}:{column}\n{source_after}",
                    row = error_info.row,
                    column = error_info.column,
                    source_after = &error_info.source_after,
                )
            }
            _ => {
                let file_path = match std::env::current_dir() {
                    Ok(cwd) => strip_common(&cwd, self.path),
                    Err(_) => self.path.display().to_string(),
                };
                write!(f, "\n --> {file_path}")
            }
        }
    }
}

struct ErrorInfo {
    row: usize,
    column: usize,
    source_after: String,
}

fn generate_row_and_column(src: &str, input: &str) -> ErrorInfo {
    let offset = src.len() - input.len();
    let (source_before, source_after) = src.split_at(offset);

    let source_after = match source_after.char_indices().enumerate().take(41).last() {
        Some((80, (i, _))) => format!("{:?}...", &source_after[..i]),
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
    let error_info: ErrorInfo = generate_row_and_column(src, input);
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

// This is used by the code generator to decide whether a named filter is part of
// Rinja or should refer to a local `filters` module. It should contain all the
// filters shipped with Rinja, even the optional ones (since optional inclusion
// in the const vector based on features seems impossible right now).
const BUILT_IN_FILTERS: &[&str] = &[
    "abs",
    "capitalize",
    "center",
    "e",
    "escape",
    "filesizeformat",
    "fmt",
    "format",
    "indent",
    "into_f64",
    "into_isize",
    "join",
    "linebreaks",
    "linebreaksbr",
    "lower",
    "lowercase",
    "paragraphbreaks",
    "safe",
    "title",
    "trim",
    "truncate",
    "upper",
    "uppercase",
    "urlencode_strict",
    "urlencode",
    "wordcount",
    // optional features, reserve the names anyway:
    "json",
];

const CRATE: &str = if cfg!(feature = "with-actix-web") {
    "::rinja_actix"
} else if cfg!(feature = "with-axum") {
    "::rinja_axum"
} else if cfg!(feature = "with-rocket") {
    "::rinja_rocket"
} else if cfg!(feature = "with-warp") {
    "::rinja_warp"
} else {
    "::rinja"
};
