use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use lb_core::{Buffer, Edit};

fn bench_insert(c: &mut Criterion) {
    let sizes = [
        ("3KB",   "fn main() {\n    println!(\"hello\");\n}\n".repeat(100)),
        ("300KB",  "fn main() {\n    println!(\"hello\");\n}\n".repeat(10_000)),
    ];

    let mut group = c.benchmark_group("insert_char_at_mid");
    for (label, content) in &sizes {
        group.bench_with_input(BenchmarkId::from_parameter(label), content, |b, text| {
            b.iter(|| {
                let mut buf = Buffer::from_str(text);
                let mid = buf.len_bytes() / 2;
                let _ = black_box(buf.apply(&Edit::insert(mid, "x")));
            });
        });
    }
    group.finish();
}

fn bench_delete(c: &mut Criterion) {
    let content = "abcdefghij\n".repeat(10_000);
    c.bench_function("delete_10_chars_at_mid", |b| {
        b.iter(|| {
            let mut buf = Buffer::from_str(&content);
            let mid = buf.len_bytes() / 2;
            let _ = black_box(buf.apply(&Edit::delete(mid, mid + 10)));
        });
    });
}

fn bench_load(c: &mut Criterion) {
    let content = "fn main() { }\n".repeat(50_000); // ~700 KB
    c.bench_function("load_700kb_buffer", |b| {
        b.iter(|| {
            black_box(Buffer::from_str(&content));
        });
    });
}

criterion_group!(benches, bench_insert, bench_delete, bench_load);
criterion_main!(benches);
