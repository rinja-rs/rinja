use criterion::{black_box, criterion_group, criterion_main, Criterion};
use quote::quote;

criterion_main!(benches);
criterion_group!(benches, functions);

fn functions(c: &mut Criterion) {
    c.bench_function("hello_world", hello_world);
}

fn hello_world(b: &mut criterion::Bencher<'_>) {
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
    b.iter(|| {
        rinja_derive_standalone::derive_template2(black_box(&ts).clone());
    })
}
