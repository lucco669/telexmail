//! Benchmark: encrypt + decrypt a 4 KB message.

use criterion::{Criterion, criterion_group, criterion_main};
use rnp::{Algorithm, Cipher, Context, Encryptor, Hash, KeyBuilder, KeyUsage, Output, decrypt};

fn bench_encrypt_decrypt(c: &mut Criterion) {
    let ctx = Context::new().expect("ctx");
    let key = KeyBuilder::new(Algorithm::Rsa)
        .bits(2048)
        .userid("bench <bench@example.com>")
        .hash(Hash::Sha256)
        .add_usage(KeyUsage::EncryptComms)
        .build(&ctx)
        .expect("enc key");
    let plaintext = vec![0x42u8; 4096];

    c.bench_function("encrypt_4k", |b| {
        b.iter(|| {
            let mut output = Output::to_memory().unwrap();
            Encryptor::new(&ctx, &plaintext)
                .unwrap()
                .add_recipient(&key)
                .cipher(Cipher::Aes256)
                .build(&mut output)
                .unwrap();
            output
        });
    });

    let mut output = Output::to_memory().unwrap();
    Encryptor::new(&ctx, &plaintext)
        .unwrap()
        .add_recipient(&key)
        .cipher(Cipher::Aes256)
        .build(&mut output)
        .unwrap();
    let ciphertext = output.into_bytes().unwrap();

    c.bench_function("decrypt_4k", |b| {
        b.iter(|| decrypt(&ctx, &ciphertext).unwrap());
    });
}

criterion_group!(benches, bench_encrypt_decrypt);
criterion_main!(benches);
