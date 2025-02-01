use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use quote::quote;

criterion_main!(benches);
criterion_group!(
    benches,
    noop,
    no_filters,
    few_filters,
    all_filters,
    some_filters_twice,
);

fn noop(c: &mut Criterion) {
    let ts = quote! {
        #[derive(Template)]
        #[template(source = "", ext = "html")]
        struct Hello;
    };
    c.bench_function("noop", |b| {
        b.iter_batched(
            || ts.clone(),
            rinja_derive_standalone::derive_template,
            BatchSize::LargeInput,
        );
    });
}

fn no_filters(c: &mut Criterion) {
    let ts = quote! {
        #[derive(Template)]
        #[template(
            source = "\
                Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod \
                tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua. At \
                vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, \
                no sea takimata sanctus est Lorem ipsum dolor sit amet.\
            ",
            ext = "html"
        )]
        struct Hello;
    };
    c.bench_function("no_filters", |b| {
        b.iter_batched(
            || ts.clone(),
            rinja_derive_standalone::derive_template,
            BatchSize::LargeInput,
        );
    });
}

fn few_filters(c: &mut Criterion) {
    let ts = quote! {
        #[derive(Template)]
        #[template(
            source = "\
                Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod \
                tempor invidunt ut labore et dolore {{ user | upper }} erat, sed diam voluptua. At \
                vero eos et accusam et justo duo dolores et ea rebum. Stet clita kasd gubergren, \
                no sea takimata sanctus est {{ user | uppercase | safe }} ipsum dolor sit amet.\
            ",
            ext = "html"
        )]
        struct Hello<'a> {
            user: &'a str
        }
    };
    c.bench_function("few_filters", |b| {
        b.iter_batched(
            || ts.clone(),
            rinja_derive_standalone::derive_template,
            BatchSize::LargeInput,
        );
    });
}

fn all_filters(c: &mut Criterion) {
    let ts = quote! {
        #[derive(Template)]
        #[template(
            source = "\
                Lorem ipsum dolor sit amet, consetetur sadipscing elitr, {{ user | capitalize }} \
                sed diam nonumy eirmod tempor invidunt ut labore et dolore {{ user | center(10) }} \
                magna aliquyam erat, sed diam voluptua. At vero eos et accusam {{ user | deref }} \
                et justo duo dolores et ea rebum. Stet clita kasd gubergren, {{ user | escape }} \
                no  sea takimata sanctus est Lorem ipsum dolor sit amet. Lorem {{ user | e }} \
                ipsum dolor sit amet, consetetur sadipscing elitr, sed {{ user | filesizeformat }} \
                diam nonumy eirmod tempor invidunt ut labore et dolore {{ user | fmt(\":?\") }} \
                magna aliquyam erat, sed diam voluptua. At vero eos {{ \"{:?}\" | format(user) }} \
                et accusam et justo duo dolores et ea rebum. Stet clita {{ user | indent(10) }} \
                kasd gubergren, no sea takimata sanctus {{ [user, user, user] | join(\", \") }} \
                est orem  ipsum dolor sit amet. Lorem ipsum dolor sit {{ user | linebreaks }} \
                amet, consetetur sadipscing elitr, sed diam nonumy {{ user | linebreaksbr }} \
                eirmod tempor invidunt ut labore et dolore magna {{ user | paragraphbreaks }} \
                aliquyam erat, sed diam voluptua. At vero eos et accusam et {{ user | lower }} \
                justo duo dolores et ea rebum. Stet clita kasd gubergren, {{ user | lowercase }} \
                no sea takimata sanctus est Lorem ipsum dolor sit amet {{ user | pluralize}}.\n\
                \n\
                Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam {{ user | ref }} \
                nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam {{ user | safe }} \
                erat, sed diam voluptua. At vero eos et accusam et justo duo {{ user | title }} \
                dolores et ea rebum. Stet clita kasd gubergren, no sea takimata {{ user | trim }} \
                sanctus est Lorem ipsum dolor sit amet. Lorem ipsum {{ user | truncate(10) }} \
                dolor sit amet, consetetur sadipscing elitr, sed diam nonumy {{ user | upper }} \
                eirmod tempor invidunt ut labore et dolore magna aliquyam {{ user | uppercase }} \
                erat, sed diam voluptua. At vero eos et accusam et justo {{ user | urlencode }} \
                duo dolores et ea rebum. Stet clita kasd gubergren, {{ user | urlencode_strict }} \
                no sea takimata sanctus est Lorem ipsum {{ [user, user, user] | join(\", \") }} \
                dolor  sit amet. Lorem ipsum dolor sit amet, consetetur {{ user | wordcount }} \
                sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut {{ user | custom }} \
                labore et dolore magna aliquyam erat, sed diam {{ user | also_custom(42) }} \
                voluptua. At vero eos et accusam et justo duo {{ user | even::<More>(custom) }} \
                dolores et ea rebum. Stet clita kasd gubergren, no sea takimata sanctus est \
                Lorem ipsum dolor sit amet.\
            ",
            ext = "html"
        )]
        struct Hello<'a> {
            user: &'a str
        }
    };
    c.bench_function("all_filters", |b| {
        b.iter_batched(
            || ts.clone(),
            rinja_derive_standalone::derive_template,
            BatchSize::LargeInput,
        );
    });
}

fn some_filters_twice(c: &mut Criterion) {
    let ts = quote! {
        #[derive(Template)]
        #[template(
            source = "\
                Lorem ipsum dolor sit amet, consetetur {{ user | capitalize | center(10) }} \
                sadipscing elitr, sed diam nonumy eirmod tempor {{ user | center(10) | deref }} \
                invidunt ut labore et dolore  magna aliquyam erat, {{ user | deref | escape }} \
                sed diam voluptua. At vero eos et accusam et justo duo {{ user | escape | e }} \
                dolores et ea rebum. Stet clita kasd gubergren, {{ user | e | filesizeformat }} \
                no  sea takimata sanctus est Lorem ipsum {{ user | filesizeformat | fmt(\":?\") }}\
                Lorem ipsum dolor sit amet.\
            ",
            ext = "html"
        )]
        struct Hello<'a> {
            user: &'a str
        }
    };
    c.bench_function("some_filters_twice", |b| {
        b.iter_batched(
            || ts.clone(),
            rinja_derive_standalone::derive_template,
            BatchSize::LargeInput,
        );
    });
}
