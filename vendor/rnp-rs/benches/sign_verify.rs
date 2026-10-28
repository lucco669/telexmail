//! Benchmark: sign + verify an inline-signed message.

use criterion::{Criterion, criterion_group, criterion_main};
use rnp::{Algorithm, Context, Hash, KeyBuilder, KeyUsage, sign, verify};

fn bench_sign_verify(c: &mut Criterion) {
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("bench <bench@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::Sign)
        .build(&ctx)
        .expect("key");
    let message = vec![0x42u8; 4096];

    c.bench_function("sign_4k_inline", |b| {
        b.iter(|| sign(&ctx, &message, &key).unwrap());
    });

    let signed = sign(&ctx, &message, &key).unwrap();
    c.bench_function("verify_4k_inline", |b| {
        b.iter(|| verify(&ctx, &signed).unwrap());
    });
}

criterion_group!(benches, bench_sign_verify);
criterion_main!(benches);
