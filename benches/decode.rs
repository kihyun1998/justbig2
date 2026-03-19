use criterion::{black_box, criterion_group, criterion_main, Criterion};
use justbig2::Decoder;

const ANNEX_H: &[u8] = include_bytes!("../vendor/jbig2dec/annex-h.jbig2");

fn bench_decode_annex_h(c: &mut Criterion) {
    c.bench_function("decode_annex_h", |b| {
        b.iter(|| {
            let mut dec = Decoder::new();
            dec.write(black_box(ANNEX_H)).unwrap();
            let page = dec.page().unwrap();
            black_box(page);
        });
    });
}

fn bench_decode_annex_h_incremental(c: &mut Criterion) {
    c.bench_function("decode_annex_h_byte_by_byte", |b| {
        b.iter(|| {
            let mut dec = Decoder::new();
            for &byte in black_box(ANNEX_H) {
                dec.write(&[byte]).unwrap();
            }
            let page = dec.page().unwrap();
            black_box(page);
        });
    });
}

criterion_group!(benches, bench_decode_annex_h, bench_decode_annex_h_incremental);
criterion_main!(benches);
