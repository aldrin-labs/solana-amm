use amm::math::stable_curve_invariant::compute;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench(c: &mut Criterion) {
    let reserves = vec![8_374u64.into(), 7_329u64.into()];
    let amp = 4;

    c.bench_function("amm::math::stable_curve_invariant::compute", |b| {
        b.iter(|| {
            compute(black_box(amp), black_box(&reserves)).unwrap();
        })
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
