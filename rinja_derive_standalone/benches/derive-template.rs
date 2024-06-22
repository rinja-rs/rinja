use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use quote::quote;

criterion_main!(benches);
criterion_group!(benches, hello_world, librustdoc);

fn hello_world(c: &mut Criterion) {
    let ts = quote! {
        #[derive(Template)]
        #[template(
            source = "<html><body><h1>Hello, {{user}}!</h1></body></html>",
            ext = "html"
        )]
        struct Hello<'a> {
            user: &'a str,
        }
    };
    c.bench_function("hello_world", |b| {
        b.iter_batched(
            || ts.clone(),
            |ts| rinja_derive_standalone::derive_template2(ts),
            BatchSize::LargeInput,
        )
    });
}

fn librustdoc(c: &mut Criterion) {
    // ///////////////////////////////////////////////////////////////////////////////////////////
    // item_info.html
    // ///////////////////////////////////////////////////////////////////////////////////////////

    let source = include_str!("../../rinja_parser/benches/librustdoc/item_info.html");
    let ts = quote! {
        #[derive(Template)]
        #[template(source = #source, ext = "html")]
        struct ItemInfo {
            items: Vec<ShortItemInfo>,
        }
    };
    c.bench_function("item_info.html", |b| {
        b.iter_batched(
            || ts.clone(),
            |ts| rinja_derive_standalone::derive_template2(ts),
            BatchSize::LargeInput,
        )
    });

    // ///////////////////////////////////////////////////////////////////////////////////////////
    // item_union.html
    // ///////////////////////////////////////////////////////////////////////////////////////////

    let source = include_str!("../../rinja_parser/benches/librustdoc/item_union.html");
    let ts = quote! {
        #[derive(Template)]
        #[template(source = #source, ext = "html")]
        struct ItemUnion<'a, 'cx> {
            cx: RefCell<&'a mut Context<'cx>>,
            it: &'a clean::Item,
            s: &'a clean::Union,
        }
    };
    c.bench_function("item_union.html", |b| {
        b.iter_batched(
            || ts.clone(),
            |ts| rinja_derive_standalone::derive_template2(ts),
            BatchSize::LargeInput,
        )
    });

    // ///////////////////////////////////////////////////////////////////////////////////////////
    // page.html
    // ///////////////////////////////////////////////////////////////////////////////////////////

    let source = include_str!("../../rinja_parser/benches/librustdoc/page.html");
    let ts = quote! {
        #[derive(Template)]
        #[template(source = #source, ext = "html")]
        struct PageLayout<'a> {
            static_root_path: String,
            page: &'a Page<'a>,
            layout: &'a Layout,
            files: &'static StaticFiles,
            themes: Vec<String>,
            sidebar: String,
            content: String,
            rust_channel: &'static str,
            pub(crate) rustdoc_version: &'a str,
            display_krate: &'a str,
            display_krate_with_trailing_slash: String,
            display_krate_version_number: &'a str,
            display_krate_version_extra: &'a str,
        }
    };
    c.bench_function("page.html", |b| {
        b.iter_batched(
            || ts.clone(),
            |ts| rinja_derive_standalone::derive_template2(ts),
            BatchSize::LargeInput,
        )
    });

    // ///////////////////////////////////////////////////////////////////////////////////////////
    // print_item.html
    // ///////////////////////////////////////////////////////////////////////////////////////////

    let source = include_str!("../../rinja_parser/benches/librustdoc/print_item.html");
    let ts = quote! {
        #[derive(Template)]
        #[template(source = #source, ext = "html")]
        struct ItemVars<'a> {
            typ: &'a str,
            name: &'a str,
            item_type: &'a str,
            path_components: Vec<PathComponent>,
            stability_since_raw: &'a str,
            src_href: Option<&'a str>,
        }
    };
    c.bench_function("print_item.html", |b| {
        b.iter_batched(
            || ts.clone(),
            |ts| rinja_derive_standalone::derive_template2(ts),
            BatchSize::LargeInput,
        )
    });

    // ///////////////////////////////////////////////////////////////////////////////////////////
    // short_item_info.html
    // ///////////////////////////////////////////////////////////////////////////////////////////

    let source = include_str!("../../rinja_parser/benches/librustdoc/short_item_info.html");
    let ts = quote! {
        #[derive(Template)]
        #[template(source = #source, ext = "html")]
        enum ShortItemInfo {
            /// A message describing the deprecation of this item
            Deprecation {
                message: String,
            },
            /// The feature corresponding to an unstable item, and optionally
            /// a tracking issue URL and number.
            Unstable {
                feature: String,
                tracking: Option<(String, u32)>,
            },
            Portability {
                message: String,
            },
        }
    };
    c.bench_function("sidebar.html", |b| {
        b.iter_batched(
            || ts.clone(),
            |ts| rinja_derive_standalone::derive_template2(ts),
            BatchSize::LargeInput,
        )
    });

    // ///////////////////////////////////////////////////////////////////////////////////////////
    // sidebar.html
    // ///////////////////////////////////////////////////////////////////////////////////////////

    let source = include_str!("../../rinja_parser/benches/librustdoc/sidebar.html");
    let ts = quote! {
        #[derive(Template)]
        #[template(source = #source, ext = "html")]
        pub(super) struct Sidebar<'a> {
            pub(super) title_prefix: &'static str,
            pub(super) title: &'a str,
            pub(super) is_crate: bool,
            pub(super) is_mod: bool,
            pub(super) blocks: Vec<LinkBlock<'a>>,
            pub(super) path: String,
        }
    };
    c.bench_function("sidebar.html", |b| {
        b.iter_batched(
            || ts.clone(),
            |ts| rinja_derive_standalone::derive_template2(ts),
            BatchSize::LargeInput,
        )
    });

    // ///////////////////////////////////////////////////////////////////////////////////////////
    // source.html
    // ///////////////////////////////////////////////////////////////////////////////////////////

    let source = include_str!("../../rinja_parser/benches/librustdoc/source.html");
    let ts = quote! {
        #[derive(Template)]
        #[template(source = #source, ext = "html")]
        struct Source<Code: std::fmt::Display> {
            embedded: bool,
            needs_expansion: bool,
            lines: RangeInclusive<usize>,
            code_html: Code,
        }
    };
    c.bench_function("source.html", |b| {
        b.iter_batched(
            || ts.clone(),
            |ts| rinja_derive_standalone::derive_template2(ts),
            BatchSize::LargeInput,
        )
    });

    // ///////////////////////////////////////////////////////////////////////////////////////////
    // type_layout.html
    // ///////////////////////////////////////////////////////////////////////////////////////////

    let source = include_str!("../../rinja_parser/benches/librustdoc/type_layout.html");
    let ts = quote! {
        #[derive(Template)]
        #[template(source = #source, ext = "html")]
        struct TypeLayout<'cx> {
            variants: Vec<(Symbol, TypeLayoutSize)>,
            type_layout_size: Result<TypeLayoutSize, &'cx LayoutError<'cx>>,
        }
    };
    c.bench_function("type_layout.html", |b| {
        b.iter_batched(
            || ts.clone(),
            |ts| rinja_derive_standalone::derive_template2(ts),
            BatchSize::LargeInput,
        )
    });

    // ///////////////////////////////////////////////////////////////////////////////////////////
    // type_layout_size.html
    // ///////////////////////////////////////////////////////////////////////////////////////////

    let source = include_str!("../../rinja_parser/benches/librustdoc/type_layout_size.html");
    let ts = quote! {
        #[derive(Template)]
        #[template(source = #source, ext = "html")]
        struct TypeLayoutSize {
            is_unsized: bool,
            is_uninhabited: bool,
            size: u64,
        }
    };
    c.bench_function("type_layout_size.html", |b| {
        b.iter_batched(
            || ts.clone(),
            |ts| rinja_derive_standalone::derive_template2(ts),
            BatchSize::LargeInput,
        )
    });
}
