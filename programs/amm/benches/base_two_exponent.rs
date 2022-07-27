use amm::math::helpers::base_two_exponent;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use decimal::Decimal;

fn bench_base_two_exponent(c: &mut Criterion) {
    c.bench_function("amm::math::helpers::base_two_exponent", |b| {
        b.iter(|| {
            base_two_exponent(black_box(Decimal::from(18_238_614u64)));
        })
    });
}

criterion_group!(benches, bench_base_two_exponent);
criterion_main!(benches);
