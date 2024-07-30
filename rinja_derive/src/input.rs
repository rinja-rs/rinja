use std::borrow::Cow;
use std::collections::hash_map::{Entry, HashMap};
use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use mime::Mime;
use once_map::OnceMap;
use parser::{Node, Parsed};
use proc_macro2::Span;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;

use crate::config::{Config, SyntaxAndCache};
use crate::{CompileError, FileInfo, MsgValidEscapers};

pub(crate) struct TemplateInput<'a> {
    pub(crate) ast: &'a syn::DeriveInput,
    pub(crate) config: &'a Config,
    pub(crate) syntax: &'a SyntaxAndCache<'a>,
    pub(crate) source: &'a Source,
    pub(crate) source_span: Option<Span>,
    pub(crate) block: Option<&'a str>,
    pub(crate) print: Print,
    pub(crate) escaper: &'a str,
    pub(crate) ext: Option<&'a str>,
    pub(crate) mime_type: String,
    pub(crate) path: Arc<Path>,
    pub(crate) fields: Vec<String>,
}

impl TemplateInput<'_> {
    /// Extract the template metadata from the `DeriveInput` structure. This
    /// mostly recovers the data for the `TemplateInput` fields from the
    /// `template()` attribute list fields.
    pub(crate) fn new<'n>(
        ast: &'n syn::DeriveInput,
        config: &'n Config,
        args: &'n TemplateArgs,
    ) -> Result<TemplateInput<'n>, CompileError> {
        let TemplateArgs {
            source,
            block,
            print,
            escaping,
            ext,
            ext_span,
            syntax,
            ..
        } = args;

        // Validate the `source` and `ext` value together, since they are
        // related. In case `source` was used instead of `path`, the value
        // of `ext` is merged into a synthetic `path` value here.
        let &(ref source, source_span) = source
            .as_ref()
            .expect("template path or source not found in attributes");
        let path = match (&source, &ext) {
            (Source::Path(path), _) => config.find_template(path, None, None)?,
            (&Source::Source(_), Some(ext)) => {
                PathBuf::from(format!("{}.{}", ast.ident, ext)).into()
            }
            (&Source::Source(_), None) => {
                return Err(CompileError::no_file_info(
                    "must include 'ext' attribute when using 'source' attribute",
                    None,
                ));
            }
        };

        // Validate syntax
        let syntax = syntax.as_deref().map_or_else(
            || Ok(config.syntaxes.get(config.default_syntax).unwrap()),
            |s| {
                config.syntaxes.get(s).ok_or_else(|| {
                    CompileError::no_file_info(format!("syntax `{s}` is undefined"), None)
                })
            },
        )?;

        // Match extension against defined output formats

        let escaping = escaping
            .as_deref()
            .unwrap_or_else(|| path.extension().map(|s| s.to_str().unwrap()).unwrap_or(""));

        let escaper = config
            .escapers
            .iter()
            .find_map(|(extensions, path)| {
                extensions
                    .contains(&Cow::Borrowed(escaping))
                    .then_some(path.as_ref())
            })
            .ok_or_else(|| {
                CompileError::no_file_info(
                    format!(
                        "no escaper defined for extension '{escaping}'. {}",
                        MsgValidEscapers(&config.escapers),
                    ),
                    *ext_span,
                )
            })?;

        let mime_type =
            extension_to_mime_type(ext_default_to_path(ext.as_deref(), &path).unwrap_or("txt"))
                .to_string();

        let empty_punctuated = syn::punctuated::Punctuated::new();
        let fields = match ast.data {
            syn::Data::Struct(ref struct_) => {
                if let syn::Fields::Named(ref fields) = &struct_.fields {
                    &fields.named
                } else {
                    &empty_punctuated
                }
            }
            syn::Data::Union(ref union_) => &union_.fields.named,
            syn::Data::Enum(_) => &empty_punctuated,
        }
        .iter()
        .map(|f| match &f.ident {
            Some(ident) => ident.to_string(),
            None => unreachable!("we checked that we are using a struct"),
        })
        .collect::<Vec<_>>();

        Ok(TemplateInput {
            ast,
            config,
            syntax,
            source,
            source_span,
            block: block.as_deref(),
            print: *print,
            escaper,
            ext: ext.as_deref(),
            mime_type,
            path,
            fields,
        })
    }

    pub(crate) fn find_used_templates(
        &self,
        map: &mut HashMap<Arc<Path>, Arc<Parsed>>,
    ) -> Result<(), CompileError> {
        let (source, source_path) = match &self.source {
            Source::Source(s) => (s.clone(), None),
            Source::Path(_) => (
                get_template_source(&self.path, None)?,
                Some(Arc::clone(&self.path)),
            ),
        };

        let mut dependency_graph = Vec::new();
        let mut check = vec![(Arc::clone(&self.path), source, source_path)];
        while let Some((path, source, source_path)) = check.pop() {
            let parsed = match self.syntax.parse(Arc::clone(&source), source_path) {
                Ok(parsed) => parsed,
                Err(err) => {
                    let msg = err
                        .message
                        .unwrap_or_else(|| "failed to parse template source".into());
                    let file_path = err
                        .file_path
                        .as_deref()
                        .unwrap_or(Path::new("<source attribute>"));
                    let file_info =
                        FileInfo::new(file_path, Some(&source), Some(&source[err.offset..]));
                    return Err(CompileError::new(msg, Some(file_info)));
                }
            };

            let mut top = true;
            let mut nested = vec![parsed.nodes()];
            while let Some(nodes) = nested.pop() {
                for n in nodes {
                    let mut add_to_check = |new_path: Arc<Path>| -> Result<(), CompileError> {
                        if let Entry::Vacant(e) = map.entry(new_path) {
                            // Add a dummy entry to `map` in order to prevent adding `path`
                            // multiple times to `check`.
                            let new_path = e.key();
                            let source = get_template_source(
                                new_path,
                                Some((&path, parsed.source(), n.span())),
                            )?;
                            check.push((new_path.clone(), source, Some(new_path.clone())));
                            e.insert(Arc::default());
                        }
                        Ok(())
                    };

                    match n {
                        Node::Extends(extends) if top => {
                            let extends = self.config.find_template(
                                extends.path,
                                Some(&path),
                                Some(FileInfo::of(extends, &path, &parsed)),
                            )?;
                            let dependency_path = (path.clone(), extends.clone());
                            if path == extends {
                                // We add the path into the graph to have a better looking error.
                                dependency_graph.push(dependency_path);
                                return cyclic_graph_error(&dependency_graph);
                            } else if dependency_graph.contains(&dependency_path) {
                                return cyclic_graph_error(&dependency_graph);
                            }
                            dependency_graph.push(dependency_path);
                            add_to_check(extends)?;
                        }
                        Node::Macro(m) if top => {
                            nested.push(&m.nodes);
                        }
                        Node::Import(import) if top => {
                            let import = self.config.find_template(
                                import.path,
                                Some(&path),
                                Some(FileInfo::of(import, &path, &parsed)),
                            )?;
                            add_to_check(import)?;
                        }
                        Node::FilterBlock(f) => {
                            nested.push(&f.nodes);
                        }
                        Node::Include(include) => {
                            let include = self.config.find_template(
                                include.path,
                                Some(&path),
                                Some(FileInfo::of(include, &path, &parsed)),
                            )?;
                            add_to_check(include)?;
                        }
                        Node::BlockDef(b) => {
                            nested.push(&b.nodes);
                        }
                        Node::If(i) => {
                            for cond in &i.branches {
                                nested.push(&cond.nodes);
                            }
                        }
                        Node::Loop(l) => {
                            nested.push(&l.body);
                            nested.push(&l.else_nodes);
                        }
                        Node::Match(m) => {
                            for arm in &m.arms {
                                nested.push(&arm.nodes);
                            }
                        }
                        Node::Lit(_)
                        | Node::Comment(_)
                        | Node::Expr(_, _)
                        | Node::Call(_)
                        | Node::Extends(_)
                        | Node::Let(_)
                        | Node::Import(_)
                        | Node::Macro(_)
                        | Node::Raw(_)
                        | Node::Continue(_)
                        | Node::Break(_) => {}
                    }
                }
                top = false;
            }
            map.insert(path, parsed);
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn extension(&self) -> Option<&str> {
        ext_default_to_path(self.ext, &self.path)
    }
}

#[derive(Debug, Default)]
pub(crate) struct TemplateArgs {
    pub(crate) source: Option<(Source, Option<Span>)>,
    block: Option<String>,
    print: Print,
    escaping: Option<String>,
    ext: Option<String>,
    ext_span: Option<Span>,
    syntax: Option<String>,
    config: Option<String>,
    pub(crate) whitespace: Option<String>,
    pub(crate) template_span: Option<Span>,
    pub(crate) config_span: Option<Span>,
}

impl TemplateArgs {
    pub(crate) fn new(ast: &'_ syn::DeriveInput) -> Result<Self, CompileError> {
        // Check that an attribute called `template()` exists once and that it is
        // the proper type (list).
        let mut span = None;
        let mut template_args = None;
        for attr in &ast.attrs {
            let path = &attr.path();
            if !path.is_ident("template") {
                continue;
            }

            span = Some(path.span());
            match attr.parse_args_with(Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated) {
                Ok(args) if template_args.is_none() => template_args = Some(args),
                Ok(_) => {
                    return Err(CompileError::no_file_info(
                        "duplicated 'template' attribute",
                        span,
                    ));
                }
                Err(e) => {
                    return Err(CompileError::no_file_info(
                        format!("unable to parse template arguments: {e}"),
                        span,
                    ));
                }
            };
        }

        let template_args = template_args
            .ok_or_else(|| CompileError::no_file_info("no attribute 'template' found", None))?;

        let mut args = Self {
            template_span: span,
            ..Self::default()
        };
        // Loop over the meta attributes and find everything that we
        // understand. Return a CompileError if something is not right.
        // `source` contains an enum that can represent `path` or `source`.
        for item in &template_args {
            let pair = match item {
                syn::Meta::NameValue(pair) => pair,
                v => {
                    return Err(CompileError::no_file_info(
                        "unsupported attribute argument",
                        Some(v.span()),
                    ));
                }
            };

            let ident = match pair.path.get_ident() {
                Some(ident) => ident,
                None => unreachable!("not possible in syn::Meta::NameValue(…)"),
            };

            let value = match &pair.value {
                syn::Expr::Lit(lit) => lit,
                syn::Expr::Group(group) => match &*group.expr {
                    syn::Expr::Lit(lit) => lit,
                    v => {
                        return Err(CompileError::no_file_info(
                            format!("unsupported argument value type for `{ident}`"),
                            Some(v.span()),
                        ));
                    }
                },
                v => {
                    return Err(CompileError::no_file_info(
                        format!("unsupported argument value type for `{ident}`"),
                        Some(v.span()),
                    ));
                }
            };

            if ident == "path" {
                source_or_path(ident, value, &mut args.source, Source::Path)?;
                args.ext_span = Some(value.span());
            } else if ident == "source" {
                source_or_path(ident, value, &mut args.source, |s| Source::Source(s.into()))?;
            } else if ident == "block" {
                set_template_str_attr(ident, value, &mut args.block)?;
            } else if ident == "print" {
                if let syn::Lit::Str(s) = &value.lit {
                    args.print = match s.value().as_str() {
                        "all" => Print::All,
                        "ast" => Print::Ast,
                        "code" => Print::Code,
                        "none" => Print::None,
                        v => {
                            return Err(CompileError::no_file_info(
                                format!("invalid value for `print` option: {v}"),
                                Some(s.span()),
                            ));
                        }
                    };
                } else {
                    return Err(CompileError::no_file_info(
                        "`print` value must be string literal",
                        Some(value.lit.span()),
                    ));
                }
            } else if ident == "escape" {
                set_template_str_attr(ident, value, &mut args.escaping)?;
            } else if ident == "ext" {
                set_template_str_attr(ident, value, &mut args.ext)?;
                args.ext_span = Some(value.span());
            } else if ident == "syntax" {
                set_template_str_attr(ident, value, &mut args.syntax)?;
            } else if ident == "config" {
                set_template_str_attr(ident, value, &mut args.config)?;
                args.config_span = Some(value.span())
            } else if ident == "whitespace" {
                set_template_str_attr(ident, value, &mut args.whitespace)?;
            } else {
                return Err(CompileError::no_file_info(
                    format!("unsupported attribute key `{ident}` found"),
                    Some(ident.span()),
                ));
            }
        }

        Ok(args)
    }

    pub(crate) fn fallback() -> Self {
        Self {
            source: Some((Source::Source("".into()), None)),
            ext: Some("txt".to_string()),
            ..Self::default()
        }
    }

    pub(crate) fn config_path(&self) -> Option<&str> {
        self.config.as_deref()
    }
}

fn source_or_path(
    name: &syn::Ident,
    value: &syn::ExprLit,
    dest: &mut Option<(Source, Option<Span>)>,
    ctor: fn(String) -> Source,
) -> Result<(), CompileError> {
    if dest.is_some() {
        Err(CompileError::no_file_info(
            "must specify `source` OR `path` exactly once",
            Some(name.span()),
        ))
    } else if let syn::Lit::Str(s) = &value.lit {
        *dest = Some((ctor(s.value()), Some(value.span())));
        Ok(())
    } else {
        Err(CompileError::no_file_info(
            format!("`{name}` value must be string literal"),
            Some(value.lit.span()),
        ))
    }
}

fn set_template_str_attr(
    name: &syn::Ident,
    value: &syn::ExprLit,
    dest: &mut Option<String>,
) -> Result<(), CompileError> {
    if dest.is_some() {
        Err(CompileError::no_file_info(
            format!("attribute `{name}` already set"),
            Some(name.span()),
        ))
    } else if let syn::Lit::Str(s) = &value.lit {
        *dest = Some(s.value());
        Ok(())
    } else {
        Err(CompileError::no_file_info(
            format!("`{name}` value must be string literal"),
            Some(value.lit.span()),
        ))
    }
}

#[inline]
fn ext_default_to_path<'a>(ext: Option<&'a str>, path: &'a Path) -> Option<&'a str> {
    ext.or_else(|| extension(path))
}

fn extension(path: &Path) -> Option<&str> {
    let ext = path.extension().map(|s| s.to_str().unwrap())?;

    const JINJA_EXTENSIONS: [&str; 3] = ["j2", "jinja", "jinja2"];
    if JINJA_EXTENSIONS.contains(&ext) {
        Path::new(path.file_stem().unwrap())
            .extension()
            .map(|s| s.to_str().unwrap())
            .or(Some(ext))
    } else {
        Some(ext)
    }
}

#[derive(Debug, Hash, PartialEq)]
pub(crate) enum Source {
    Path(String),
    Source(Arc<str>),
}

#[derive(Clone, Copy, Debug, PartialEq, Hash)]
pub(crate) enum Print {
    All,
    Ast,
    Code,
    None,
}

impl Default for Print {
    fn default() -> Self {
        Self::None
    }
}

pub(crate) fn extension_to_mime_type(ext: &str) -> Mime {
    let basic_type = mime_guess::from_ext(ext).first_or_octet_stream();
    for (simple, utf_8) in &TEXT_TYPES {
        if &basic_type == simple {
            return utf_8.clone();
        }
    }
    basic_type
}

const TEXT_TYPES: [(Mime, Mime); 7] = [
    (mime::TEXT_PLAIN, mime::TEXT_PLAIN_UTF_8),
    (mime::TEXT_HTML, mime::TEXT_HTML_UTF_8),
    (mime::TEXT_CSS, mime::TEXT_CSS_UTF_8),
    (mime::TEXT_CSV, mime::TEXT_CSV_UTF_8),
    (
        mime::TEXT_TAB_SEPARATED_VALUES,
        mime::TEXT_TAB_SEPARATED_VALUES_UTF_8,
    ),
    (
        mime::APPLICATION_JAVASCRIPT,
        mime::APPLICATION_JAVASCRIPT_UTF_8,
    ),
    (mime::IMAGE_SVG, mime::IMAGE_SVG),
];

fn cyclic_graph_error(dependency_graph: &[(Arc<Path>, Arc<Path>)]) -> Result<(), CompileError> {
    Err(CompileError::no_file_info(
        format!(
            "cyclic dependency in graph {:#?}",
            dependency_graph
                .iter()
                .map(|e| format!("{:#?} --> {:#?}", e.0, e.1))
                .collect::<Vec<String>>()
        ),
        None,
    ))
}

pub(crate) fn get_template_source(
    tpl_path: &Arc<Path>,
    import_from: Option<(&Arc<Path>, &str, &str)>,
) -> Result<Arc<str>, CompileError> {
    static CACHE: OnceLock<OnceMap<Arc<Path>, Arc<str>>> = OnceLock::new();

    CACHE.get_or_init(OnceMap::new).get_or_try_insert_ref(
        tpl_path,
        (),
        Arc::clone,
        |_, tpl_path| match read_to_string(tpl_path) {
            Ok(mut source) => {
                if source.ends_with('\n') {
                    let _ = source.pop();
                }
                let source = Arc::from(source);
                Ok((Arc::clone(&source), source))
            }
            Err(err) => Err(CompileError::new(
                format_args!(
                    "unable to open template file '{}': {err}",
                    tpl_path.to_str().unwrap(),
                ),
                import_from.map(|(node_file, file_source, node_source)| {
                    FileInfo::new(node_file, Some(file_source), Some(node_source))
                }),
            )),
        },
        |_, _, cached| Arc::clone(cached),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ext() {
        assert_eq!(extension(Path::new("foo-bar.txt")), Some("txt"));
        assert_eq!(extension(Path::new("foo-bar.html")), Some("html"));
        assert_eq!(extension(Path::new("foo-bar.unknown")), Some("unknown"));
        assert_eq!(extension(Path::new("foo-bar.svg")), Some("svg"));

        assert_eq!(extension(Path::new("foo/bar/baz.txt")), Some("txt"));
        assert_eq!(extension(Path::new("foo/bar/baz.html")), Some("html"));
        assert_eq!(extension(Path::new("foo/bar/baz.unknown")), Some("unknown"));
        assert_eq!(extension(Path::new("foo/bar/baz.svg")), Some("svg"));
    }

    #[test]
    fn test_double_ext() {
        assert_eq!(extension(Path::new("foo-bar.html.txt")), Some("txt"));
        assert_eq!(extension(Path::new("foo-bar.txt.html")), Some("html"));
        assert_eq!(extension(Path::new("foo-bar.txt.unknown")), Some("unknown"));

        assert_eq!(extension(Path::new("foo/bar/baz.html.txt")), Some("txt"));
        assert_eq!(extension(Path::new("foo/bar/baz.txt.html")), Some("html"));
        assert_eq!(
            extension(Path::new("foo/bar/baz.txt.unknown")),
            Some("unknown")
        );
    }

    #[test]
    fn test_skip_jinja_ext() {
        assert_eq!(extension(Path::new("foo-bar.html.j2")), Some("html"));
        assert_eq!(extension(Path::new("foo-bar.html.jinja")), Some("html"));
        assert_eq!(extension(Path::new("foo-bar.html.jinja2")), Some("html"));

        assert_eq!(extension(Path::new("foo/bar/baz.txt.j2")), Some("txt"));
        assert_eq!(extension(Path::new("foo/bar/baz.txt.jinja")), Some("txt"));
        assert_eq!(extension(Path::new("foo/bar/baz.txt.jinja2")), Some("txt"));
    }

    #[test]
    fn test_only_jinja_ext() {
        assert_eq!(extension(Path::new("foo-bar.j2")), Some("j2"));
        assert_eq!(extension(Path::new("foo-bar.jinja")), Some("jinja"));
        assert_eq!(extension(Path::new("foo-bar.jinja2")), Some("jinja2"));
    }

    #[test]
    fn get_source() {
        let path = Config::new("", None, None, None)
            .and_then(|config| config.find_template("b.html", None, None))
            .unwrap();
        assert_eq!(get_template_source(&path, None).unwrap(), "bar".into());
    }
}
