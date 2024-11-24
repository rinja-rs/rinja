use std::borrow::Cow;
use std::collections::hash_map::{Entry, HashMap};
use std::fs::read_to_string;
use std::iter::FusedIterator;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, OnceLock};

use mime::Mime;
use parser::node::Whitespace;
use parser::{Node, Parsed};
use proc_macro2::Span;
use rustc_hash::FxBuildHasher;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Attribute, Expr, ExprLit, Ident, Lit, LitBool, LitStr, Meta, Token};

use crate::config::{Config, SyntaxAndCache};
use crate::{CompileError, FileInfo, MsgValidEscapers, OnceMap};

pub(crate) struct TemplateInput<'a> {
    pub(crate) ast: &'a syn::DeriveInput,
    pub(crate) config: &'a Config,
    pub(crate) syntax: &'a SyntaxAndCache<'a>,
    pub(crate) source: &'a Source,
    pub(crate) source_span: Option<Span>,
    pub(crate) block: Option<&'a str>,
    pub(crate) print: Print,
    pub(crate) escaper: &'a str,
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
            source: (source, source_span),
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
        let path = match (&source, &ext) {
            (Source::Path(path), _) => config.find_template(path, None, None)?,
            (&Source::Source(_), Some(ext)) => {
                PathBuf::from(format!("{}.{}", ast.ident, ext)).into()
            }
            (&Source::Source(_), None) => {
                return Err(CompileError::no_file_info(
                    #[cfg(not(feature = "code-in-doc"))]
                    "must include `ext` attribute when using `source` attribute",
                    #[cfg(feature = "code-in-doc")]
                    "must include `ext` attribute when using `source` or `in_doc` attribute",
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
            .or_else(|| path.extension().and_then(|s| s.to_str()))
            .unwrap_or_default();

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
                        "no escaper defined for extension '{escaping}'. You can define an escaper \
                        in the config file (named `rinja.toml` by default). {}",
                        MsgValidEscapers(&config.escapers),
                    ),
                    *ext_span,
                )
            })?;

        let mime_type =
            extension_to_mime_type(ext.as_deref().or_else(|| extension(&path)).unwrap_or("txt"))
                .to_string();

        let empty_punctuated = Punctuated::new();
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
            source_span: *source_span,
            block: block.as_deref(),
            print: *print,
            escaper,
            mime_type,
            path,
            fields,
        })
    }

    pub(crate) fn find_used_templates(
        &self,
        map: &mut HashMap<Arc<Path>, Arc<Parsed>, FxBuildHasher>,
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
}

#[derive(Debug)]
pub(crate) struct TemplateArgs {
    pub(crate) source: (Source, Option<Span>),
    block: Option<String>,
    print: Print,
    escaping: Option<String>,
    ext: Option<String>,
    ext_span: Option<Span>,
    syntax: Option<String>,
    config: Option<String>,
    pub(crate) whitespace: Option<Whitespace>,
    pub(crate) template_span: Option<Span>,
    pub(crate) config_span: Option<Span>,
}

impl TemplateArgs {
    pub(crate) fn new(ast: &syn::DeriveInput) -> Result<Self, CompileError> {
        // FIXME: implement once <https://github.com/rust-lang/rfcs/pull/3715> is stable
        if let syn::Data::Union(data) = &ast.data {
            return Err(CompileError::new_with_span(
                "rinja templates are not supported for `union` types, only `struct` and `enum`",
                None,
                Some(data.union_token.span),
            ));
        }

        let args = PartialTemplateArgs::new(&ast.attrs)?;
        let Some(template) = args.template else {
            return Err(CompileError::no_file_info(
                "no attribute `template` found",
                None,
            ));
        };
        Ok(Self {
            source: match args.source {
                Some(PartialTemplateArgsSource::Path(s)) => {
                    (Source::Path(s.value().into()), Some(s.span()))
                }
                Some(PartialTemplateArgsSource::Source(s)) => {
                    (Source::Source(s.value().into()), Some(s.span()))
                }
                #[cfg(feature = "code-in-doc")]
                Some(PartialTemplateArgsSource::InDoc(s)) => {
                    source_from_docs(s.span(), &args.meta_docs, ast)?
                }
                None => {
                    return Err(CompileError::no_file_info(
                        #[cfg(not(feature = "code-in-doc"))]
                        "specify one template argument `path` or `source`",
                        #[cfg(feature = "code-in-doc")]
                        "specify one template argument `path`, `source` or `in_doc`",
                        Some(template.span()),
                    ));
                }
            },
            block: args.block.map(|value| value.value()),
            print: args.print.unwrap_or_default(),
            escaping: args.escape.map(|value| value.value()),
            ext: args.ext.as_ref().map(|value| value.value()),
            ext_span: args.ext.as_ref().map(|value| value.span()),
            syntax: args.syntax.map(|value| value.value()),
            config: args.config.as_ref().map(|value| value.value()),
            whitespace: args.whitespace,
            template_span: Some(template.span()),
            config_span: args.config.as_ref().map(|value| value.span()),
        })
    }

    pub(crate) fn fallback() -> Self {
        Self {
            source: (Source::Source("".into()), None),
            block: None,
            print: Print::default(),
            escaping: None,
            ext: Some("txt".to_string()),
            ext_span: None,
            syntax: None,
            config: None,
            whitespace: None,
            template_span: None,
            config_span: None,
        }
    }

    pub(crate) fn config_path(&self) -> Option<&str> {
        self.config.as_deref()
    }
}

/// Try to find the source in the comment, in a `rinja` code block.
///
/// This is only done if no path or source was given in the `#[template]` attribute.
#[cfg(feature = "code-in-doc")]
fn source_from_docs(
    span: Span,
    docs: &[Attribute],
    ast: &syn::DeriveInput,
) -> Result<(Source, Option<Span>), CompileError> {
    let (source_span, source) = collect_comment_blocks(span, docs, ast)?;
    let source = strip_common_ws_prefix(source);
    let source = collect_rinja_code_blocks(span, ast, source)?;
    Ok((source, source_span))
}

#[cfg(feature = "code-in-doc")]
fn collect_comment_blocks(
    span: Span,
    docs: &[Attribute],
    ast: &syn::DeriveInput,
) -> Result<(Option<Span>, String), CompileError> {
    let mut source_span: Option<Span> = None;
    let mut assign_span = |kv: &syn::MetaNameValue| {
        // FIXME: uncomment once <https://github.com/rust-lang/rust/issues/54725> is stable
        // let new_span = kv.path.span();
        // source_span = Some(match source_span {
        //     Some(cur_span) => cur_span.join(new_span).unwrap_or(cur_span),
        //     None => new_span,
        // });

        if source_span.is_none() {
            source_span = Some(kv.path.span());
        }
    };

    let mut source = String::new();
    for a in docs {
        // is a comment?
        let Meta::NameValue(kv) = &a.meta else {
            continue;
        };
        if !kv.path.is_ident("doc") {
            continue;
        }

        // is an understood comment, e.g. not `#[doc = inline_str(…)]`
        let mut value = &kv.value;
        let value = loop {
            match value {
                Expr::Lit(lit) => break lit,
                Expr::Group(group) => value = &group.expr,
                _ => continue,
            }
        };
        let Lit::Str(value) = &value.lit else {
            continue;
        };

        assign_span(kv);
        source.push_str(value.value().as_str());
        source.push('\n');
    }
    if source.is_empty() {
        return Err(no_rinja_code_block(span, ast));
    }

    Ok((source_span, source))
}

#[cfg(feature = "code-in-doc")]
fn no_rinja_code_block(span: Span, ast: &syn::DeriveInput) -> CompileError {
    let kind = match &ast.data {
        syn::Data::Struct(_) => "struct",
        syn::Data::Enum(_) => "enum",
        // actually unreachable: `union`s are rejected by `TemplateArgs::new()`
        syn::Data::Union(_) => "union",
    };
    CompileError::no_file_info(
        format!(
            "when using `in_doc` with the value `true`, the {kind}'s documentation needs a \
             `rinja` code block"
        ),
        Some(span),
    )
}

#[cfg(feature = "code-in-doc")]
fn strip_common_ws_prefix(source: String) -> String {
    let mut common_prefix_iter = source
        .lines()
        .filter_map(|s| Some(&s[..s.find(|c: char| !c.is_ascii_whitespace())?]));
    let mut common_prefix = common_prefix_iter.next().unwrap_or_default();
    for p in common_prefix_iter {
        if common_prefix.is_empty() {
            break;
        }
        let ((pos, _), _) = common_prefix
            .char_indices()
            .zip(p.char_indices())
            .take_while(|(l, r)| l == r)
            .last()
            .unwrap_or_default();
        common_prefix = &common_prefix[..pos];
    }
    if common_prefix.is_empty() {
        return source;
    }

    source
        .lines()
        .flat_map(|s| [s.get(common_prefix.len()..).unwrap_or_default(), "\n"])
        .collect()
}

#[cfg(feature = "code-in-doc")]
fn collect_rinja_code_blocks(
    span: Span,
    ast: &syn::DeriveInput,
    source: String,
) -> Result<Source, CompileError> {
    use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

    let mut tmpl_source = String::new();
    let mut in_rinja_code = false;
    let mut had_rinja_code = false;
    for e in Parser::new(&source) {
        match (in_rinja_code, e) {
            (false, Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(s)))) => {
                if s.split(",").any(|s| JINJA_EXTENSIONS.contains(&s)) {
                    in_rinja_code = true;
                    had_rinja_code = true;
                }
            }
            (true, Event::End(TagEnd::CodeBlock)) => in_rinja_code = false,
            (true, Event::Text(text)) => tmpl_source.push_str(&text),
            _ => {}
        }
    }
    if !had_rinja_code {
        return Err(no_rinja_code_block(span, ast));
    }

    if tmpl_source.ends_with('\n') {
        tmpl_source.pop();
    }
    Ok(Source::Source(tmpl_source.into()))
}

struct ResultIter<I, E>(Result<I, Option<E>>);

impl<I: IntoIterator, E> From<Result<I, E>> for ResultIter<I::IntoIter, E> {
    fn from(value: Result<I, E>) -> Self {
        Self(match value {
            Ok(i) => Ok(i.into_iter()),
            Err(e) => Err(Some(e)),
        })
    }
}

impl<I: Iterator, E> Iterator for ResultIter<I, E> {
    type Item = Result<I::Item, E>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.0 {
            Ok(iter) => Some(Ok(iter.next()?)),
            Err(err) => Some(Err(err.take()?)),
        }
    }
}

impl<I: FusedIterator, E> FusedIterator for ResultIter<I, E> {}

fn extension(path: &Path) -> Option<&str> {
    let ext = path.extension()?.to_str()?;
    if JINJA_EXTENSIONS.contains(&ext) {
        // an extension was found: file stem cannot be absent
        Path::new(path.file_stem().unwrap())
            .extension()
            .and_then(|s| s.to_str())
            .or(Some(ext))
    } else {
        Some(ext)
    }
}

#[derive(Debug, Clone, Hash, PartialEq)]
pub(crate) enum Source {
    Path(Arc<str>),
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

impl FromStr for Print {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" => Ok(Self::All),
            "ast" => Ok(Self::Ast),
            "code" => Ok(Self::Code),
            "none" => Ok(Self::None),
            _ => Err(format!("invalid value for `print` option: {s}")),
        }
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

    CACHE.get_or_init(OnceMap::default).get_or_try_insert(
        tpl_path,
        |tpl_path| match read_to_string(tpl_path) {
            Ok(mut source) => {
                if source.ends_with('\n') {
                    let _ = source.pop();
                }
                Ok((Arc::clone(tpl_path), Arc::from(source)))
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
        Arc::clone,
    )
}

#[derive(Default)]
pub(crate) struct PartialTemplateArgs {
    pub(crate) template: Option<Ident>,
    pub(crate) meta_docs: Vec<Attribute>,
    pub(crate) source: Option<PartialTemplateArgsSource>,
    pub(crate) block: Option<LitStr>,
    pub(crate) print: Option<Print>,
    pub(crate) escape: Option<LitStr>,
    pub(crate) ext: Option<LitStr>,
    pub(crate) syntax: Option<LitStr>,
    pub(crate) config: Option<LitStr>,
    pub(crate) whitespace: Option<Whitespace>,
}

pub(crate) enum PartialTemplateArgsSource {
    Path(LitStr),
    Source(LitStr),
    #[cfg(feature = "code-in-doc")]
    InDoc(#[allow(dead_code)] LitBool),
}

// implement PartialTemplateArgs::new()
const _: () = {
    impl PartialTemplateArgs {
        pub(crate) fn new(attrs: &[Attribute]) -> Result<Self, CompileError> {
            new(attrs)
        }
    }

    #[inline]
    fn new(attrs: &[Attribute]) -> Result<PartialTemplateArgs, CompileError> {
        let mut this = PartialTemplateArgs::default();
        for attr in attrs {
            let Some(ident) = attr.path().get_ident() else {
                continue;
            };
            if ident == "doc" {
                this.meta_docs.push(attr.clone());
                continue;
            } else if ident == "template" {
                this.template = Some(ident.clone());
            } else {
                continue;
            }

            let args = attr
                .parse_args_with(<Punctuated<Meta, Token![,]>>::parse_terminated)
                .map_err(|e| {
                    CompileError::no_file_info(
                        format!("unable to parse template arguments: {e}"),
                        Some(attr.path().span()),
                    )
                })?;
            for arg in args {
                let pair = match arg {
                    Meta::NameValue(pair) => pair,
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

                let value = get_lit(ident, pair.value)?;

                if ident == "path" {
                    ensure_source_only_once(ident, &this.source)?;
                    this.source = Some(PartialTemplateArgsSource::Path(get_strlit(ident, value)?));
                } else if ident == "source" {
                    ensure_source_only_once(ident, &this.source)?;
                    this.source =
                        Some(PartialTemplateArgsSource::Source(get_strlit(ident, value)?));
                } else if ident == "in_doc" {
                    let value = get_boollit(ident, value)?;
                    if !value.value() {
                        continue;
                    }
                    ensure_source_only_once(ident, &this.source)?;

                    #[cfg(not(feature = "code-in-doc"))]
                    {
                        return Err(CompileError::no_file_info(
                            "enable feature `code-in-doc` to use `in_doc` argument",
                            Some(ident.span()),
                        ));
                    }
                    #[cfg(feature = "code-in-doc")]
                    {
                        this.source = Some(PartialTemplateArgsSource::InDoc(value));
                    }
                } else if ident == "block" {
                    set_strlit_pair(ident, value, &mut this.block)?;
                } else if ident == "print" {
                    set_parseable_string(ident, value, &mut this.print)?;
                } else if ident == "escape" {
                    set_strlit_pair(ident, value, &mut this.escape)?;
                } else if ident == "ext" {
                    set_strlit_pair(ident, value, &mut this.ext)?;
                } else if ident == "syntax" {
                    set_strlit_pair(ident, value, &mut this.syntax)?;
                } else if ident == "config" {
                    set_strlit_pair(ident, value, &mut this.config)?;
                } else if ident == "whitespace" {
                    set_parseable_string(ident, value, &mut this.whitespace)?;
                } else {
                    return Err(CompileError::no_file_info(
                        format!("unsupported template attribute `{ident}` found"),
                        Some(ident.span()),
                    ));
                }
            }
        }
        Ok(this)
    }

    fn set_strlit_pair(
        name: &Ident,
        value: ExprLit,
        dest: &mut Option<LitStr>,
    ) -> Result<(), CompileError> {
        ensure_only_once(name, dest)?;
        *dest = Some(get_strlit(name, value)?);
        Ok(())
    }

    fn set_parseable_string<T: FromStr<Err: ToString>>(
        name: &Ident,
        value: ExprLit,
        dest: &mut Option<T>,
    ) -> Result<(), CompileError> {
        ensure_only_once(name, dest)?;
        let str_value = get_strlit(name, value)?;
        *dest = Some(
            str_value
                .value()
                .parse()
                .map_err(|msg| CompileError::no_file_info(msg, Some(str_value.span())))?,
        );
        Ok(())
    }

    fn ensure_only_once<T>(name: &Ident, dest: &mut Option<T>) -> Result<(), CompileError> {
        if dest.is_none() {
            Ok(())
        } else {
            Err(CompileError::no_file_info(
                format!("template attribute `{name}` already set"),
                Some(name.span()),
            ))
        }
    }

    fn get_lit(name: &Ident, mut expr: Expr) -> Result<ExprLit, CompileError> {
        loop {
            match expr {
                Expr::Lit(lit) => return Ok(lit),
                Expr::Group(group) => expr = *group.expr,
                v => {
                    return Err(CompileError::no_file_info(
                        format!("template attribute `{name}` expects a literal"),
                        Some(v.span()),
                    ));
                }
            }
        }
    }

    fn get_strlit(name: &Ident, value: ExprLit) -> Result<LitStr, CompileError> {
        if let Lit::Str(s) = value.lit {
            Ok(s)
        } else {
            Err(CompileError::no_file_info(
                format!("template attribute `{name}` expects a string literal"),
                Some(value.lit.span()),
            ))
        }
    }

    fn get_boollit(name: &Ident, value: ExprLit) -> Result<LitBool, CompileError> {
        if let Lit::Bool(s) = value.lit {
            Ok(s)
        } else {
            Err(CompileError::no_file_info(
                format!("template attribute `{name}` expects a boolean value"),
                Some(value.lit.span()),
            ))
        }
    }

    fn ensure_source_only_once(
        name: &Ident,
        source: &Option<PartialTemplateArgsSource>,
    ) -> Result<(), CompileError> {
        if source.is_some() {
            return Err(CompileError::no_file_info(
                #[cfg(feature = "code-in-doc")]
                "must specify `source`, `path` or `is_doc` exactly once",
                #[cfg(not(feature = "code-in-doc"))]
                "must specify `source` or `path` exactly once",
                Some(name.span()),
            ));
        }
        Ok(())
    }
};

const JINJA_EXTENSIONS: &[&str] = &["j2", "jinja", "jinja2", "rinja"];

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
