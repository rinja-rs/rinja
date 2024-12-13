use std::borrow::Cow;
use std::collections::hash_map::{Entry, HashMap};
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;
use std::{mem, str};

use parser::node::{
    Call, Comment, Cond, CondTest, FilterBlock, If, Include, Let, Lit, Loop, Macro, Match,
    Whitespace, Ws,
};
use parser::{
    CharLit, CharPrefix, Expr, Filter, FloatKind, IntKind, Node, Num, Span, StrLit, StrPrefix,
    Target, WithSpan,
};
use rustc_hash::FxBuildHasher;

use crate::heritage::{Context, Heritage};
use crate::html::write_escaped_str;
use crate::input::{Source, TemplateInput};
use crate::integration::{Buffer, impl_everything, write_header};
use crate::{BUILTIN_FILTERS, CompileError, FileInfo, MsgValidEscapers, fmt_left, fmt_right};

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
    // The template input state: original struct AST and attributes
    input: &'a TemplateInput<'a>,
    // All contexts, keyed by the package-relative template path
    contexts: &'a HashMap<&'a Arc<Path>, Context<'a>, FxBuildHasher>,
    // The heritage contains references to blocks and their ancestry
    heritage: Option<&'h Heritage<'a, 'h>>,
    // Variables accessible directly from the current scope (not redirected to context)
    locals: MapChain<'a>,
    // Suffix whitespace from the previous literal. Will be flushed to the
    // output buffer unless suppressed by whitespace suppression on the next
    // non-literal.
    next_ws: Option<&'a str>,
    // Whitespace suppression from the previous non-literal. Will be used to
    // determine whether to flush prefix whitespace from the next literal.
    skip_ws: Whitespace,
    // If currently in a block, this will contain the name of a potential parent block
    super_block: Option<(&'a str, usize)>,
    // Buffer for writable
    buf_writable: WritableBuffer<'a>,
    // Used in blocks to check if we are inside a filter block.
    is_in_filter_block: usize,
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

    fn push_locals<T, F>(&mut self, callback: F) -> Result<T, CompileError>
    where
        F: FnOnce(&mut Self) -> Result<T, CompileError>,
    {
        self.locals.scopes.push(HashMap::default());
        let res = callback(self);
        self.locals.scopes.pop().unwrap();
        res
    }

    fn with_child<'b, T, F>(
        &mut self,
        heritage: Option<&'b Heritage<'a, 'b>>,
        callback: F,
    ) -> Result<T, CompileError>
    where
        F: FnOnce(&mut Generator<'a, 'b>) -> Result<T, CompileError>,
    {
        self.locals.scopes.push(HashMap::default());

        let buf_writable = mem::take(&mut self.buf_writable);
        let locals = mem::replace(&mut self.locals, MapChain::new_empty());

        let mut child = Generator::new(
            self.input,
            self.contexts,
            heritage,
            locals,
            self.buf_writable.discard,
            self.is_in_filter_block,
        );
        child.buf_writable = buf_writable;
        let res = callback(&mut child);
        Generator {
            locals: self.locals,
            buf_writable: self.buf_writable,
            ..
        } = child;

        self.locals.scopes.pop().unwrap();
        res
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
            "fn render_into<RinjaW>(&self, writer: &mut RinjaW) -> rinja::Result<()>\
            where \
                RinjaW: rinja::helpers::core::fmt::Write + ?rinja::helpers::core::marker::Sized\
            {\
                use rinja::filters::{AutoEscape as _, WriteWritable as _};\
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

    fn impl_template_inner(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
    ) -> Result<usize, CompileError> {
        buf.set_discard(self.buf_writable.discard);
        let size_hint = if let Some(heritage) = self.heritage {
            self.handle(heritage.root, heritage.root.nodes, buf, AstLevel::Top)
        } else {
            self.handle(ctx, ctx.nodes, buf, AstLevel::Top)
        }?;
        self.flush_ws(Ws(None, None));
        buf.set_discard(false);
        Ok(size_hint)
    }

    // Helper methods for handling node types

    fn handle(
        &mut self,
        ctx: &Context<'a>,
        nodes: &'a [Node<'_>],
        buf: &mut Buffer,
        level: AstLevel,
    ) -> Result<usize, CompileError> {
        let mut size_hint = 0;
        for n in nodes {
            match *n {
                Node::Lit(ref lit) => {
                    self.visit_lit(lit);
                }
                Node::Comment(ref comment) => {
                    self.write_comment(comment);
                }
                Node::Expr(ws, ref val) => {
                    self.write_expr(ws, val);
                }
                Node::Let(ref l) => {
                    self.write_let(ctx, buf, l)?;
                }
                Node::If(ref i) => {
                    size_hint += self.write_if(ctx, buf, i)?;
                }
                Node::Match(ref m) => {
                    size_hint += self.write_match(ctx, buf, m)?;
                }
                Node::Loop(ref loop_block) => {
                    size_hint += self.write_loop(ctx, buf, loop_block)?;
                }
                Node::BlockDef(ref b) => {
                    size_hint +=
                        self.write_block(ctx, buf, Some(b.name), Ws(b.ws1.0, b.ws2.1), b.span())?;
                }
                Node::Include(ref i) => {
                    size_hint += self.handle_include(ctx, buf, i)?;
                }
                Node::Call(ref call) => {
                    size_hint += self.write_call(ctx, buf, call)?;
                }
                Node::FilterBlock(ref filter) => {
                    size_hint += self.write_filter_block(ctx, buf, filter)?;
                }
                Node::Macro(ref m) => {
                    if level != AstLevel::Top {
                        return Err(ctx.generate_error(
                            "macro blocks only allowed at the top level",
                            m.span(),
                        ));
                    }
                    self.flush_ws(m.ws1);
                    self.prepare_ws(m.ws2);
                }
                Node::Raw(ref raw) => {
                    self.handle_ws(raw.ws1);
                    self.visit_lit(&raw.lit);
                    self.handle_ws(raw.ws2);
                }
                Node::Import(ref i) => {
                    if level != AstLevel::Top {
                        return Err(ctx.generate_error(
                            "import blocks only allowed at the top level",
                            i.span(),
                        ));
                    }
                    self.handle_ws(i.ws);
                }
                Node::Extends(ref e) => {
                    if level != AstLevel::Top {
                        return Err(ctx.generate_error(
                            "extend blocks only allowed at the top level",
                            e.span(),
                        ));
                    }
                    // No whitespace handling: child template top-level is not used,
                    // except for the blocks defined in it.
                }
                Node::Break(ref ws) => {
                    self.handle_ws(**ws);
                    self.write_buf_writable(ctx, buf)?;
                    buf.write("break;");
                }
                Node::Continue(ref ws) => {
                    self.handle_ws(**ws);
                    self.write_buf_writable(ctx, buf)?;
                    buf.write("continue;");
                }
            }
        }

        if AstLevel::Top == level {
            // Handle any pending whitespace.
            if self.next_ws.is_some() {
                self.flush_ws(Ws(Some(self.skip_ws), None));
            }

            size_hint += self.write_buf_writable(ctx, buf)?;
        }
        Ok(size_hint)
    }

    fn is_var_defined(&self, var_name: &str) -> bool {
        self.locals.get(var_name).is_some() || self.input.fields.iter().any(|f| f == var_name)
    }

    fn evaluate_condition(
        &self,
        expr: WithSpan<'a, Expr<'a>>,
        only_contains_is_defined: &mut bool,
    ) -> (EvaluatedResult, WithSpan<'a, Expr<'a>>) {
        let (expr, span) = expr.deconstruct();

        match expr {
            Expr::NumLit(_, _)
            | Expr::StrLit(_)
            | Expr::CharLit(_)
            | Expr::Var(_)
            | Expr::Path(_)
            | Expr::Array(_)
            | Expr::Attr(_, _)
            | Expr::Index(_, _)
            | Expr::Filter(_)
            | Expr::Range(_, _, _)
            | Expr::Call(_, _)
            | Expr::RustMacro(_, _)
            | Expr::Try(_)
            | Expr::Tuple(_)
            | Expr::NamedArgument(_, _)
            | Expr::FilterSource
            | Expr::As(_, _)
            | Expr::Concat(_) => {
                *only_contains_is_defined = false;
                (EvaluatedResult::Unknown, WithSpan::new(expr, span))
            }
            Expr::BoolLit(true) => (EvaluatedResult::AlwaysTrue, WithSpan::new(expr, span)),
            Expr::BoolLit(false) => (EvaluatedResult::AlwaysFalse, WithSpan::new(expr, span)),
            Expr::Unary("!", inner) => {
                let (result, expr) = self.evaluate_condition(*inner, only_contains_is_defined);
                match result {
                    EvaluatedResult::AlwaysTrue => (
                        EvaluatedResult::AlwaysFalse,
                        WithSpan::new(Expr::BoolLit(false), ""),
                    ),
                    EvaluatedResult::AlwaysFalse => (
                        EvaluatedResult::AlwaysTrue,
                        WithSpan::new(Expr::BoolLit(true), ""),
                    ),
                    EvaluatedResult::Unknown => (
                        EvaluatedResult::Unknown,
                        WithSpan::new(Expr::Unary("!", Box::new(expr)), span),
                    ),
                }
            }
            Expr::Unary(_, _) => (EvaluatedResult::Unknown, WithSpan::new(expr, span)),
            Expr::BinOp("&&", left, right) => {
                let (result_left, expr_left) =
                    self.evaluate_condition(*left, only_contains_is_defined);
                if result_left == EvaluatedResult::AlwaysFalse {
                    // The right side of the `&&` won't be evaluated, no need to go any further.
                    return (result_left, WithSpan::new(Expr::BoolLit(false), ""));
                }
                let (result_right, expr_right) =
                    self.evaluate_condition(*right, only_contains_is_defined);
                match (result_left, result_right) {
                    (EvaluatedResult::AlwaysTrue, EvaluatedResult::AlwaysTrue) => (
                        EvaluatedResult::AlwaysTrue,
                        WithSpan::new(Expr::BoolLit(true), ""),
                    ),
                    (_, EvaluatedResult::AlwaysFalse) => (
                        EvaluatedResult::AlwaysFalse,
                        WithSpan::new(
                            Expr::BinOp("&&", Box::new(expr_left), Box::new(expr_right)),
                            span,
                        ),
                    ),
                    (EvaluatedResult::AlwaysTrue, _) => (result_right, expr_right),
                    (_, EvaluatedResult::AlwaysTrue) => (result_left, expr_left),
                    _ => (
                        EvaluatedResult::Unknown,
                        WithSpan::new(
                            Expr::BinOp("&&", Box::new(expr_left), Box::new(expr_right)),
                            span,
                        ),
                    ),
                }
            }
            Expr::BinOp("||", left, right) => {
                let (result_left, expr_left) =
                    self.evaluate_condition(*left, only_contains_is_defined);
                if result_left == EvaluatedResult::AlwaysTrue {
                    // The right side of the `||` won't be evaluated, no need to go any further.
                    return (result_left, WithSpan::new(Expr::BoolLit(true), ""));
                }
                let (result_right, expr_right) =
                    self.evaluate_condition(*right, only_contains_is_defined);
                match (result_left, result_right) {
                    (EvaluatedResult::AlwaysFalse, EvaluatedResult::AlwaysFalse) => (
                        EvaluatedResult::AlwaysFalse,
                        WithSpan::new(Expr::BoolLit(false), ""),
                    ),
                    (_, EvaluatedResult::AlwaysTrue) => (
                        EvaluatedResult::AlwaysTrue,
                        WithSpan::new(
                            Expr::BinOp("||", Box::new(expr_left), Box::new(expr_right)),
                            span,
                        ),
                    ),
                    (EvaluatedResult::AlwaysFalse, _) => (result_right, expr_right),
                    (_, EvaluatedResult::AlwaysFalse) => (result_left, expr_left),
                    _ => (
                        EvaluatedResult::Unknown,
                        WithSpan::new(
                            Expr::BinOp("||", Box::new(expr_left), Box::new(expr_right)),
                            span,
                        ),
                    ),
                }
            }
            Expr::BinOp(_, _, _) => {
                *only_contains_is_defined = false;
                (EvaluatedResult::Unknown, WithSpan::new(expr, span))
            }
            Expr::Group(inner) => {
                let (result, expr) = self.evaluate_condition(*inner, only_contains_is_defined);
                (result, WithSpan::new(Expr::Group(Box::new(expr)), span))
            }
            Expr::IsDefined(left) => {
                // Variable is defined so we want to keep the condition.
                if self.is_var_defined(left) {
                    (
                        EvaluatedResult::AlwaysTrue,
                        WithSpan::new(Expr::BoolLit(true), ""),
                    )
                } else {
                    (
                        EvaluatedResult::AlwaysFalse,
                        WithSpan::new(Expr::BoolLit(false), ""),
                    )
                }
            }
            Expr::IsNotDefined(left) => {
                // Variable is defined so we don't want to keep the condition.
                if self.is_var_defined(left) {
                    (
                        EvaluatedResult::AlwaysFalse,
                        WithSpan::new(Expr::BoolLit(false), ""),
                    )
                } else {
                    (
                        EvaluatedResult::AlwaysTrue,
                        WithSpan::new(Expr::BoolLit(true), ""),
                    )
                }
            }
        }
    }

    fn write_if(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        if_: &'a If<'_>,
    ) -> Result<usize, CompileError> {
        let mut flushed = 0;
        let mut arm_sizes = Vec::new();
        let mut has_else = false;

        let conds = Conds::compute_branches(self, if_);

        if let Some(ws_before) = conds.ws_before {
            self.handle_ws(ws_before);
        }

        let mut iter = conds.conds.iter().enumerate().peekable();
        while let Some((pos, cond_info)) = iter.next() {
            let cond = cond_info.cond;

            if pos == 0 {
                self.handle_ws(cond.ws);
                flushed += self.write_buf_writable(ctx, buf)?;
            }

            self.push_locals(|this| {
                let mut arm_size = 0;

                if let Some(CondTest { target, expr, .. }) = &cond.cond {
                    let expr = cond_info.cond_expr.as_ref().unwrap_or(expr);

                    if pos == 0 {
                        if cond_info.generate_condition {
                            buf.write("if ");
                        }
                        // Otherwise it means it will be the only condition generated,
                        // so nothing to be added here.
                    } else if cond_info.generate_condition {
                        buf.write("} else if ");
                    } else {
                        buf.write("} else {");
                        has_else = true;
                    }

                    if let Some(target) = target {
                        let mut expr_buf = Buffer::new();
                        buf.write("let ");
                        // If this is a chain condition, then we need to declare the variable after the
                        // left expression has been handled but before the right expression is handled
                        // but this one should have access to the let-bound variable.
                        match &**expr {
                            Expr::BinOp(op, ref left, ref right) if *op == "||" || *op == "&&" => {
                                this.visit_expr(ctx, &mut expr_buf, left)?;
                                this.visit_target(buf, true, true, target);
                                expr_buf.write(format_args!(" {op} "));
                                this.visit_condition(ctx, &mut expr_buf, right)?;
                            }
                            _ => {
                                this.visit_expr(ctx, &mut expr_buf, expr)?;
                                this.visit_target(buf, true, true, target);
                            }
                        }
                        buf.write(format_args!("= &{expr_buf} {{"));
                    } else if cond_info.generate_condition {
                        this.visit_condition(ctx, buf, expr)?;
                        buf.write('{');
                    }
                } else if pos != 0 {
                    buf.write("} else {");
                    has_else = true;
                }

                if cond_info.generate_content {
                    arm_size += this.handle(ctx, &cond.nodes, buf, AstLevel::Nested)?;
                }
                arm_sizes.push(arm_size);

                if let Some((_, cond_info)) = iter.peek() {
                    let cond = cond_info.cond;

                    this.handle_ws(cond.ws);
                    flushed += this.write_buf_writable(ctx, buf)?;
                } else {
                    if let Some(ws_after) = conds.ws_after {
                        this.handle_ws(ws_after);
                    }
                    this.handle_ws(if_.ws);
                    flushed += this.write_buf_writable(ctx, buf)?;
                }
                Ok(0)
            })?;
        }

        if conds.nb_conds > 0 {
            buf.write('}');
        }

        if !has_else && !conds.conds.is_empty() {
            arm_sizes.push(0);
        }
        Ok(flushed + median(&mut arm_sizes))
    }

    #[allow(clippy::too_many_arguments)]
    fn write_match(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        m: &'a Match<'a>,
    ) -> Result<usize, CompileError> {
        let Match {
            ws1,
            ref expr,
            ref arms,
            ws2,
        } = *m;

        self.flush_ws(ws1);
        let flushed = self.write_buf_writable(ctx, buf)?;
        let mut arm_sizes = Vec::new();

        let expr_code = self.visit_expr_root(ctx, expr)?;
        buf.write(format_args!("match &{expr_code} {{"));

        let mut arm_size = 0;
        let mut iter = arms.iter().enumerate().peekable();
        while let Some((i, arm)) = iter.next() {
            if i == 0 {
                self.handle_ws(arm.ws);
            }

            self.push_locals(|this| {
                for (index, target) in arm.target.iter().enumerate() {
                    if index != 0 {
                        buf.write('|');
                    }
                    this.visit_target(buf, true, true, target);
                }
                buf.write(" => {");

                arm_size = this.handle(ctx, &arm.nodes, buf, AstLevel::Nested)?;

                if let Some((_, arm)) = iter.peek() {
                    this.handle_ws(arm.ws);
                    arm_sizes.push(arm_size + this.write_buf_writable(ctx, buf)?);

                    buf.write('}');
                } else {
                    this.handle_ws(ws2);
                    arm_sizes.push(arm_size + this.write_buf_writable(ctx, buf)?);
                    buf.write('}');
                }
                Ok(0)
            })?;
        }

        buf.write('}');

        Ok(flushed + median(&mut arm_sizes))
    }

    #[allow(clippy::too_many_arguments)]
    fn write_loop(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        loop_block: &'a WithSpan<'_, Loop<'_>>,
    ) -> Result<usize, CompileError> {
        self.handle_ws(loop_block.ws1);
        self.push_locals(|this| {
            let expr_code = this.visit_expr_root(ctx, &loop_block.iter)?;

            let has_else_nodes = !loop_block.else_nodes.is_empty();

            let flushed = this.write_buf_writable(ctx, buf)?;
            buf.write('{');
            if has_else_nodes {
                buf.write("let mut _did_loop = false;");
            }
            match &*loop_block.iter {
                Expr::Range(_, _, _) => buf.write(format_args!("let _iter = {expr_code};")),
                Expr::Array(..) => buf.write(format_args!("let _iter = {expr_code}.iter();")),
                // If `iter` is a call then we assume it's something that returns
                // an iterator. If not then the user can explicitly add the needed
                // call without issues.
                Expr::Call(..) | Expr::Index(..) => {
                    buf.write(format_args!("let _iter = ({expr_code}).into_iter();"));
                }
                // If accessing `self` then it most likely needs to be
                // borrowed, to prevent an attempt of moving.
                _ if expr_code.starts_with("self.") => {
                    buf.write(format_args!("let _iter = (&{expr_code}).into_iter();"));
                }
                // If accessing a field then it most likely needs to be
                // borrowed, to prevent an attempt of moving.
                Expr::Attr(..) => {
                    buf.write(format_args!("let _iter = (&{expr_code}).into_iter();"));
                }
                // Otherwise, we borrow `iter` assuming that it implements `IntoIterator`.
                _ => buf.write(format_args!("let _iter = ({expr_code}).into_iter();")),
            }
            if let Some(cond) = &loop_block.cond {
                this.push_locals(|this| {
                    buf.write("let _iter = _iter.filter(|");
                    this.visit_target(buf, true, true, &loop_block.var);
                    buf.write("| -> bool {");
                    this.visit_expr(ctx, buf, cond)?;
                    buf.write("});");
                    Ok(0)
                })?;
            }

            let size_hint1 = this.push_locals(|this| {
                buf.write("for (");
                this.visit_target(buf, true, true, &loop_block.var);
                buf.write(", _loop_item) in rinja::helpers::TemplateLoop::new(_iter) {");

                if has_else_nodes {
                    buf.write("_did_loop = true;");
                }
                let mut size_hint1 = this.handle(ctx, &loop_block.body, buf, AstLevel::Nested)?;
                this.handle_ws(loop_block.ws2);
                size_hint1 += this.write_buf_writable(ctx, buf)?;
                Ok(size_hint1)
            })?;
            buf.write('}');

            let size_hint2;
            if has_else_nodes {
                buf.write("if !_did_loop {");
                size_hint2 = this.push_locals(|this| {
                    let mut size_hint =
                        this.handle(ctx, &loop_block.else_nodes, buf, AstLevel::Nested)?;
                    this.handle_ws(loop_block.ws3);
                    size_hint += this.write_buf_writable(ctx, buf)?;
                    Ok(size_hint)
                })?;
                buf.write('}');
            } else {
                this.handle_ws(loop_block.ws3);
                size_hint2 = this.write_buf_writable(ctx, buf)?;
            }

            buf.write('}');
            Ok(flushed + ((size_hint1 * 3) + size_hint2) / 2)
        })
    }

    fn write_call(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        call: &'a WithSpan<'_, Call<'_>>,
    ) -> Result<usize, CompileError> {
        let Call {
            ws,
            scope,
            name,
            ref args,
        } = **call;
        if name == "super" {
            return self.write_block(ctx, buf, None, ws, call.span());
        }

        let (def, own_ctx) = if let Some(s) = scope {
            let path = ctx.imports.get(s).ok_or_else(|| {
                ctx.generate_error(format_args!("no import found for scope {s:?}"), call.span())
            })?;
            let mctx = self.contexts.get(path).ok_or_else(|| {
                ctx.generate_error(format_args!("context for {path:?} not found"), call.span())
            })?;
            let def = mctx.macros.get(name).ok_or_else(|| {
                ctx.generate_error(
                    format_args!("macro {name:?} not found in scope {s:?}"),
                    call.span(),
                )
            })?;
            (def, mctx)
        } else {
            let def = ctx.macros.get(name).ok_or_else(|| {
                ctx.generate_error(format_args!("macro {name:?} not found"), call.span())
            })?;
            (def, ctx)
        };

        self.flush_ws(ws); // Cannot handle_ws() here: whitespace from macro definition comes first
        let size_hint = self.push_locals(|this| {
            macro_call_ensure_arg_count(call, def, ctx)?;

            this.write_buf_writable(ctx, buf)?;
            buf.write('{');
            this.prepare_ws(def.ws1);

            let mut named_arguments: HashMap<&str, _, FxBuildHasher> = HashMap::default();
            // Since named arguments can only be passed last, we only need to check if the last argument
            // is a named one.
            if let Some(Expr::NamedArgument(_, _)) = args.last().map(|expr| &**expr) {
                // First we check that all named arguments actually exist in the called item.
                for (index, arg) in args.iter().enumerate().rev() {
                    let Expr::NamedArgument(arg_name, _) = &**arg else {
                        break;
                    };
                    if !def.args.iter().any(|(arg, _)| arg == arg_name) {
                        return Err(ctx.generate_error(
                            format_args!("no argument named `{arg_name}` in macro {name:?}"),
                            call.span(),
                        ));
                    }
                    named_arguments.insert(arg_name, (index, arg));
                }
            }

            let mut value = Buffer::new();

            // Handling both named and unnamed arguments requires to be careful of the named arguments
            // order. To do so, we iterate through the macro defined arguments and then check if we have
            // a named argument with this name:
            //
            // * If there is one, we add it and move to the next argument.
            // * If there isn't one, then we pick the next argument (we can do it without checking
            //   anything since named arguments are always last).
            let mut allow_positional = true;
            let mut used_named_args = vec![false; args.len()];
            for (index, (arg, default_value)) in def.args.iter().enumerate() {
                let expr = if let Some((index, expr)) = named_arguments.get(arg) {
                    used_named_args[*index] = true;
                    allow_positional = false;
                    expr
                } else {
                    match args.get(index) {
                        Some(arg_expr) if !matches!(**arg_expr, Expr::NamedArgument(_, _)) => {
                            // If there is already at least one named argument, then it's not allowed
                            // to use unnamed ones at this point anymore.
                            if !allow_positional {
                                return Err(ctx.generate_error(
                                    format_args!(
                                        "cannot have unnamed argument (`{arg}`) after named argument \
                                         in call to macro {name:?}"
                                    ),
                                    call.span(),
                                ));
                            }
                            arg_expr
                        }
                        Some(arg_expr) if used_named_args[index] => {
                            let Expr::NamedArgument(name, _) = **arg_expr else { unreachable!() };
                            return Err(ctx.generate_error(
                                format_args!("`{name}` is passed more than once"),
                                call.span(),
                            ));
                        }
                        _ => {
                            if let Some(default_value) = default_value {
                                default_value
                            } else {
                                return Err(ctx.generate_error(format_args!("missing `{arg}` argument"), call.span()));
                            }
                        }
                    }
                };
                match &**expr {
                    // If `expr` is already a form of variable then
                    // don't reintroduce a new variable. This is
                    // to avoid moving non-copyable values.
                    Expr::Var(name) if *name != "self" => {
                        let var = this.locals.resolve_or_self(name);
                        this.locals
                            .insert(Cow::Borrowed(arg), LocalMeta::with_ref(var));
                    }
                    Expr::Attr(obj, attr) => {
                        let mut attr_buf = Buffer::new();
                        this.visit_attr(ctx, &mut attr_buf, obj, attr)?;

                        let attr = attr_buf.into_string();
                        let var = this.locals.resolve(&attr).unwrap_or(attr);
                        this.locals
                            .insert(Cow::Borrowed(arg), LocalMeta::with_ref(var));
                    }
                    // Everything else still needs to become variables,
                    // to avoid having the same logic be executed
                    // multiple times, e.g. in the case of macro
                    // parameters being used multiple times.
                    _ => {
                        value.clear();
                        let (before, after) = if !is_copyable(expr) {
                            ("&(", ")")
                        } else {
                            ("", "")
                        };
                        value.write(this.visit_expr_root(ctx, expr)?);
                        // We need to normalize the arg to write it, thus we need to add it to
                        // locals in the normalized manner
                        let normalized_arg = normalize_identifier(arg);
                        buf.write(format_args!("let {} = {before}{value}{after};", normalized_arg));
                        this.locals.insert_with_default(Cow::Borrowed(normalized_arg));
                    }
                }
            }

            let mut size_hint = this.handle(own_ctx, &def.nodes, buf, AstLevel::Nested)?;

            this.flush_ws(def.ws2);
            size_hint += this.write_buf_writable(ctx, buf)?;
            buf.write('}');
            Ok(size_hint)
        })?;
        self.prepare_ws(ws);
        Ok(size_hint)
    }

    fn write_filter_block(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        filter: &'a WithSpan<'_, FilterBlock<'_>>,
    ) -> Result<usize, CompileError> {
        self.write_buf_writable(ctx, buf)?;
        self.flush_ws(filter.ws1);
        self.is_in_filter_block += 1;
        self.write_buf_writable(ctx, buf)?;
        buf.write('{');

        // build `FmtCell` that contains the inner block
        buf.write(format_args!(
            "let {FILTER_SOURCE} = rinja::helpers::FmtCell::new(\
                |writer: &mut rinja::helpers::core::fmt::Formatter<'_>| -> rinja::Result<()> {{"
        ));
        let size_hint = self.push_locals(|this| {
            this.prepare_ws(filter.ws1);
            let size_hint = this.handle(ctx, &filter.nodes, buf, AstLevel::Nested)?;
            this.flush_ws(filter.ws2);
            this.write_buf_writable(ctx, buf)?;
            Ok(size_hint)
        })?;
        buf.write(
            "\
                rinja::Result::Ok(())\
            });",
        );

        // display the `FmtCell`
        let mut filter_buf = Buffer::new();
        let display_wrap = self.visit_filter(
            ctx,
            &mut filter_buf,
            filter.filters.name,
            &filter.filters.arguments,
            filter.span(),
        )?;
        let filter_buf = match display_wrap {
            DisplayWrap::Wrapped => fmt_left!("{filter_buf}"),
            DisplayWrap::Unwrapped => fmt_right!(
                "(&&rinja::filters::AutoEscaper::new(&({filter_buf}), {})).rinja_auto_escape()?",
                self.input.escaper,
            ),
        };
        buf.write(format_args!(
            "if rinja::helpers::core::write!(writer, \"{{}}\", {filter_buf}).is_err() {{\
                return {FILTER_SOURCE}.take_err();\
            }}"
        ));

        buf.write('}');
        self.is_in_filter_block -= 1;
        self.prepare_ws(filter.ws2);
        Ok(size_hint)
    }

    fn handle_include(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        i: &'a WithSpan<'_, Include<'_>>,
    ) -> Result<usize, CompileError> {
        self.flush_ws(i.ws);
        self.write_buf_writable(ctx, buf)?;
        let file_info = ctx
            .path
            .map(|path| FileInfo::of(i.span(), path, ctx.parsed));
        let path = self
            .input
            .config
            .find_template(i.path, Some(&self.input.path), file_info)?;

        // We clone the context of the child in order to preserve their macros and imports.
        // But also add all the imports and macros from this template that don't override the
        // child's ones to preserve this template's context.
        let child_ctx = &mut self.contexts[&path].clone();
        for (name, mac) in &ctx.macros {
            child_ctx.macros.entry(name).or_insert(mac);
        }
        for (name, import) in &ctx.imports {
            child_ctx
                .imports
                .entry(name)
                .or_insert_with(|| import.clone());
        }

        // Create a new generator for the child, and call it like in `impl_template` as if it were
        // a full template, while preserving the context.
        let heritage = if !child_ctx.blocks.is_empty() || child_ctx.extends.is_some() {
            Some(Heritage::new(child_ctx, self.contexts))
        } else {
            None
        };

        let handle_ctx = match &heritage {
            Some(heritage) => heritage.root,
            None => child_ctx,
        };

        let size_hint = self.with_child(heritage.as_ref(), |child| {
            let mut size_hint = 0;
            size_hint += child.handle(handle_ctx, handle_ctx.nodes, buf, AstLevel::Top)?;
            size_hint += child.write_buf_writable(handle_ctx, buf)?;
            Ok(size_hint)
        })?;

        self.prepare_ws(i.ws);

        Ok(size_hint)
    }

    fn is_shadowing_variable(
        &self,
        ctx: &Context<'_>,
        var: &Target<'a>,
        l: Span<'_>,
    ) -> Result<bool, CompileError> {
        match var {
            Target::Name(name) => {
                let name = normalize_identifier(name);
                match self.locals.get(name) {
                    // declares a new variable
                    None => Ok(false),
                    // an initialized variable gets shadowed
                    Some(meta) if meta.initialized => Ok(true),
                    // initializes a variable that was introduced in a LetDecl before
                    _ => Ok(false),
                }
            }
            Target::Placeholder(_) => Ok(false),
            Target::Rest(var_name) => {
                if let Some(var_name) = **var_name {
                    match self.is_shadowing_variable(ctx, &Target::Name(var_name), l) {
                        Ok(false) => {}
                        outcome => return outcome,
                    }
                }
                Ok(false)
            }
            Target::Tuple(_, targets) => {
                for target in targets {
                    match self.is_shadowing_variable(ctx, target, l) {
                        Ok(false) => continue,
                        outcome => return outcome,
                    }
                }
                Ok(false)
            }
            Target::Struct(_, named_targets) => {
                for (_, target) in named_targets {
                    match self.is_shadowing_variable(ctx, target, l) {
                        Ok(false) => continue,
                        outcome => return outcome,
                    }
                }
                Ok(false)
            }
            _ => Err(ctx.generate_error(
                "literals are not allowed on the left-hand side of an assignment",
                l,
            )),
        }
    }

    fn write_let(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        l: &'a WithSpan<'_, Let<'_>>,
    ) -> Result<(), CompileError> {
        self.handle_ws(l.ws);

        let Some(val) = &l.val else {
            self.write_buf_writable(ctx, buf)?;
            buf.write("let ");
            self.visit_target(buf, false, true, &l.var);
            buf.write(';');
            return Ok(());
        };

        let mut expr_buf = Buffer::new();
        self.visit_expr(ctx, &mut expr_buf, val)?;

        let shadowed = self.is_shadowing_variable(ctx, &l.var, l.span())?;
        if shadowed {
            // Need to flush the buffer if the variable is being shadowed,
            // to ensure the old variable is used.
            self.write_buf_writable(ctx, buf)?;
        }
        if shadowed
            || !matches!(l.var, Target::Name(_))
            || matches!(&l.var, Target::Name(name) if self.locals.get(name).is_none())
        {
            buf.write("let ");
        }

        self.visit_target(buf, true, true, &l.var);
        let (before, after) = if !is_copyable(val) {
            ("&(", ")")
        } else {
            ("", "")
        };
        buf.write(format_args!(" = {before}{expr_buf}{after};"));
        Ok(())
    }

    // If `name` is `Some`, this is a call to a block definition, and we have to find
    // the first block for that name from the ancestry chain. If name is `None`, this
    // is from a `super()` call, and we can get the name from `self.super_block`.
    fn write_block(
        &mut self,
        ctx: &Context<'a>,
        buf: &mut Buffer,
        name: Option<&'a str>,
        outer: Ws,
        node: Span<'_>,
    ) -> Result<usize, CompileError> {
        if self.is_in_filter_block > 0 {
            return Err(ctx.generate_error("cannot have a block inside a filter block", node));
        }
        // Flush preceding whitespace according to the outer WS spec
        self.flush_ws(outer);

        let cur = match (name, self.super_block) {
            // The top-level context contains a block definition
            (Some(cur_name), None) => (cur_name, 0),
            // A block definition contains a block definition of the same name
            (Some(cur_name), Some((prev_name, _))) if cur_name == prev_name => {
                return Err(ctx.generate_error(
                    format_args!("cannot define recursive blocks ({cur_name})"),
                    node,
                ));
            }
            // A block definition contains a definition of another block
            (Some(cur_name), Some((_, _))) => (cur_name, 0),
            // `super()` was called inside a block
            (None, Some((prev_name, gen))) => (prev_name, gen + 1),
            // `super()` is called from outside a block
            (None, None) => {
                return Err(ctx.generate_error("cannot call 'super()' outside block", node));
            }
        };

        self.write_buf_writable(ctx, buf)?;

        let block_fragment_write = self.input.block == name && self.buf_writable.discard;
        // Allow writing to the buffer if we're in the block fragment
        if block_fragment_write {
            self.buf_writable.discard = false;
        }
        let prev_buf_discard = buf.is_discard();
        buf.set_discard(self.buf_writable.discard);

        // Get the block definition from the heritage chain
        let heritage = self
            .heritage
            .ok_or_else(|| ctx.generate_error("no block ancestors available", node))?;
        let (child_ctx, def) = *heritage.blocks[cur.0].get(cur.1).ok_or_else(|| {
            ctx.generate_error(
                match name {
                    None => fmt_left!("no super() block found for block '{}'", cur.0),
                    Some(name) => fmt_right!(move "no block found for name '{name}'"),
                },
                node,
            )
        })?;

        // We clone the context of the child in order to preserve their macros and imports.
        // But also add all the imports and macros from this template that don't override the
        // child's ones to preserve this template's context.
        let mut child_ctx = child_ctx.clone();
        for (name, mac) in &ctx.macros {
            child_ctx.macros.entry(name).or_insert(mac);
        }
        for (name, import) in &ctx.imports {
            child_ctx
                .imports
                .entry(name)
                .or_insert_with(|| import.clone());
        }

        let size_hint = self.with_child(Some(heritage), |child| {
            // Handle inner whitespace suppression spec and process block nodes
            child.prepare_ws(def.ws1);

            child.super_block = Some(cur);
            let size_hint = child.handle(&child_ctx, &def.nodes, buf, AstLevel::Block)?;

            if !child.locals.is_current_empty() {
                // Need to flush the buffer before popping the variable stack
                child.write_buf_writable(ctx, buf)?;
            }

            child.flush_ws(def.ws2);
            Ok(size_hint)
        })?;

        // Restore original block context and set whitespace suppression for
        // succeeding whitespace according to the outer WS spec
        self.prepare_ws(outer);

        // If we are rendering a specific block and the discard changed, it means that we're done
        // with the block we want to render and that from this point, everything will be discarded.
        //
        // To get this block content rendered as well, we need to write to the buffer before then.
        if buf.is_discard() != prev_buf_discard {
            self.write_buf_writable(ctx, buf)?;
        }
        // Restore the original buffer discarding state
        if block_fragment_write {
            self.buf_writable.discard = true;
        }
        buf.set_discard(prev_buf_discard);

        Ok(size_hint)
    }

    fn write_expr(&mut self, ws: Ws, s: &'a WithSpan<'a, Expr<'a>>) {
        self.handle_ws(ws);
        let items = if let Expr::Concat(exprs) = &**s {
            exprs
        } else {
            std::slice::from_ref(s)
        };
        for s in items {
            self.buf_writable
                .push(compile_time_escape(s, self.input.escaper).unwrap_or(Writable::Expr(s)));
        }
    }

    // Write expression buffer and empty
    fn write_buf_writable(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
    ) -> Result<usize, CompileError> {
        let mut size_hint = 0;
        let items = mem::take(&mut self.buf_writable.buf);
        let mut it = items.iter().enumerate().peekable();

        while let Some((_, Writable::Lit(s))) = it.peek() {
            size_hint += buf.write_writer(s);
            it.next();
        }
        if it.peek().is_none() {
            return Ok(size_hint);
        }

        let mut targets = Buffer::new();
        let mut lines = Buffer::new();
        let mut expr_cache = HashMap::with_capacity(self.buf_writable.len());
        // the `last_line` contains any sequence of trailing simple `writer.write_str()` calls
        let mut trailing_simple_lines = Vec::new();

        buf.write("match (");
        while let Some((idx, s)) = it.next() {
            match s {
                Writable::Lit(s) => {
                    let mut items = vec![s];
                    while let Some((_, Writable::Lit(s))) = it.peek() {
                        items.push(s);
                        it.next();
                    }
                    if it.peek().is_some() {
                        for s in items {
                            size_hint += lines.write_writer(s);
                        }
                    } else {
                        trailing_simple_lines = items;
                        break;
                    }
                }
                Writable::Expr(s) => {
                    size_hint += 3;

                    let mut expr_buf = Buffer::new();
                    let expr = match self.visit_expr(ctx, &mut expr_buf, s)? {
                        DisplayWrap::Wrapped => expr_buf.into_string(),
                        DisplayWrap::Unwrapped => format!(
                            "(&&rinja::filters::AutoEscaper::new(&({expr_buf}), {})).\
                                rinja_auto_escape()?",
                            self.input.escaper,
                        ),
                    };
                    let idx = if is_cacheable(s) {
                        match expr_cache.entry(expr) {
                            Entry::Occupied(e) => *e.get(),
                            Entry::Vacant(e) => {
                                buf.write(format_args!("&({}),", e.key()));
                                targets.write(format_args!("expr{idx},"));
                                e.insert(idx);
                                idx
                            }
                        }
                    } else {
                        buf.write(format_args!("&({expr}),"));
                        targets.write(format_args!("expr{idx}, "));
                        idx
                    };
                    lines.write(format_args!(
                        "(&&rinja::filters::Writable(expr{idx})).rinja_write(writer)?;",
                    ));
                }
            }
        }
        buf.write(format_args!(
            ") {{\
                ({targets}) => {{\
                    {lines}\
                }}\
            }}"
        ));

        for s in trailing_simple_lines {
            size_hint += buf.write_writer(s);
        }

        Ok(size_hint)
    }

    fn visit_lit(&mut self, lit: &'a Lit<'_>) {
        assert!(self.next_ws.is_none());
        let Lit { lws, val, rws } = *lit;
        if !lws.is_empty() {
            match self.skip_ws {
                Whitespace::Suppress => {}
                _ if val.is_empty() => {
                    assert!(rws.is_empty());
                    self.next_ws = Some(lws);
                }
                Whitespace::Preserve => {
                    self.buf_writable.push(Writable::Lit(Cow::Borrowed(lws)));
                }
                Whitespace::Minimize => {
                    self.buf_writable.push(Writable::Lit(Cow::Borrowed(
                        match lws.contains('\n') {
                            true => "\n",
                            false => " ",
                        },
                    )));
                }
            }
        }

        if !val.is_empty() {
            self.skip_ws = Whitespace::Preserve;
            self.buf_writable.push(Writable::Lit(Cow::Borrowed(val)));
        }

        if !rws.is_empty() {
            self.next_ws = Some(rws);
        }
    }

    fn write_comment(&mut self, comment: &'a WithSpan<'_, Comment<'_>>) {
        self.handle_ws(comment.ws);
    }

    // Visitor methods for expression types

    fn visit_expr_root(
        &mut self,
        ctx: &Context<'_>,
        expr: &WithSpan<'_, Expr<'_>>,
    ) -> Result<String, CompileError> {
        let mut buf = Buffer::new();
        self.visit_expr(ctx, &mut buf, expr)?;
        Ok(buf.into_string())
    }

    fn visit_expr(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        expr: &WithSpan<'_, Expr<'_>>,
    ) -> Result<DisplayWrap, CompileError> {
        Ok(match **expr {
            Expr::BoolLit(s) => self.visit_bool_lit(buf, s),
            Expr::NumLit(s, _) => self.visit_num_lit(buf, s),
            Expr::StrLit(ref s) => self.visit_str_lit(buf, s),
            Expr::CharLit(ref s) => self.visit_char_lit(buf, s),
            Expr::Var(s) => self.visit_var(buf, s),
            Expr::Path(ref path) => self.visit_path(buf, path),
            Expr::Array(ref elements) => self.visit_array(ctx, buf, elements)?,
            Expr::Attr(ref obj, name) => self.visit_attr(ctx, buf, obj, name)?,
            Expr::Index(ref obj, ref key) => self.visit_index(ctx, buf, obj, key)?,
            Expr::Filter(Filter {
                name,
                ref arguments,
            }) => self.visit_filter(ctx, buf, name, arguments, expr.span())?,
            Expr::Unary(op, ref inner) => self.visit_unary(ctx, buf, op, inner)?,
            Expr::BinOp(op, ref left, ref right) => self.visit_binop(ctx, buf, op, left, right)?,
            Expr::Range(op, ref left, ref right) => {
                self.visit_range(ctx, buf, op, left.as_deref(), right.as_deref())?
            }
            Expr::Group(ref inner) => self.visit_group(ctx, buf, inner)?,
            Expr::Call(ref obj, ref args) => self.visit_call(ctx, buf, obj, args)?,
            Expr::RustMacro(ref path, args) => self.visit_rust_macro(buf, path, args),
            Expr::Try(ref expr) => self.visit_try(ctx, buf, expr)?,
            Expr::Tuple(ref exprs) => self.visit_tuple(ctx, buf, exprs)?,
            Expr::NamedArgument(_, ref expr) => self.visit_named_argument(ctx, buf, expr)?,
            Expr::FilterSource => self.visit_filter_source(buf),
            Expr::IsDefined(var_name) => self.visit_is_defined(buf, true, var_name)?,
            Expr::IsNotDefined(var_name) => self.visit_is_defined(buf, false, var_name)?,
            Expr::As(ref expr, target) => self.visit_as(ctx, buf, expr, target)?,
            Expr::Concat(ref exprs) => self.visit_concat(ctx, buf, exprs)?,
        })
    }

    fn visit_condition(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        expr: &WithSpan<'_, Expr<'_>>,
    ) -> Result<(), CompileError> {
        match &**expr {
            Expr::BoolLit(_) | Expr::IsDefined(_) | Expr::IsNotDefined(_) => {
                self.visit_expr(ctx, buf, expr)?;
            }
            Expr::Unary("!", expr) => {
                buf.write('!');
                self.visit_condition(ctx, buf, expr)?;
            }
            Expr::BinOp(op @ ("&&" | "||"), left, right) => {
                self.visit_condition(ctx, buf, left)?;
                buf.write(format_args!(" {op} "));
                self.visit_condition(ctx, buf, right)?;
            }
            Expr::Group(expr) => {
                buf.write('(');
                self.visit_condition(ctx, buf, expr)?;
                buf.write(')');
            }
            _ => {
                buf.write("rinja::helpers::as_bool(&(");
                self.visit_expr(ctx, buf, expr)?;
                buf.write("))");
            }
        }
        Ok(())
    }

    fn visit_is_defined(
        &mut self,
        buf: &mut Buffer,
        is_defined: bool,
        left: &str,
    ) -> Result<DisplayWrap, CompileError> {
        match (is_defined, self.is_var_defined(left)) {
            (true, true) | (false, false) => buf.write("true"),
            _ => buf.write("false"),
        }
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_as(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        expr: &WithSpan<'_, Expr<'_>>,
        target: &str,
    ) -> Result<DisplayWrap, CompileError> {
        buf.write("rinja::helpers::get_primitive_value(&(");
        self.visit_expr(ctx, buf, expr)?;
        buf.write(format_args!(
            ")) as rinja::helpers::core::primitive::{target}"
        ));
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_concat(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        exprs: &[WithSpan<'_, Expr<'_>>],
    ) -> Result<DisplayWrap, CompileError> {
        match exprs {
            [] => unreachable!(),
            [expr] => self.visit_expr(ctx, buf, expr),
            exprs => {
                let (l, r) = exprs.split_at((exprs.len() + 1) / 2);
                buf.write("rinja::helpers::Concat(&(");
                self.visit_concat(ctx, buf, l)?;
                buf.write("), &(");
                self.visit_concat(ctx, buf, r)?;
                buf.write("))");
                Ok(DisplayWrap::Unwrapped)
            }
        }
    }

    fn visit_try(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        expr: &WithSpan<'_, Expr<'_>>,
    ) -> Result<DisplayWrap, CompileError> {
        buf.write("rinja::helpers::map_try(");
        self.visit_expr(ctx, buf, expr)?;
        buf.write(")?");
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_rust_macro(&mut self, buf: &mut Buffer, path: &[&str], args: &str) -> DisplayWrap {
        self.visit_path(buf, path);
        buf.write("!(");
        buf.write(args);
        buf.write(')');

        DisplayWrap::Unwrapped
    }

    fn visit_filter(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        name: &str,
        args: &[WithSpan<'_, Expr<'_>>],
        node: Span<'_>,
    ) -> Result<DisplayWrap, CompileError> {
        let filter = match name {
            "deref" => Self::_visit_deref_filter,
            "escape" | "e" => Self::_visit_escape_filter,
            "filesizeformat" => Self::_visit_humansize,
            "fmt" => Self::_visit_fmt_filter,
            "format" => Self::_visit_format_filter,
            "join" => Self::_visit_join_filter,
            "json" | "tojson" => Self::_visit_json_filter,
            "linebreaks" | "linebreaksbr" | "paragraphbreaks" => Self::_visit_linebreaks_filter,
            "pluralize" => Self::_visit_pluralize_filter,
            "ref" => Self::_visit_ref_filter,
            "safe" => Self::_visit_safe_filter,
            "urlencode" | "urlencode_strict" => Self::_visit_urlencode,
            name if BUILTIN_FILTERS.contains(&name) => Self::_visit_builtin_filter,
            _ => Self::_visit_custom_filter,
        };
        filter(self, ctx, buf, name, args, node)
    }

    fn _visit_custom_filter(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        name: &str,
        args: &[WithSpan<'_, Expr<'_>>],
        _node: Span<'_>,
    ) -> Result<DisplayWrap, CompileError> {
        buf.write(format_args!("filters::{name}("));
        self._visit_args(ctx, buf, args)?;
        buf.write(")?");
        Ok(DisplayWrap::Unwrapped)
    }

    fn _visit_builtin_filter(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        name: &str,
        args: &[WithSpan<'_, Expr<'_>>],
        _node: Span<'_>,
    ) -> Result<DisplayWrap, CompileError> {
        buf.write(format_args!("rinja::filters::{name}("));
        self._visit_args(ctx, buf, args)?;
        buf.write(")?");
        Ok(DisplayWrap::Unwrapped)
    }

    fn _visit_urlencode(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        name: &str,
        args: &[WithSpan<'_, Expr<'_>>],
        node: Span<'_>,
    ) -> Result<DisplayWrap, CompileError> {
        if cfg!(not(feature = "urlencode")) {
            return Err(ctx.generate_error(
                format_args!("the `{name}` filter requires the `urlencode` feature to be enabled"),
                node,
            ));
        }

        // Both filters return HTML-safe strings.
        buf.write(format_args!(
            "rinja::filters::HtmlSafeOutput(rinja::filters::{name}(",
        ));
        self._visit_args(ctx, buf, args)?;
        buf.write(")?)");
        Ok(DisplayWrap::Unwrapped)
    }

    fn _visit_humansize(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        name: &str,
        args: &[WithSpan<'_, Expr<'_>>],
        _node: Span<'_>,
    ) -> Result<DisplayWrap, CompileError> {
        // All filters return numbers, and any default formatted number is HTML safe.
        buf.write(format_args!(
            "rinja::filters::HtmlSafeOutput(rinja::filters::{name}(\
                 rinja::helpers::get_primitive_value(&("
        ));
        self._visit_args(ctx, buf, args)?;
        buf.write(")) as rinja::helpers::core::primitive::f32)?)");
        Ok(DisplayWrap::Unwrapped)
    }

    fn _visit_pluralize_filter(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        _name: &str,
        args: &[WithSpan<'_, Expr<'_>>],
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
            _ => {
                return Err(
                    ctx.generate_error("unexpected argument(s) in `pluralize` filter", node)
                );
            }
        };
        if let Some(is_singular) = expr_is_int_lit_plus_minus_one(count) {
            let value = if is_singular { sg } else { pl };
            self._visit_auto_escaped_arg(ctx, buf, value)?;
        } else {
            buf.write("rinja::filters::pluralize(");
            self._visit_arg(ctx, buf, count)?;
            for value in [sg, pl] {
                buf.write(',');
                self._visit_auto_escaped_arg(ctx, buf, value)?;
            }
            buf.write(")?");
        }
        Ok(DisplayWrap::Wrapped)
    }

    fn _visit_linebreaks_filter(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        name: &str,
        args: &[WithSpan<'_, Expr<'_>>],
        node: Span<'_>,
    ) -> Result<DisplayWrap, CompileError> {
        if args.len() != 1 {
            return Err(ctx.generate_error(
                format_args!("unexpected argument(s) in `{name}` filter"),
                node,
            ));
        }
        buf.write(format_args!(
            "rinja::filters::{name}(&(&&rinja::filters::AutoEscaper::new(&(",
        ));
        self._visit_args(ctx, buf, args)?;
        // The input is always HTML escaped, regardless of the selected escaper:
        buf.write("), rinja::filters::Html)).rinja_auto_escape()?)?");
        // The output is marked as HTML safe, not safe in all contexts:
        Ok(DisplayWrap::Unwrapped)
    }

    fn _visit_ref_filter(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        _name: &str,
        args: &[WithSpan<'_, Expr<'_>>],
        node: Span<'_>,
    ) -> Result<DisplayWrap, CompileError> {
        let arg = match args {
            [arg] => arg,
            _ => return Err(ctx.generate_error("unexpected argument(s) in `as_ref` filter", node)),
        };
        buf.write('&');
        self.visit_expr(ctx, buf, arg)?;
        Ok(DisplayWrap::Unwrapped)
    }

    fn _visit_deref_filter(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        _name: &str,
        args: &[WithSpan<'_, Expr<'_>>],
        node: Span<'_>,
    ) -> Result<DisplayWrap, CompileError> {
        let arg = match args {
            [arg] => arg,
            _ => return Err(ctx.generate_error("unexpected argument(s) in `deref` filter", node)),
        };
        buf.write('*');
        self.visit_expr(ctx, buf, arg)?;
        Ok(DisplayWrap::Unwrapped)
    }

    fn _visit_json_filter(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        _name: &str,
        args: &[WithSpan<'_, Expr<'_>>],
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
            _ => return Err(ctx.generate_error("unexpected argument(s) in `json` filter", node)),
        };

        buf.write(format_args!("rinja::filters::{filter}("));
        self._visit_args(ctx, buf, args)?;
        buf.write(")?");
        Ok(DisplayWrap::Unwrapped)
    }

    fn _visit_safe_filter(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        _name: &str,
        args: &[WithSpan<'_, Expr<'_>>],
        node: Span<'_>,
    ) -> Result<DisplayWrap, CompileError> {
        if args.len() != 1 {
            return Err(ctx.generate_error("unexpected argument(s) in `safe` filter", node));
        }
        buf.write("rinja::filters::safe(");
        self._visit_args(ctx, buf, args)?;
        buf.write(format_args!(", {})?", self.input.escaper));
        Ok(DisplayWrap::Wrapped)
    }

    fn _visit_escape_filter(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        _name: &str,
        args: &[WithSpan<'_, Expr<'_>>],
        node: Span<'_>,
    ) -> Result<DisplayWrap, CompileError> {
        if args.len() > 2 {
            return Err(ctx.generate_error("only two arguments allowed to escape filter", node));
        }
        let opt_escaper = match args.get(1).map(|expr| &**expr) {
            Some(Expr::StrLit(StrLit { prefix, content })) => {
                if let Some(prefix) = prefix {
                    let kind = if *prefix == StrPrefix::Binary {
                        "slice"
                    } else {
                        "CStr"
                    };
                    return Err(ctx.generate_error(
                        format_args!(
                            "invalid escaper `b{content:?}`. Expected a string, found a {kind}"
                        ),
                        args[1].span(),
                    ));
                }
                Some(content)
            }
            Some(_) => {
                return Err(ctx.generate_error("invalid escaper type for escape filter", node));
            }
            None => None,
        };
        let escaper = match opt_escaper {
            Some(name) => self
                .input
                .config
                .escapers
                .iter()
                .find_map(|(extensions, path)| {
                    extensions
                        .contains(&Cow::Borrowed(name))
                        .then_some(path.as_ref())
                })
                .ok_or_else(|| {
                    ctx.generate_error(
                        format_args!(
                            "invalid escaper '{name}' for `escape` filter. {}",
                            MsgValidEscapers(&self.input.config.escapers),
                        ),
                        node,
                    )
                })?,
            None => self.input.escaper,
        };
        buf.write("rinja::filters::escape(");
        self._visit_args(ctx, buf, &args[..1])?;
        buf.write(format_args!(", {escaper})?"));
        Ok(DisplayWrap::Wrapped)
    }

    fn _visit_format_filter(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        _name: &str,
        args: &[WithSpan<'_, Expr<'_>>],
        node: Span<'_>,
    ) -> Result<DisplayWrap, CompileError> {
        if !args.is_empty() {
            if let Expr::StrLit(ref fmt) = *args[0] {
                buf.write("rinja::helpers::std::format!(");
                self.visit_str_lit(buf, fmt);
                if args.len() > 1 {
                    buf.write(',');
                    self._visit_args(ctx, buf, &args[1..])?;
                }
                buf.write(')');
                return Ok(DisplayWrap::Unwrapped);
            }
        }
        Err(ctx.generate_error(r#"use filter format like `"a={} b={}"|format(a, b)`"#, node))
    }

    fn _visit_fmt_filter(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        _name: &str,
        args: &[WithSpan<'_, Expr<'_>>],
        node: Span<'_>,
    ) -> Result<DisplayWrap, CompileError> {
        if let [_, arg2] = args {
            if let Expr::StrLit(ref fmt) = **arg2 {
                buf.write("rinja::helpers::std::format!(");
                self.visit_str_lit(buf, fmt);
                buf.write(',');
                self._visit_args(ctx, buf, &args[..1])?;
                buf.write(')');
                return Ok(DisplayWrap::Unwrapped);
            }
        }
        Err(ctx.generate_error(r#"use filter fmt like `value|fmt("{:?}")`"#, node))
    }

    // Force type coercion on first argument to `join` filter (see #39).
    fn _visit_join_filter(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        _name: &str,
        args: &[WithSpan<'_, Expr<'_>>],
        _node: Span<'_>,
    ) -> Result<DisplayWrap, CompileError> {
        buf.write("rinja::filters::join((&");
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                buf.write(", &");
            }
            self.visit_expr(ctx, buf, arg)?;
            if i == 0 {
                buf.write(").into_iter()");
            }
        }
        buf.write(")?");
        Ok(DisplayWrap::Unwrapped)
    }

    fn _visit_args(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        args: &[WithSpan<'_, Expr<'_>>],
    ) -> Result<(), CompileError> {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                buf.write(',');
            }
            self._visit_arg(ctx, buf, arg)?;
        }
        Ok(())
    }

    fn _visit_arg(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        arg: &WithSpan<'_, Expr<'_>>,
    ) -> Result<(), CompileError> {
        self._visit_arg_inner(ctx, buf, arg, false)
    }

    fn _visit_arg_inner(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        arg: &WithSpan<'_, Expr<'_>>,
        // This parameter is needed because even though Expr::Unary is not copyable, we might still
        // be able to skip a few levels.
        need_borrow: bool,
    ) -> Result<(), CompileError> {
        if let Expr::Unary(expr @ ("*" | "&"), ref arg) = **arg {
            buf.write(expr);
            return self._visit_arg_inner(ctx, buf, arg, true);
        }
        let borrow = need_borrow || !is_copyable(arg);
        if borrow {
            buf.write("&(");
        }
        match **arg {
            Expr::Call(ref left, _) if !matches!(***left, Expr::Path(_)) => {
                buf.write('{');
                self.visit_expr(ctx, buf, arg)?;
                buf.write('}');
            }
            _ => {
                self.visit_expr(ctx, buf, arg)?;
            }
        }
        if borrow {
            buf.write(')');
        }
        Ok(())
    }

    fn _visit_auto_escaped_arg(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        arg: &WithSpan<'_, Expr<'_>>,
    ) -> Result<(), CompileError> {
        if let Some(Writable::Lit(arg)) = compile_time_escape(arg, self.input.escaper) {
            if !arg.is_empty() {
                buf.write("rinja::filters::Safe(");
                buf.write_escaped_str(&arg);
                buf.write(')');
            } else {
                buf.write("rinja::helpers::Empty");
            }
        } else {
            buf.write("(&&rinja::filters::AutoEscaper::new(");
            self._visit_arg(ctx, buf, arg)?;
            buf.write(format_args!(
                ", {})).rinja_auto_escape()?",
                self.input.escaper
            ));
        }
        Ok(())
    }

    fn visit_attr(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        obj: &WithSpan<'_, Expr<'_>>,
        attr: &str,
    ) -> Result<DisplayWrap, CompileError> {
        if let Expr::Var(name) = **obj {
            if name == "loop" {
                if attr == "index" {
                    buf.write("(_loop_item.index + 1)");
                    return Ok(DisplayWrap::Unwrapped);
                } else if attr == "index0" {
                    buf.write("_loop_item.index");
                    return Ok(DisplayWrap::Unwrapped);
                } else if attr == "first" {
                    buf.write("_loop_item.first");
                    return Ok(DisplayWrap::Unwrapped);
                } else if attr == "last" {
                    buf.write("_loop_item.last");
                    return Ok(DisplayWrap::Unwrapped);
                } else {
                    return Err(ctx.generate_error("unknown loop variable", obj.span()));
                }
            }
        }
        self.visit_expr(ctx, buf, obj)?;
        buf.write(format_args!(".{}", normalize_identifier(attr)));
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_index(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        obj: &WithSpan<'_, Expr<'_>>,
        key: &WithSpan<'_, Expr<'_>>,
    ) -> Result<DisplayWrap, CompileError> {
        buf.write('&');
        self.visit_expr(ctx, buf, obj)?;
        buf.write('[');
        self.visit_expr(ctx, buf, key)?;
        buf.write(']');
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_call(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        left: &WithSpan<'_, Expr<'_>>,
        args: &[WithSpan<'_, Expr<'_>>],
    ) -> Result<DisplayWrap, CompileError> {
        match &**left {
            Expr::Attr(sub_left, method) if ***sub_left == Expr::Var("loop") => match *method {
                "cycle" => match args {
                    [arg] => {
                        if matches!(**arg, Expr::Array(ref arr) if arr.is_empty()) {
                            return Err(ctx.generate_error(
                                "loop.cycle(…) cannot use an empty array",
                                arg.span(),
                            ));
                        }
                        buf.write(
                            "\
                            ({\
                                let _cycle = &(",
                        );
                        self.visit_expr(ctx, buf, arg)?;
                        buf.write(
                            "\
                                );\
                                let _len = _cycle.len();\
                                if _len == 0 {\
                                    return rinja::helpers::core::result::Result::Err(rinja::Error::Fmt);\
                                }\
                                _cycle[_loop_item.index % _len]\
                            })",
                        );
                    }
                    _ => {
                        return Err(ctx.generate_error(
                            "loop.cycle(…) cannot use an empty array",
                            left.span(),
                        ));
                    }
                },
                s => {
                    return Err(
                        ctx.generate_error(format_args!("unknown loop method: {s:?}"), left.span())
                    );
                }
            },
            sub_left => {
                match sub_left {
                    Expr::Var(name) => match self.locals.resolve(name) {
                        Some(resolved) => buf.write(resolved),
                        None => buf.write(format_args!("self.{}", normalize_identifier(name))),
                    },
                    _ => {
                        self.visit_expr(ctx, buf, left)?;
                    }
                }
                buf.write('(');
                self._visit_args(ctx, buf, args)?;
                buf.write(')');
            }
        }
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_unary(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        op: &str,
        inner: &WithSpan<'_, Expr<'_>>,
    ) -> Result<DisplayWrap, CompileError> {
        buf.write(op);
        self.visit_expr(ctx, buf, inner)?;
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_range(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        op: &str,
        left: Option<&WithSpan<'_, Expr<'_>>>,
        right: Option<&WithSpan<'_, Expr<'_>>>,
    ) -> Result<DisplayWrap, CompileError> {
        if let Some(left) = left {
            self.visit_expr(ctx, buf, left)?;
        }
        buf.write(op);
        if let Some(right) = right {
            self.visit_expr(ctx, buf, right)?;
        }
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_binop(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        op: &str,
        left: &WithSpan<'_, Expr<'_>>,
        right: &WithSpan<'_, Expr<'_>>,
    ) -> Result<DisplayWrap, CompileError> {
        self.visit_expr(ctx, buf, left)?;
        buf.write(format_args!(" {op} "));
        self.visit_expr(ctx, buf, right)?;
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_group(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        inner: &WithSpan<'_, Expr<'_>>,
    ) -> Result<DisplayWrap, CompileError> {
        buf.write('(');
        self.visit_expr(ctx, buf, inner)?;
        buf.write(')');
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_tuple(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        exprs: &[WithSpan<'_, Expr<'_>>],
    ) -> Result<DisplayWrap, CompileError> {
        buf.write('(');
        for (index, expr) in exprs.iter().enumerate() {
            if index > 0 {
                buf.write(' ');
            }
            self.visit_expr(ctx, buf, expr)?;
            buf.write(',');
        }
        buf.write(')');
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_named_argument(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        expr: &WithSpan<'_, Expr<'_>>,
    ) -> Result<DisplayWrap, CompileError> {
        self.visit_expr(ctx, buf, expr)?;
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_array(
        &mut self,
        ctx: &Context<'_>,
        buf: &mut Buffer,
        elements: &[WithSpan<'_, Expr<'_>>],
    ) -> Result<DisplayWrap, CompileError> {
        buf.write('[');
        for (i, el) in elements.iter().enumerate() {
            if i > 0 {
                buf.write(',');
            }
            self.visit_expr(ctx, buf, el)?;
        }
        buf.write(']');
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_path(&mut self, buf: &mut Buffer, path: &[&str]) -> DisplayWrap {
        for (i, part) in path.iter().enumerate() {
            if i > 0 {
                buf.write("::");
            }
            buf.write(part);
        }
        DisplayWrap::Unwrapped
    }

    fn visit_var(&mut self, buf: &mut Buffer, s: &str) -> DisplayWrap {
        if s == "self" {
            buf.write(s);
            return DisplayWrap::Unwrapped;
        }

        buf.write(normalize_identifier(&self.locals.resolve_or_self(s)));
        DisplayWrap::Unwrapped
    }

    fn visit_filter_source(&mut self, buf: &mut Buffer) -> DisplayWrap {
        // We can assume that the body of the `{% filter %}` was already escaped.
        // And if it's not, then this was done intentionally.
        buf.write(format_args!("rinja::filters::Safe(&{FILTER_SOURCE})"));
        DisplayWrap::Wrapped
    }

    fn visit_bool_lit(&mut self, buf: &mut Buffer, s: bool) -> DisplayWrap {
        if s {
            buf.write("true");
        } else {
            buf.write("false");
        }
        DisplayWrap::Unwrapped
    }

    fn visit_str_lit(&mut self, buf: &mut Buffer, s: &StrLit<'_>) -> DisplayWrap {
        if let Some(prefix) = s.prefix {
            buf.write(prefix.to_char());
        }
        buf.write(format_args!("\"{}\"", s.content));
        DisplayWrap::Unwrapped
    }

    fn visit_char_lit(&mut self, buf: &mut Buffer, c: &CharLit<'_>) -> DisplayWrap {
        if c.prefix == Some(CharPrefix::Binary) {
            buf.write('b');
        }
        buf.write(format_args!("'{}'", c.content));
        DisplayWrap::Unwrapped
    }

    fn visit_num_lit(&mut self, buf: &mut Buffer, s: &str) -> DisplayWrap {
        buf.write(s);
        DisplayWrap::Unwrapped
    }

    fn visit_target(
        &mut self,
        buf: &mut Buffer,
        initialized: bool,
        first_level: bool,
        target: &Target<'a>,
    ) {
        match target {
            Target::Placeholder(_) => buf.write('_'),
            Target::Rest(s) => {
                if let Some(var_name) = &**s {
                    self.locals
                        .insert(Cow::Borrowed(var_name), LocalMeta::initialized());
                    buf.write(var_name);
                    buf.write(" @ ");
                }
                buf.write("..");
            }
            Target::Name(name) => {
                let name = normalize_identifier(name);
                match initialized {
                    true => self
                        .locals
                        .insert(Cow::Borrowed(name), LocalMeta::initialized()),
                    false => self.locals.insert_with_default(Cow::Borrowed(name)),
                }
                buf.write(name);
            }
            Target::OrChain(targets) => match targets.first() {
                None => buf.write('_'),
                Some(first_target) => {
                    self.visit_target(buf, initialized, first_level, first_target);
                    for target in &targets[1..] {
                        buf.write('|');
                        self.visit_target(buf, initialized, first_level, target);
                    }
                }
            },
            Target::Tuple(path, targets) => {
                buf.write_separated_path(path);
                buf.write('(');
                for target in targets {
                    self.visit_target(buf, initialized, false, target);
                    buf.write(',');
                }
                buf.write(')');
            }
            Target::Array(path, targets) => {
                buf.write_separated_path(path);
                buf.write('[');
                for target in targets {
                    self.visit_target(buf, initialized, false, target);
                    buf.write(',');
                }
                buf.write(']');
            }
            Target::Struct(path, targets) => {
                buf.write_separated_path(path);
                buf.write('{');
                for (name, target) in targets {
                    if let Target::Rest(_) = target {
                        buf.write("..");
                        continue;
                    }

                    buf.write(normalize_identifier(name));
                    buf.write(": ");
                    self.visit_target(buf, initialized, false, target);
                    buf.write(',');
                }
                buf.write('}');
            }
            Target::Path(path) => {
                self.visit_path(buf, path);
                buf.write("{}");
            }
            Target::StrLit(s) => {
                if first_level {
                    buf.write('&');
                }
                self.visit_str_lit(buf, s);
            }
            Target::NumLit(s, _) => {
                if first_level {
                    buf.write('&');
                }
                self.visit_num_lit(buf, s);
            }
            Target::CharLit(s) => {
                if first_level {
                    buf.write('&');
                }
                self.visit_char_lit(buf, s);
            }
            Target::BoolLit(s) => {
                if first_level {
                    buf.write('&');
                }
                buf.write(s);
            }
        }
    }

    // Helper methods for dealing with whitespace nodes

    // Combines `flush_ws()` and `prepare_ws()` to handle both trailing whitespace from the
    // preceding literal and leading whitespace from the succeeding literal.
    fn handle_ws(&mut self, ws: Ws) {
        self.flush_ws(ws);
        self.prepare_ws(ws);
    }

    fn should_trim_ws(&self, ws: Option<Whitespace>) -> Whitespace {
        ws.unwrap_or(self.input.config.whitespace)
    }

    // If the previous literal left some trailing whitespace in `next_ws` and the
    // prefix whitespace suppressor from the given argument, flush that whitespace.
    // In either case, `next_ws` is reset to `None` (no trailing whitespace).
    fn flush_ws(&mut self, ws: Ws) {
        if self.next_ws.is_none() {
            return;
        }

        // If `whitespace` is set to `suppress`, we keep the whitespace characters only if there is
        // a `+` character.
        match self.should_trim_ws(ws.0) {
            Whitespace::Preserve => {
                let val = self.next_ws.unwrap();
                if !val.is_empty() {
                    self.buf_writable.push(Writable::Lit(Cow::Borrowed(val)));
                }
            }
            Whitespace::Minimize => {
                let val = self.next_ws.unwrap();
                if !val.is_empty() {
                    self.buf_writable.push(Writable::Lit(Cow::Borrowed(
                        match val.contains('\n') {
                            true => "\n",
                            false => " ",
                        },
                    )));
                }
            }
            Whitespace::Suppress => {}
        }
        self.next_ws = None;
    }

    // Sets `skip_ws` to match the suffix whitespace suppressor from the given
    // argument, to determine whether to suppress leading whitespace from the
    // next literal.
    fn prepare_ws(&mut self, ws: Ws) {
        self.skip_ws = self.should_trim_ws(ws.1);
    }
}

fn macro_call_ensure_arg_count(
    call: &WithSpan<'_, Call<'_>>,
    def: &Macro<'_>,
    ctx: &Context<'_>,
) -> Result<(), CompileError> {
    if call.args.len() == def.args.len() {
        // exactly enough arguments were provided
        return Ok(());
    }

    let nb_default_args = def
        .args
        .iter()
        .rev()
        .take_while(|(_, default_value)| default_value.is_some())
        .count();
    if call.args.len() < def.args.len() && call.args.len() >= def.args.len() - nb_default_args {
        // all missing arguments have a default value, and there weren't too many args
        return Ok(());
    }

    // either too many or not enough arguments were provided
    let (expected_args, extra) = match nb_default_args {
        0 => (def.args.len(), ""),
        _ => (nb_default_args, "at least "),
    };
    Err(ctx.generate_error(
        format_args!(
            "macro {:?} expected {extra}{expected_args} argument{}, found {}",
            def.name,
            if expected_args != 1 { "s" } else { "" },
            call.args.len(),
        ),
        call.span(),
    ))
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

struct CondInfo<'a> {
    cond: &'a WithSpan<'a, Cond<'a>>,
    cond_expr: Option<WithSpan<'a, Expr<'a>>>,
    generate_condition: bool,
    generate_content: bool,
}

struct Conds<'a> {
    conds: Vec<CondInfo<'a>>,
    ws_before: Option<Ws>,
    ws_after: Option<Ws>,
    nb_conds: usize,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum EvaluatedResult {
    AlwaysTrue,
    AlwaysFalse,
    Unknown,
}

impl<'a> Conds<'a> {
    fn compute_branches(generator: &Generator<'a, '_>, i: &'a If<'a>) -> Self {
        let mut conds = Vec::with_capacity(i.branches.len());
        let mut ws_before = None;
        let mut ws_after = None;
        let mut nb_conds = 0;
        let mut stop_loop = false;

        for cond in &i.branches {
            if stop_loop {
                ws_after = Some(cond.ws);
                break;
            }
            if let Some(CondTest {
                expr,
                contains_bool_lit_or_is_defined,
                ..
            }) = &cond.cond
            {
                let mut only_contains_is_defined = true;

                let (evaluated_result, cond_expr) = if *contains_bool_lit_or_is_defined {
                    let (evaluated_result, expr) =
                        generator.evaluate_condition(expr.clone(), &mut only_contains_is_defined);
                    (evaluated_result, Some(expr))
                } else {
                    (EvaluatedResult::Unknown, None)
                };

                match evaluated_result {
                    // We generate the condition in case some calls are changing a variable, but
                    // no need to generate the condition body since it will never be called.
                    //
                    // However, if the condition only contains "is (not) defined" checks, then we
                    // can completely skip it.
                    EvaluatedResult::AlwaysFalse => {
                        if only_contains_is_defined {
                            if conds.is_empty() && ws_before.is_none() {
                                // If this is the first `if` and it's skipped, we definitely don't
                                // want its whitespace control to be lost.
                                ws_before = Some(cond.ws);
                            }
                            continue;
                        }
                        nb_conds += 1;
                        conds.push(CondInfo {
                            cond,
                            cond_expr,
                            generate_condition: true,
                            generate_content: false,
                        });
                    }
                    // This case is more interesting: it means that we will always enter this
                    // condition, meaning that any following should not be generated. Another
                    // thing to take into account: if there are no if branches before this one,
                    // no need to generate an `else`.
                    EvaluatedResult::AlwaysTrue => {
                        let generate_condition = !only_contains_is_defined;
                        if generate_condition {
                            nb_conds += 1;
                        }
                        conds.push(CondInfo {
                            cond,
                            cond_expr,
                            generate_condition,
                            generate_content: true,
                        });
                        // Since it's always true, we can stop here.
                        stop_loop = true;
                    }
                    EvaluatedResult::Unknown => {
                        nb_conds += 1;
                        conds.push(CondInfo {
                            cond,
                            cond_expr,
                            generate_condition: true,
                            generate_content: true,
                        });
                    }
                }
            } else {
                let generate_condition = !conds.is_empty();
                if generate_condition {
                    nb_conds += 1;
                }
                conds.push(CondInfo {
                    cond,
                    cond_expr: None,
                    generate_condition,
                    generate_content: true,
                });
            }
        }
        Self {
            conds,
            ws_before,
            ws_after,
            nb_conds,
        }
    }
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
        Expr::Call(..) | Expr::Path(..) | Expr::Filter(..) | Expr::RustMacro(..) => true,
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

/// Returns `true` if the outcome of this expression may be used multiple times in the same
/// `write!()` call, without evaluating the expression again, i.e. the expression should be
/// side-effect free.
fn is_cacheable(expr: &WithSpan<'_, Expr<'_>>) -> bool {
    match &**expr {
        // Literals are the definition of pure:
        Expr::BoolLit(_) => true,
        Expr::NumLit(_, _) => true,
        Expr::StrLit(_) => true,
        Expr::CharLit(_) => true,
        // fmt::Display should have no effects:
        Expr::Var(_) => true,
        Expr::Path(_) => true,
        // Check recursively:
        Expr::Array(args) => args.iter().all(is_cacheable),
        Expr::Attr(lhs, _) => is_cacheable(lhs),
        Expr::Index(lhs, rhs) => is_cacheable(lhs) && is_cacheable(rhs),
        Expr::Filter(Filter { arguments, .. }) => arguments.iter().all(is_cacheable),
        Expr::Unary(_, arg) => is_cacheable(arg),
        Expr::BinOp(_, lhs, rhs) => is_cacheable(lhs) && is_cacheable(rhs),
        Expr::IsDefined(_) | Expr::IsNotDefined(_) => true,
        Expr::Range(_, lhs, rhs) => {
            lhs.as_ref().map_or(true, |v| is_cacheable(v))
                && rhs.as_ref().map_or(true, |v| is_cacheable(v))
        }
        Expr::Group(arg) => is_cacheable(arg),
        Expr::Tuple(args) => args.iter().all(is_cacheable),
        Expr::NamedArgument(_, expr) => is_cacheable(expr),
        Expr::As(expr, _) => is_cacheable(expr),
        Expr::Try(expr) => is_cacheable(expr),
        Expr::Concat(args) => args.iter().all(is_cacheable),
        // We have too little information to tell if the expression is pure:
        Expr::Call(_, _) => false,
        Expr::RustMacro(_, _) => false,
        // Should never be encountered:
        Expr::FilterSource => unreachable!("FilterSource in expression?"),
    }
}

const FILTER_SOURCE: &str = "__rinja_filter_block";

fn median(sizes: &mut [usize]) -> usize {
    if sizes.is_empty() {
        return 0;
    }
    sizes.sort_unstable();
    if sizes.len() % 2 == 1 {
        sizes[sizes.len() / 2]
    } else {
        (sizes[sizes.len() / 2 - 1] + sizes[sizes.len() / 2]) / 2
    }
}

#[derive(Clone, Copy, PartialEq)]
enum AstLevel {
    Top,
    Block,
    Nested,
}

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

    if ident.len() > parser::node::MAX_KW_LEN {
        return ident;
    }
    let kws = parser::node::KWS[ident.len()];

    let mut padded_ident = [b'_'; parser::node::MAX_KW_LEN];
    padded_ident[..ident.len()].copy_from_slice(ident.as_bytes());

    // Since the individual buckets are quite short, a linear search is faster than a binary search.
    let replacement = match kws.iter().find(|probe| {
        padded_ident == <[u8; parser::node::MAX_KW_LEN]>::try_from(&probe[2..]).unwrap()
    }) {
        Some(replacement) => replacement,
        None => return ident,
    };

    // SAFETY: We know that the input byte slice is pure-ASCII.
    unsafe { std::str::from_utf8_unchecked(&replacement[..ident.len() + 2]) }
}
