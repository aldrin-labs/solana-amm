use amm::math::stable_curve_invariant::compute;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_two_reserves(c: &mut Criterion) {
    let reserves =
        vec![8_374_498_120_012_u64.into(), 8_329_984_930_238_u64.into()];
    let amp = 4;

    // [1.4986 us 1.5056 us 1.5150 us]
    c.bench_function("amm::math::stable_curve_invariant::compute", |b| {
        b.iter(|| {
            compute(black_box(amp), black_box(&reserves)).unwrap();
        })
    });
}

fn bench_three_reserves(c: &mut Criterion) {
    let reserves = vec![
        8_374_498_120_012_u64.into(),
        8_329_984_930_238_u64.into(),
        8_338_259_921_130_u64.into(),
    ];
    let amp = 4;

    // [1.6640 us 1.6723 us 1.6802 us]
    c.bench_function("amm::math::stable_curve_invariant::compute", |b| {
        b.iter(|| {
            compute(black_box(amp), black_box(&reserves)).unwrap();
        })
    });
}

criterion_group!(benches, bench_two_reserves, bench_three_reserves);
criterion_main!(benches);
