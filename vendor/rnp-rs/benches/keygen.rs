//! Benchmark: RSA-2048 keypair generation.

use criterion::{Criterion, criterion_group, criterion_main};
use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage};

fn bench_keygen(c: &mut Criterion) {
    let ctx = Context::new().expect("ctx");

    c.bench_function("keygen_rsa_2048", |b| {
        b.iter(|| {
            KeyBuilder::new(Algorithm::Rsa)
                .bits(2048)
                .userid("bench <bench@example.com>")
                .hash(Hash::Sha256)
                .add_usage(KeyUsage::Sign)
                .add_usage(KeyUsage::Certify)
                .build(&ctx)
                .unwrap()
        });
    });
}

criterion_group!(benches, bench_keygen);
criterion_main!(benches);
