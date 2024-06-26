#![deny(elided_lifetimes_in_paths)]
#![deny(unreachable_pub)]

mod config;
mod generator;
mod heritage;
mod input;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use config::{read_config_file, Config};
use generator::{Generator, MapChain};
use heritage::{Context, Heritage};
use input::{Print, TemplateArgs, TemplateInput};
use parser::{generate_error_info, strip_common, ErrorInfo, ParseError};
use proc_macro2::{Span, TokenStream};

#[cfg(not(feature = "__standalone"))]
macro_rules! pub_if_standalone {
    (pub $($tt:tt)*) => {
        $($tt)*
    }
}

#[cfg(feature = "__standalone")]
macro_rules! pub_if_standalone {
    ($($tt:tt)*) => {
        $($tt)*
    }
}

#[cfg(not(feature = "__standalone"))]
#[proc_macro_derive(Template, attributes(template))]
pub fn derive_template(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_template2(input.into()).into()
}

pub_if_standalone! {
    pub fn derive_template2(input: TokenStream) -> TokenStream {
        let ast = syn::parse2(input).unwrap();
        match build_template(&ast) {
            Ok(source) => source.parse().unwrap(),
            Err(e) => {
                let mut e = e.into_compile_error();
                if let Ok(source) = build_skeleton(&ast) {
                    let source: TokenStream = source.parse().unwrap();
                    e.extend(source);
                }
                e
            }
        }
    }
}

fn build_skeleton(ast: &syn::DeriveInput) -> Result<String, CompileError> {
    let template_args = TemplateArgs::fallback();
    let config = Config::new("", None, None)?;
    let input = TemplateInput::new(ast, &config, &template_args)?;
    let mut contexts = HashMap::new();
    let parsed = parser::Parsed::default();
    contexts.insert(&input.path, Context::empty(&parsed));
    Generator::new(
        &input,
        &contexts,
        None,
        MapChain::default(),
        input.block.is_some(),
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
    let config_path = template_args.config_path();
    let s = read_config_file(config_path)?;
    let config = Config::new(&s, config_path, template_args.whitespace.as_deref())?;
    let input = TemplateInput::new(ast, &config, &template_args)?;

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
                return Err(CompileError::no_file_info(format!(
                    "cannot find block {}",
                    block_name
                )));
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
    span: Span,
}

impl CompileError {
    fn new<S: fmt::Display>(msg: S, file_info: Option<FileInfo<'_, '_, '_>>) -> Self {
        let msg = match file_info {
            Some(file_info) => format!("{msg}{file_info}"),
            None => msg.to_string(),
        };
        Self {
            msg,
            span: Span::call_site(),
        }
    }

    fn no_file_info<S: fmt::Display>(msg: S) -> Self {
        Self {
            msg: msg.to_string(),
            span: Span::call_site(),
        }
    }

    fn into_compile_error(self) -> TokenStream {
        syn::Error::new(self.span, self.msg).to_compile_error()
    }
}

impl std::error::Error for CompileError {}

impl fmt::Display for CompileError {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str(&self.msg)
    }
}

impl From<ParseError> for CompileError {
    #[inline]
    fn from(e: ParseError) -> Self {
        // It already has the correct message so no need to do anything.
        Self::no_file_info(e)
    }
}

struct FileInfo<'a, 'b, 'c> {
    path: &'a Path,
    source: Option<&'b str>,
    node_source: Option<&'c str>,
}

impl<'a, 'b, 'c> FileInfo<'a, 'b, 'c> {
    fn new(path: &'a Path, source: Option<&'b str>, node_source: Option<&'c str>) -> Self {
        Self {
            path,
            source,
            node_source,
        }
    }
}

impl<'a, 'b, 'c> fmt::Display for FileInfo<'a, 'b, 'c> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.source, self.node_source) {
            (Some(source), Some(node_source)) => {
                let (
                    ErrorInfo {
                        row,
                        column,
                        source_after,
                    },
                    file_path,
                ) = generate_error_info(source, node_source, self.path);
                write!(
                    f,
                    "\n  --> {file_path}:{row}:{column}\n{source_after}",
                    row = row + 1
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
    "paragraphbreaks",
    "lower",
    "lowercase",
    "safe",
    "title",
    "trim",
    "truncate",
    "upper",
    "uppercase",
    "urlencode",
    "urlencode_strict",
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
