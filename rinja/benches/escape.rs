use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rinja::filters::{escape, Html};

criterion_main!(benches);
criterion_group!(benches, functions);

fn functions(c: &mut Criterion) {
    c.bench_function("Escaping", escaping);
}

fn escaping(b: &mut criterion::Bencher<'_>) {
    b.iter(|| {
        for &s in black_box(STRINGS) {
            format!("{}", escape(s, Html).unwrap());
        }
    });
}

const STRINGS: &[&str] = include!("strings.inc");
